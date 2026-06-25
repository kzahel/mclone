$script:McloneQuestVdUnsetMarker = "__mclone_unset__"
$script:McloneQuestVdDefaultStatePath = Join-Path ([System.IO.Path]::GetTempPath()) "mclone-quest-virtual-desktop-state.json"
$script:McloneQuestVdPackage = "VirtualDesktop.Android"
$script:McloneQuestVdManifest = "C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json"
$script:McloneQuestVdExe = "C:\Program Files\Virtual Desktop Streamer\VirtualDesktop.Streamer.exe"
$script:McloneQuestVdServiceName = "VirtualDesktop.Service.exe"

function Resolve-McloneQuestVdStatePath {
    param([string]$StatePath)

    if ($StatePath) {
        return $StatePath
    }
    return $script:McloneQuestVdDefaultStatePath
}

function Invoke-McloneNativeOutput {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    $oldErrorActionPreference = $ErrorActionPreference
    $exitCode = 0
    try {
        $ErrorActionPreference = "Continue"
        $output = @(& $FilePath @Arguments 2> $null)
        if ($null -ne $LASTEXITCODE) {
            $exitCode = $LASTEXITCODE
        }
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
    }

    if ($exitCode -ne 0) {
        throw "$FilePath $($Arguments -join ' ') failed with exit code $exitCode"
    }

    return $output
}

function Invoke-McloneNativeQuiet {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    $oldErrorActionPreference = $ErrorActionPreference
    $exitCode = 0
    try {
        $ErrorActionPreference = "Continue"
        & $FilePath @Arguments *> $null
        if ($null -ne $LASTEXITCODE) {
            $exitCode = $LASTEXITCODE
        }
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
    }

    if ($exitCode -ne 0) {
        throw "$FilePath $($Arguments -join ' ') failed with exit code $exitCode"
    }
}

function Invoke-McloneAdbQuiet {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string[]]$Arguments
    )

    Invoke-McloneNativeQuiet $AdbPath (@("-s", $Serial) + $Arguments)
}

function Invoke-McloneAdbOutput {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string[]]$Arguments
    )

    return Invoke-McloneNativeOutput $AdbPath (@("-s", $Serial) + $Arguments)
}

function Get-McloneFirstAdbDeviceSerial {
    param([string]$AdbPath)

    $devices = Invoke-McloneNativeOutput $AdbPath @("devices")
    foreach ($line in $devices) {
        if ($line -match "^(?<serial>\S+)\s+device$") {
            return $Matches.serial
        }
    }
    return $null
}

function Get-McloneQuestAdbTarget {
    param(
        [string]$AdbPath,
        [string]$Serial
    )

    if (-not $AdbPath) {
        $adb = Get-Command adb -ErrorAction SilentlyContinue
        if (-not $adb) {
            throw "adb was not found. Install Android platform-tools, or put adb on PATH."
        }
        $AdbPath = $adb.Source
    }

    if (-not $Serial) {
        $Serial = Get-McloneFirstAdbDeviceSerial $AdbPath
    }

    if (-not $Serial) {
        throw "No adb device is connected."
    }

    return [pscustomobject]@{
        AdbPath = $AdbPath
        Serial = $Serial
    }
}

function Read-McloneAndroidSetting {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string]$Namespace,
        [string]$Name
    )

    try {
        $value = (Invoke-McloneAdbOutput $AdbPath $Serial @("shell", "settings", "get", $Namespace, $Name)) -join "`n"
        $value = $value.Trim()
        if (-not $value -or $value -eq "null") {
            return $script:McloneQuestVdUnsetMarker
        }
        return $value
    } catch {
        return $script:McloneQuestVdUnsetMarker
    }
}

function Restore-McloneAndroidSetting {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string]$Namespace,
        [string]$Name,
        [string]$Value
    )

    try {
        if ($Value -eq $script:McloneQuestVdUnsetMarker) {
            Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "settings", "delete", $Namespace, $Name)
        } else {
            Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "settings", "put", $Namespace, $Name, $Value)
        }
    } catch {
        Write-Host "Warning: failed to restore Android setting $Namespace/$Name`: $($_.Exception.Message)"
    }
}

