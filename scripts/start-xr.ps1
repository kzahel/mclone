param(
    [switch]$Release,
    [switch]$BuildOnly,
    [switch]$CheckOnly,
    [int]$Frames = 120,
    [switch]$UseActiveRuntime,
    [switch]$NoVirtualDesktop,
    [switch]$NoQuestLaunch,
    [switch]$NoQuestRestore,
    [switch]$AllowUnsupported,
    [string]$RuntimeJson,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$McloneArgs
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$VirtualDesktopManifest = "C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json"
$VirtualDesktopExe = "C:\Program Files\Virtual Desktop Streamer\VirtualDesktop.Streamer.exe"
$VirtualDesktopServiceName = "VirtualDesktop.Service.exe"
$QuestVirtualDesktopPackage = "VirtualDesktop.Android"
$UnsetMarker = "__mclone_unset__"
$QuestAdbPath = $null
$QuestSerial = $null
$QuestSettingsSaved = $false
$PreviousStayOn = $UnsetMarker
$PreviousSkipLaunchCheck = $UnsetMarker
$PreviousRequireControllers = $UnsetMarker

function Write-CommandLine {
    param([string[]]$Command)

    $quoted = $Command | ForEach-Object {
        if ($_ -match "[\s`"']") {
            '"' + ($_ -replace '"', '\"') + '"'
        } else {
            $_
        }
    }
    Write-Host ("Command: " + ($quoted -join " "))
}

function Get-ActiveOpenXrRuntime {
    try {
        $value = (Get-ItemProperty -Path "HKLM:\SOFTWARE\Khronos\OpenXR\1" -ErrorAction Stop).ActiveRuntime
        if ($value) {
            return $value
        }
    } catch {
    }
    return $null
}

function Start-VirtualDesktopHost {
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
    Start-Process -FilePath $VirtualDesktopExe -WorkingDirectory (Split-Path $VirtualDesktopExe)
    Start-Sleep -Seconds 5
}

function Invoke-NativeOutput {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    $oldErrorActionPreference = $ErrorActionPreference
    $exitCode = 0
    try {
        $ErrorActionPreference = "Continue"
        $output = & $FilePath @Arguments 2> $null
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

function Invoke-NativeQuiet {
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

function Invoke-AdbQuiet {
    param([string[]]$Arguments)

    if (-not $QuestAdbPath -or -not $QuestSerial) {
        return
    }
    Invoke-NativeQuiet $QuestAdbPath (@("-s", $QuestSerial) + $Arguments)
}

function Invoke-AdbOutput {
    param([string[]]$Arguments)

    if (-not $QuestAdbPath -or -not $QuestSerial) {
        return @()
    }
    return Invoke-NativeOutput $QuestAdbPath (@("-s", $QuestSerial) + $Arguments)
}

function Read-AndroidSetting {
    param(
        [string]$Namespace,
        [string]$Name
    )

    try {
        $value = (Invoke-AdbOutput @("shell", "settings", "get", $Namespace, $Name)) -join "`n"
        $value = $value.Trim()
        if (-not $value -or $value -eq "null") {
            return $UnsetMarker
        }
        return $value
    } catch {
        return $UnsetMarker
    }
}

function Restore-AndroidSetting {
    param(
        [string]$Namespace,
        [string]$Name,
        [string]$Value
    )

    try {
        if ($Value -eq $UnsetMarker) {
            Invoke-AdbQuiet @("shell", "settings", "delete", $Namespace, $Name)
        } else {
            Invoke-AdbQuiet @("shell", "settings", "put", $Namespace, $Name, $Value)
        }
    } catch {
        Write-Host "Warning: failed to restore Android setting $Namespace/$Name`: $($_.Exception.Message)"
    }
}

function Save-QuestSettings {
    if ($QuestSettingsSaved) {
        return
    }

    $script:PreviousStayOn = Read-AndroidSetting "global" "stay_on_while_plugged_in"
    $script:PreviousSkipLaunchCheck = Read-AndroidSetting "secure" "skip_launch_check_requires_controllers_enabled"
    $script:PreviousRequireControllers = Read-AndroidSetting "global" "require_controllers_for_vr_apps"
    $script:QuestSettingsSaved = $true
}

function Disable-QuestProximitySensor {
    Invoke-AdbQuiet @("shell", "setprop", "debug.oculus.disableProximity", "1")
}

function Enable-QuestProximitySensor {
    try {
        Invoke-AdbQuiet @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to re-enable Quest proximity property: $($_.Exception.Message)"
    }
    try {
        Invoke-AdbQuiet @("shell", "am", "broadcast", "-a", "com.oculus.vrpowermanager.prox_open", "--ei", "timeout", "0")
    } catch {
        Write-Host "Warning: failed to send Quest proximity-open broadcast: $($_.Exception.Message)"
    }
}

function Restore-QuestAfterTest {
    if ($NoQuestRestore -or -not $QuestSettingsSaved -or -not $QuestAdbPath -or -not $QuestSerial) {
        return
    }

    Write-Host "Restoring Quest wake/proximity settings."
    try {
        Invoke-AdbQuiet @("shell", "am", "force-stop", $QuestVirtualDesktopPackage)
    } catch {
    }
    Restore-AndroidSetting "global" "stay_on_while_plugged_in" $PreviousStayOn
    Restore-AndroidSetting "secure" "skip_launch_check_requires_controllers_enabled" $PreviousSkipLaunchCheck
    Restore-AndroidSetting "global" "require_controllers_for_vr_apps" $PreviousRequireControllers
    Enable-QuestProximitySensor
    try {
        Invoke-AdbQuiet @("shell", "input", "keyevent", "KEYCODE_SLEEP")
    } catch {
        Write-Host "Warning: failed to sleep Quest after restore: $($_.Exception.Message)"
    }
    try {
        Invoke-AdbQuiet @("shell", "setprop", "debug.oculus.disableProximity", "0")
    } catch {
        Write-Host "Warning: failed to clear Quest proximity property after restore: $($_.Exception.Message)"
    }
}

function Get-FirstAdbDeviceSerial {
    param([string]$AdbPath)

    $devices = Invoke-NativeOutput $AdbPath @("devices")
    foreach ($line in $devices) {
        if ($line -match "^(?<serial>\S+)\s+device$") {
            return $Matches.serial
        }
    }
    return $null
}

function Start-QuestVirtualDesktop {
    $adb = Get-Command adb -ErrorAction SilentlyContinue
    if (-not $adb) {
        Write-Host "adb was not found; skipping Quest Virtual Desktop launch."
        return
    }

    $serial = Get-FirstAdbDeviceSerial $adb.Source
    if (-not $serial) {
        Write-Host "No adb device is connected; skipping Quest Virtual Desktop launch."
        return
    }

    $script:QuestAdbPath = $adb.Source
    $script:QuestSerial = $serial
    Save-QuestSettings

    Write-Host "Waking Quest $serial and launching $QuestVirtualDesktopPackage."
    Disable-QuestProximitySensor
    Invoke-AdbQuiet @("shell", "settings", "put", "global", "stay_on_while_plugged_in", "3")
    Invoke-AdbQuiet @("shell", "settings", "put", "secure", "skip_launch_check_requires_controllers_enabled", "1")
    Invoke-AdbQuiet @("shell", "settings", "put", "global", "require_controllers_for_vr_apps", "0")
    Invoke-AdbQuiet @("shell", "input", "keyevent", "KEYCODE_WAKEUP")
    Invoke-AdbQuiet @("shell", "am", "broadcast", "-a", "com.oculus.vrpowermanager.prox_close", "--ei", "timeout", "0")
    Invoke-AdbQuiet @(
        "shell",
        "monkey",
        "-p",
        $QuestVirtualDesktopPackage,
        "-c",
        "android.intent.category.LAUNCHER",
        "1"
    )
    Start-Sleep -Seconds 15
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Missing required command: cargo. Install Rust with rustup, or make sure Cargo is on PATH."
}

$platformSupported =
    ($env:OS -eq "Windows_NT") -or
    ($PSVersionTable.PSEdition -eq "Core" -and ($IsWindows -or $IsLinux -or $IsMacOS))
if (-not $platformSupported -and -not $AllowUnsupported) {
    Write-Error "mclone XR startup is implemented for macOS Metal and Windows/Linux Vulkan. Use -AllowUnsupported to try this platform anyway."
}

$cargoMode = if ($Release) { "--release" } else { $null }
$cargoCheck = @("cargo", "check")
$cargoBuild = @("cargo", "build")
$cargoRun = @("cargo", "run")
if ($cargoMode) {
    $cargoCheck += $cargoMode
    $cargoBuild += $cargoMode
    $cargoRun += $cargoMode
}
$cargoCommon = @("--manifest-path", "native/Cargo.toml", "-p", "mclone-native-client", "--features", "xr")
$cargoCheck += $cargoCommon
$cargoBuild += $cargoCommon
$cargoRun += $cargoCommon

if ($CheckOnly) {
    Write-Host "Checking mclone XR feature..."
    Write-CommandLine $cargoCheck
    Push-Location $RepoRoot
    try {
        & $cargoCheck[0] $cargoCheck[1..($cargoCheck.Count - 1)]
    } finally {
        Pop-Location
    }
    exit $LASTEXITCODE
}

if ($BuildOnly) {
    Write-Host "Building mclone XR binary..."
    Write-CommandLine $cargoBuild
    Push-Location $RepoRoot
    try {
        & $cargoBuild[0] $cargoBuild[1..($cargoBuild.Count - 1)]
    } finally {
        Pop-Location
    }
    exit $LASTEXITCODE
}

if ($RuntimeJson) {
    $env:XR_RUNTIME_JSON = $RuntimeJson
    Write-Host "Using OpenXR runtime: $RuntimeJson"
} elseif (-not $env:XR_RUNTIME_JSON -and -not $UseActiveRuntime -and -not $NoVirtualDesktop -and (Test-Path $VirtualDesktopManifest)) {
    $env:XR_RUNTIME_JSON = $VirtualDesktopManifest
    Write-Host "Using Virtual Desktop OpenXR runtime: $VirtualDesktopManifest"
} elseif (-not $env:XR_RUNTIME_JSON -and $env:OS -eq "Windows_NT") {
    $activeRuntime = Get-ActiveOpenXrRuntime
    if ($activeRuntime) {
        $env:XR_RUNTIME_JSON = $activeRuntime
        Write-Host "Using OpenXR active runtime: $activeRuntime"
    } else {
        Write-Host "No OpenXR ActiveRuntime registry entry found; relying on loader discovery."
    }
} elseif ($env:XR_RUNTIME_JSON) {
    Write-Host "Using OpenXR runtime from environment: $env:XR_RUNTIME_JSON"
}

$exitCode = 0
try {
    if (-not $NoVirtualDesktop -and (Test-Path $VirtualDesktopManifest)) {
        Start-VirtualDesktopHost
    }

    if (-not $NoQuestLaunch) {
        Start-QuestVirtualDesktop
    }

    if (-not $env:RUST_LOG) {
        $env:RUST_LOG = "warn,mclone_native_client=info"
    }

    $appArgs = @("--xr-clear-smoke", "--frames", "$Frames") + $McloneArgs
    $runCommand = $cargoRun + @("--") + $appArgs

    Write-Host "Starting mclone XR smoke..."
    Write-CommandLine $runCommand
    Push-Location $RepoRoot
    try {
        & $cargoRun[0] $cargoRun[1..($cargoRun.Count - 1)] -- @appArgs
        $exitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
} finally {
    Restore-QuestAfterTest
}
exit $exitCode
