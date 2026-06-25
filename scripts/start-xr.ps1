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
$QuestStartupStatePath = Join-Path ([System.IO.Path]::GetTempPath()) "mclone-start-xr-quest-vd-state.json"

Import-Module (Join-Path $PSScriptRoot "xr-quest-virtual-desktop.psm1") -Force

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

$virtualDesktopManifest = Get-McloneVirtualDesktopRuntimeManifest
if ($RuntimeJson) {
    $env:XR_RUNTIME_JSON = $RuntimeJson
    Write-Host "Using OpenXR runtime: $RuntimeJson"
} elseif (-not $env:XR_RUNTIME_JSON -and -not $UseActiveRuntime -and -not $NoVirtualDesktop -and (Test-Path $virtualDesktopManifest)) {
    $env:XR_RUNTIME_JSON = $virtualDesktopManifest
    Write-Host "Using Virtual Desktop OpenXR runtime: $virtualDesktopManifest"
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

$questPrepared = $false
$exitCode = 0
try {
    if (-not $NoVirtualDesktop -and (Test-Path $virtualDesktopManifest)) {
        Start-McloneVirtualDesktopHost
    }

    if (-not $NoQuestLaunch) {
        Start-McloneQuestVirtualDesktop -StatePath $QuestStartupStatePath | Out-Null
        $questPrepared = $true
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
    if ($questPrepared -and -not $NoQuestRestore) {
        Restore-McloneQuestVirtualDesktopState -StatePath $QuestStartupStatePath -StopQuestApp -SleepAfterRestore
    }
}
exit $exitCode