function Save-McloneQuestVirtualDesktopState {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string]$StatePath,
        [string]$PackageName = $script:McloneQuestVdPackage,
        [switch]$Force
    )

    $resolvedStatePath = Resolve-McloneQuestVdStatePath $StatePath
    if ((Test-Path $resolvedStatePath) -and -not $Force) {
        Write-Host "Quest restore state already exists; preserving it: $resolvedStatePath"
        return $resolvedStatePath
    }

    $state = [ordered]@{
        saved_at = (Get-Date).ToUniversalTime().ToString("o")
        adb_serial = $Serial
        package = $PackageName
        settings = [ordered]@{
            stay_on_while_plugged_in = Read-McloneAndroidSetting $AdbPath $Serial "global" "stay_on_while_plugged_in"
            skip_launch_check_requires_controllers_enabled = Read-McloneAndroidSetting $AdbPath $Serial "secure" "skip_launch_check_requires_controllers_enabled"
            require_controllers_for_vr_apps = Read-McloneAndroidSetting $AdbPath $Serial "global" "require_controllers_for_vr_apps"
        }
    }

    $stateDir = Split-Path $resolvedStatePath -Parent
    if ($stateDir -and -not (Test-Path $stateDir)) {
        New-Item -ItemType Directory -Path $stateDir | Out-Null
    }

    $state | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $resolvedStatePath -Encoding UTF8
    Write-Host "Saved Quest restore state: $resolvedStatePath"
    return $resolvedStatePath
}

function Start-McloneVirtualDesktopHost {
    param(
        [string]$VirtualDesktopExe = $script:McloneQuestVdExe,
        [string]$VirtualDesktopServiceName = $script:McloneQuestVdServiceName
    )

    $service = Get-Service -Name $VirtualDesktopServiceName -ErrorAction SilentlyContinue
    if ($service) {
        if ($service.Status -ne "Running") {
            Write-Host "Starting Virtual Desktop service: $VirtualDesktopServiceName"
            Start-Service -Name $VirtualDesktopServiceName
            $service.WaitForStatus("Running", [TimeSpan]::FromSeconds(15))
        } else {
            Write-Host "Virtual Desktop service is already running."
        }
    } else {
        Write-Host "Virtual Desktop service was not found: $VirtualDesktopServiceName"
    }

    $streamer = Get-Process -Name "VirtualDesktop.Streamer" -ErrorAction SilentlyContinue
    if ($streamer) {
        Write-Host "Virtual Desktop Streamer is already running."
        return
    }

    if (-not (Test-Path $VirtualDesktopExe)) {
        Write-Host "Virtual Desktop Streamer executable was not found: $VirtualDesktopExe"
        return
    }

    Write-Host "Launching Virtual Desktop Streamer."
    Start-Process -FilePath $VirtualDesktopExe -WorkingDirectory (Split-Path $VirtualDesktopExe) -WindowStyle Hidden
    Start-Sleep -Seconds 5
}

function Start-McloneQuestVirtualDesktop {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string]$StatePath,
        [string]$PackageName = $script:McloneQuestVdPackage,
        [int]$LaunchWaitSeconds = 15,
        [switch]$ForceSave,
        [switch]$NoWake,
        [switch]$NoProximityDisable,
        [switch]$NoLaunch
    )

    $target = Get-McloneQuestAdbTarget -AdbPath $AdbPath -Serial $Serial
    $AdbPath = $target.AdbPath
    $Serial = $target.Serial

    Save-McloneQuestVirtualDesktopState -AdbPath $AdbPath -Serial $Serial -StatePath $StatePath -PackageName $PackageName -Force:$ForceSave | Out-Null

    Write-Host "Preparing Quest $Serial for Virtual Desktop."
    if (-not $NoProximityDisable) {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "setprop", "debug.oculus.disableProximity", "1")
    }
    Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "settings", "put", "global", "stay_on_while_plugged_in", "3")
    Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "settings", "put", "secure", "skip_launch_check_requires_controllers_enabled", "1")
    Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "settings", "put", "global", "require_controllers_for_vr_apps", "0")

    if (-not $NoWake) {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "input", "keyevent", "KEYCODE_WAKEUP")
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "am", "broadcast", "-a", "com.oculus.vrpowermanager.prox_close", "--ei", "timeout", "0")
    }

    if (-not $NoLaunch) {
        Write-Host "Launching $PackageName on Quest $Serial."
        Invoke-McloneAdbQuiet $AdbPath $Serial @(
            "shell",
            "monkey",
            "-p",
            $PackageName,
            "-c",
            "android.intent.category.LAUNCHER",
            "1"
        )
        if ($LaunchWaitSeconds -gt 0) {
            Start-Sleep -Seconds $LaunchWaitSeconds
        }
    }

    return [pscustomobject]@{
        AdbPath = $AdbPath
        Serial = $Serial
        StatePath = Resolve-McloneQuestVdStatePath $StatePath
    }
}

