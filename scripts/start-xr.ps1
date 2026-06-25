param(
    [switch]$Release,
    [switch]$BuildOnly,
    [switch]$CheckOnly,
    [ValidateSet("clear", "mclone")]
    [string]$Smoke = "clear",
    [int]$Frames = 120,
    [ValidateSet("virtual-desktop", "active", "json", "environment")]
    [string]$Runtime = "virtual-desktop",
    [switch]$UseActiveRuntime,
    [switch]$NoVirtualDesktop,
    [switch]$NoQuestLaunch,
    [switch]$NoQuestRestore,
    [switch]$NoSystemPanelDismiss,
    [switch]$AllowUnsupported,
    [string]$RuntimeJson,
    [string]$ViewPose,
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
if ($RuntimeJson -and $Runtime -eq "virtual-desktop") {
    $Runtime = "json"
}
if ($UseActiveRuntime) {
    $Runtime = "active"
}
if ($NoVirtualDesktop -and $Runtime -eq "virtual-desktop") {
    $Runtime = "active"
}

if ($Runtime -eq "json") {
    if (-not $RuntimeJson) {
        Write-Error "-Runtime json requires -RuntimeJson PATH."
    }
    $env:XR_RUNTIME_JSON = $RuntimeJson
    Write-Host "Using OpenXR runtime JSON: $RuntimeJson"
} elseif ($Runtime -eq "virtual-desktop") {
    if (Test-Path $virtualDesktopManifest) {
        $env:XR_RUNTIME_JSON = $virtualDesktopManifest
        Write-Host "Using Virtual Desktop OpenXR runtime: $virtualDesktopManifest"
    } else {
        Write-Host "Virtual Desktop OpenXR runtime was not found; using active runtime discovery."
        $Runtime = "active"
    }
}

if ($Runtime -eq "active" -and $env:OS -eq "Windows_NT") {
    Remove-Item Env:XR_RUNTIME_JSON -ErrorAction SilentlyContinue
    $activeRuntime = Get-ActiveOpenXrRuntime
    if ($activeRuntime) {
        Write-Host "Using OpenXR active runtime: $activeRuntime"
    } else {
        Write-Host "No OpenXR ActiveRuntime registry entry found; relying on loader discovery."
    }
} elseif ($Runtime -eq "environment" -and $env:XR_RUNTIME_JSON) {
    Write-Host "Using OpenXR runtime from environment: $env:XR_RUNTIME_JSON"
} elseif ($Runtime -eq "environment") {
    Write-Host "Using OpenXR loader/runtime discovery from the current environment."
}

$questPrepared = $false
$exitCode = 0
try {
    if ($Runtime -eq "virtual-desktop" -and -not $NoVirtualDesktop -and (Test-Path $virtualDesktopManifest)) {
        Start-McloneVirtualDesktopHost
    }

    if ($Runtime -eq "virtual-desktop" -and -not $NoQuestLaunch) {
        Start-McloneQuestVirtualDesktop -StatePath $QuestStartupStatePath -NoSystemPanelDismiss:$NoSystemPanelDismiss | Out-Null
        $questPrepared = $true
    }

    if (-not $env:RUST_LOG) {
        $env:RUST_LOG = "warn,mclone_native_client=info"
    }

    $smokeFlag = if ($Smoke -eq "mclone") { "--xr-mclone-smoke" } else { "--xr-clear-smoke" }
    if ($ViewPose -and $Smoke -ne "mclone") {
        Write-Error "-ViewPose requires -Smoke mclone."
    }
    $appArgs = @($smokeFlag, "--frames", "$Frames")
    if ($ViewPose) {
        $appArgs += @("--xr-view-pose", $ViewPose)
    }
    $appArgs += $McloneArgs
    $runCommand = $cargoRun + @("--") + $appArgs

    Write-Host "Starting mclone XR $Smoke smoke..."
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
    } elseif ($NoQuestRestore) {
        Write-Host "Quest restore/sleep skipped by -NoQuestRestore; headset may remain awake."
    } elseif ($NoQuestLaunch) {
        try {
            Restore-McloneQuestVirtualDesktopState -SleepAfterRestore
        } catch {
            Write-Host "Warning: failed to restore Quest state after connected smoke: $($_.Exception.Message)"
        }
        try {
            Suspend-McloneQuestHeadset
        } catch {
            Write-Host "Warning: failed to sleep Quest after connected smoke: $($_.Exception.Message)"
        }
    }
}
exit $exitCode
