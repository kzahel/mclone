param(
    [switch]$Restore,
    [switch]$NoVirtualDesktop,
    [switch]$NoQuestLaunch,
    [switch]$NoWake,
    [switch]$NoProximityDisable,
    [switch]$ForceSave,
    [switch]$StopQuestApp,
    [switch]$SleepAfterRestore,
    [switch]$KeepStateFile,
    [string]$StatePath,
    [string]$AdbSerial,
    [int]$LaunchWaitSeconds = 15
)

$ErrorActionPreference = "Stop"

# Keep this flow aligned with Playbox's Quest wake/restore helpers:
#   ~/code/playbox/android/validate-common.sh
#   ~/code/playbox/scripts/run_playbox_wivrn_capture.sh
# Use those as the source pattern before adding new ADB wake/proximity behavior.
Import-Module (Join-Path $PSScriptRoot "xr-quest-virtual-desktop.psm1") -Force

if ($Restore) {
    Restore-McloneQuestVirtualDesktopState `
        -StatePath $StatePath `
        -Serial $AdbSerial `
        -StopQuestApp:$StopQuestApp `
        -SleepAfterRestore:$SleepAfterRestore `
        -KeepStateFile:$KeepStateFile
    exit 0
}

if (-not $NoVirtualDesktop) {
    Start-McloneVirtualDesktopHost
}

if (-not $NoQuestLaunch) {
    $result = Start-McloneQuestVirtualDesktop `
        -StatePath $StatePath `
        -Serial $AdbSerial `
        -LaunchWaitSeconds $LaunchWaitSeconds `
        -ForceSave:$ForceSave `
        -NoWake:$NoWake `
        -NoProximityDisable:$NoProximityDisable

    Write-Host "Quest Virtual Desktop startup sequence is ready."
    Write-Host "Connect to this PC in the headset, then run: pnpm native:xr:windows:smoke:connected"
    Write-Host "Restore Quest wake/proximity settings with: pnpm native:xr:windows:restore"
    Write-Host "Restore state path: $($result.StatePath)"
} else {
    Write-Host "Virtual Desktop host startup complete; Quest launch skipped."
}