function Restore-McloneQuestVirtualDesktopState {
    param(
        [string]$AdbPath,
        [string]$Serial,
        [string]$StatePath,
        [switch]$StopQuestApp,
        [switch]$SleepAfterRestore,
        [switch]$KeepStateFile
    )

    $resolvedStatePath = Resolve-McloneQuestVdStatePath $StatePath
    if (-not (Test-Path $resolvedStatePath)) {
        Write-Host "No Quest restore state found: $resolvedStatePath"
        return
    }

    $state = Get-Content -LiteralPath $resolvedStatePath -Raw | ConvertFrom-Json
    $target = Get-McloneQuestAdbTarget -AdbPath $AdbPath -Serial $(if ($Serial) { $Serial } else { $state.adb_serial })
    $AdbPath = $target.AdbPath
    $Serial = $target.Serial
    $packageName = if ($state.package) { $state.package } else { $script:McloneQuestVdPackage }

    Write-Host "Restoring Quest wake/proximity settings from $resolvedStatePath."
    if ($StopQuestApp) {
        try {
            Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "am", "force-stop", $packageName)
        } catch {
            Write-Host "Warning: failed to stop Quest package $packageName`: $($_.Exception.Message)"
        }
    }

    Restore-McloneAndroidSetting $AdbPath $Serial "global" "stay_on_while_plugged_in" $state.settings.stay_on_while_plugged_in
    Restore-McloneAndroidSetting $AdbPath $Serial "secure" "skip_launch_check_requires_controllers_enabled" $state.settings.skip_launch_check_requires_controllers_enabled
    Restore-McloneAndroidSetting $AdbPath $Serial "global" "require_controllers_for_vr_apps" $state.settings.require_controllers_for_vr_apps

    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to clear Quest proximity property: $($_.Exception.Message)"
    }
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "am", "broadcast", "-a", "com.oculus.vrpowermanager.prox_open", "--ei", "timeout", "0")
    } catch {
        Write-Host "Warning: failed to send Quest proximity-open broadcast: $($_.Exception.Message)"
    }
    if ($SleepAfterRestore) {
        try {
            Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "input", "keyevent", "KEYCODE_SLEEP")
        } catch {
            Write-Host "Warning: failed to sleep Quest after restore: $($_.Exception.Message)"
        }
    }
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to clear Quest proximity property after restore: $($_.Exception.Message)"
    }

    if (-not $KeepStateFile) {
        Remove-Item -LiteralPath $resolvedStatePath -Force
    }
}

function Suspend-McloneQuestHeadset {
    param(
        [string]$AdbPath,
        [string]$Serial
    )

    $target = Get-McloneQuestAdbTarget -AdbPath $AdbPath -Serial $Serial
    $AdbPath = $target.AdbPath
    $Serial = $target.Serial

    Write-Host "Sleeping Quest $Serial."
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to clear Quest proximity property: $($_.Exception.Message)"
    }
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "am", "broadcast", "-a", "com.oculus.vrpowermanager.prox_open", "--ei", "timeout", "0")
    } catch {
        Write-Host "Warning: failed to send Quest proximity-open broadcast: $($_.Exception.Message)"
    }
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "input", "keyevent", "KEYCODE_SLEEP")
    } catch {
        Write-Host "Warning: failed to sleep Quest: $($_.Exception.Message)"
    }
    try {
        Invoke-McloneAdbQuiet $AdbPath $Serial @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to clear Quest proximity property after sleep: $($_.Exception.Message)"
    }
}

function Get-McloneVirtualDesktopRuntimeManifest {
    return $script:McloneQuestVdManifest
}

Export-ModuleMember -Function @(
    "Get-McloneVirtualDesktopRuntimeManifest",
    "Start-McloneVirtualDesktopHost",
    "Start-McloneQuestVirtualDesktop",
    "Restore-McloneQuestVirtualDesktopState",
    "Suspend-McloneQuestHeadset"
)
