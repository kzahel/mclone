$script:McloneQuestVdPackage = "VirtualDesktop.Android"
$script:McloneQuestVdManifest = "C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json"
$script:McloneQuestVdExe = "C:\Program Files\Virtual Desktop Streamer\VirtualDesktop.Streamer.exe"
$script:McloneQuestVdServiceName = "VirtualDesktop.Service.exe"

function Resolve-McloneQuestTestbedCli {
    $candidates = New-Object "System.Collections.Generic.List[string]"
    if ($env:QUEST_TESTBED_CLI) {
        $candidates.Add($env:QUEST_TESTBED_CLI)
    }
    $candidates.Add((Join-Path $PSScriptRoot "..\..\quest-testbed\bin\quest.ps1"))
    $candidates.Add((Join-Path $HOME "code\quest-testbed\bin\quest.ps1"))
    $candidates.Add((Join-Path $HOME "Documents\code\quest-testbed\bin\quest.ps1"))

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    throw "quest-testbed was not found. Clone https://github.com/kzahel/quest-testbed beside mclone, or set QUEST_TESTBED_CLI."
}

function Invoke-McloneQuestTestbed {
    param([string[]]$Arguments)

    $cli = Resolve-McloneQuestTestbedCli
    $output = @(& $cli @Arguments)
    if ($LASTEXITCODE -ne 0) {
        throw "quest-testbed failed with exit code $LASTEXITCODE`: $($Arguments -join ' ')"
    }
    return $output
}

function Get-McloneQuestSerial {
    param(
        [string]$AdbPath,
        [string]$Serial
    )

    $arguments = @()
    if ($AdbPath) {
        $arguments += @("--adb", $AdbPath)
    }
    if ($Serial) {
        $arguments += @("--serial", $Serial)
    }
    $arguments += "serial"
    $output = @(Invoke-McloneQuestTestbed -Arguments $arguments)
    if ($output.Count -eq 0) {
        throw "quest-testbed did not return a Quest serial."
    }
    return "$($output[-1])".Trim()
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
        [switch]$NoSystemPanelDismiss,
        [switch]$NoLaunch
    )

    $null = $StatePath
    $null = $ForceSave
    $Serial = Get-McloneQuestSerial -AdbPath $AdbPath -Serial $Serial
    $prefix = @()
    if ($AdbPath) {
        $prefix += @("--adb", $AdbPath)
    }
    $prefix += @("--serial", $Serial)
    $begin = $prefix + @(
        "begin",
        "--owner-pid", "$PID",
        "--stop-package", $PackageName
    )
    if ($NoWake) {
        $begin += "--no-wake"
    }
    if ($NoProximityDisable) {
        $begin += "--no-proximity-disable"
    }
    if ($NoSystemPanelDismiss) {
        $begin += "--no-dismiss-dialogs"
    }

    Write-Host "Preparing Quest $Serial through quest-testbed."
    Invoke-McloneQuestTestbed -Arguments $begin | ForEach-Object { Write-Host $_ }

    if (-not $NoLaunch) {
        Write-Host "Launching $PackageName on Quest $Serial."
        Invoke-McloneQuestTestbed -Arguments ($prefix + @("launch", $PackageName)) |
            ForEach-Object { Write-Host $_ }
        if ($LaunchWaitSeconds -gt 0) {
            Start-Sleep -Seconds $LaunchWaitSeconds
        }
    }

    return [pscustomobject]@{
        AdbPath = $AdbPath
        Serial = $Serial
        StatePath = "/data/local/tmp/quest-testbed-session.json"
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

    $null = $StatePath
    $null = $StopQuestApp
    $null = $SleepAfterRestore
    if ($KeepStateFile) {
        Write-Host "Warning: -KeepStateFile is obsolete; quest-testbed clears the journal only after successful restoration."
    }
    $Serial = Get-McloneQuestSerial -AdbPath $AdbPath -Serial $Serial
    $arguments = @()
    if ($AdbPath) {
        $arguments += @("--adb", $AdbPath)
    }
    $arguments += @("--serial", $Serial, "end")
    Invoke-McloneQuestTestbed -Arguments $arguments | ForEach-Object { Write-Host $_ }
}

function Suspend-McloneQuestHeadset {
    param(
        [string]$AdbPath,
        [string]$Serial
    )

    $Serial = Get-McloneQuestSerial -AdbPath $AdbPath -Serial $Serial
    $arguments = @()
    if ($AdbPath) {
        $arguments += @("--adb", $AdbPath)
    }
    $arguments += @("--serial", $Serial, "sleep")
    Invoke-McloneQuestTestbed -Arguments $arguments | ForEach-Object { Write-Host $_ }
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
