param(
    [switch]$Release,
    [switch]$BuildOnly,
    [switch]$CheckOnly,
    [int]$Frames = 120,
    [switch]$UseActiveRuntime,
    [switch]$NoVirtualDesktop,
    [switch]$NoQuestLaunch,
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

function Start-QuestVirtualDesktop {
    $adb = Get-Command adb -ErrorAction SilentlyContinue
    if (-not $adb) {
        Write-Host "adb was not found; skipping Quest Virtual Desktop launch."
        return
    }

    $devices = & $adb.Source devices
    $hasDevice = $devices | Select-String -Pattern "`tdevice$" -Quiet
    if (-not $hasDevice) {
        Write-Host "No adb device is connected; skipping Quest Virtual Desktop launch."
        return
    }

    Write-Host "Waking Quest and launching $QuestVirtualDesktopPackage."
    Invoke-NativeQuiet $adb.Source @("shell", "svc", "power", "stayon", "true")
    Invoke-NativeQuiet $adb.Source @("shell", "input", "keyevent", "KEYCODE_WAKEUP")
    Invoke-NativeQuiet $adb.Source @(
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
} finally {
    Pop-Location
}
exit $LASTEXITCODE
