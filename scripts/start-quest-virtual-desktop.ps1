param(
    [switch]$Restore,
    [switch]$SleepOnly,
    [switch]$NoVirtualDesktop,
    [switch]$NoQuestLaunch,
    [switch]$NoWake,
    [switch]$NoProximityDisable,
    [switch]$NoSystemPanelDismiss,
    [switch]$ForceSave,
    [switch]$StopQuestApp,
    [switch]$SleepAfterRestore,
    [switch]$KeepStateFile,
    [string]$StatePath,
    [string]$AdbSerial,
    [int]$LaunchWaitSeconds = 15
)

$ErrorActionPreference = "Stop"

# quest-testbed owns ADB selection, wake/proximity state, recovery, and sleep.
# This module retains only the Virtual Desktop host and app-launch policy.
Import-Module (Join-Path $PSScriptRoot "xr-quest-virtual-desktop.psm1") -Force

if ($SleepOnly) {
    Suspend-McloneQuestHeadset -Serial $AdbSerial
    exit 0
}

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
        -NoProximityDisable:$NoProximityDisable `
        -NoSystemPanelDismiss:$NoSystemPanelDismiss

    Write-Host "Quest Virtual Desktop startup sequence is ready."
    Write-Host "Connect to this PC in the headset, then run: pnpm native:xr:windows:smoke:connected"
    Write-Host "Restore the quest-testbed lease and sleep with: pnpm native:xr:windows:restore"
    Write-Host "Recovery journal: $($result.StatePath)"
} else {
    Write-Host "Virtual Desktop host startup complete; Quest launch skipped."
}
