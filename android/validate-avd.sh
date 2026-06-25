#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/validate-common.sh"

MCLONE_ANDROID_APP_ID="${MCLONE_ANDROID_APP_ID:-com.kzahel.mclone}"
MCLONE_ANDROID_ACTIVITY="${MCLONE_ANDROID_ACTIVITY:-android.app.NativeActivity}"
APK_PATH="${MCLONE_ANDROID_APK:-$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk}"
AVD_NAME="${MCLONE_ANDROID_AVD:-jstorrent-tablet}"
SCREENSHOT_PATH="${MCLONE_ANDROID_SCREENSHOT:-/tmp/mclone-android-avd-clear.png}"
LOG_PATH="${MCLONE_ANDROID_LOGCAT:-/tmp/mclone-android-avd-logcat.txt}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-120}"
SMOKE_SECONDS="${MCLONE_ANDROID_SMOKE_SECONDS:-3}"
STAGE_ASSETS="${MCLONE_ANDROID_STAGE_ASSETS:-1}"
SKIP_BUILD=0
KEEP_EMULATOR=0
HEADLESS=1
SERIAL=""
STARTED_EMULATOR=0
EMULATOR_PID=""

usage() {
    cat <<'USAGE'
Usage: android/validate-avd.sh [options]

Build, install, launch, and smoke-test the flat Mclone Android APK on an AVD.

Options:
  --avd NAME          AVD name to boot when no emulator is already online.
  --serial SERIAL     Use an already-running emulator/device serial.
  --skip-build        Reuse the existing APK.
  --keep-emulator     Leave an emulator started by this script running.
  --window            Show the emulator window instead of using -no-window.
  --screenshot PATH   Local screenshot output path.
  --log PATH          Local logcat output path.
  --timeout SECONDS   Boot/device wait timeout.
  --smoke-seconds N   Seconds to wait after launch before validation.
  --asset-pack PATH   Local packed assets file to stage before launch.
  --skip-assets       Do not stage the packed Minecraft assets before launch.
  --touch-swipe SPEC  Inject a touch swipe before capture: x1,y1,x2,y2,duration_ms.
  -h, --help          Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --avd)
            AVD_NAME="$2"
            shift 2
            ;;
        --serial)
            SERIAL="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --keep-emulator)
            KEEP_EMULATOR=1
            shift
            ;;
        --window)
            HEADLESS=0
            shift
            ;;
        --screenshot)
            SCREENSHOT_PATH="$2"
            shift 2
            ;;
        --log)
            LOG_PATH="$2"
            shift 2
            ;;
        --timeout)
            BOOT_TIMEOUT_SECONDS="$2"
            shift 2
            ;;
        --smoke-seconds)
            SMOKE_SECONDS="$2"
            shift 2
            ;;
        --asset-pack)
            MCLONE_ANDROID_ASSET_PACK="$2"
            shift 2
            ;;
        --skip-assets)
            STAGE_ASSETS=0
            shift
            ;;
        --touch-swipe)
            MCLONE_ANDROID_TOUCH_SWIPE="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            mclone_die "unknown option: $1"
            ;;
    esac
done

cleanup() {
    local status=$?
    if [[ "$STARTED_EMULATOR" == "1" && "$KEEP_EMULATOR" != "1" && -n "$SERIAL" ]]; then
        mclone_note "Stopping emulator $SERIAL"
        "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1 || true
    elif [[ "$STARTED_EMULATOR" == "1" && "$KEEP_EMULATOR" != "1" && -n "$EMULATOR_PID" ]]; then
        kill "$EMULATOR_PID" >/dev/null 2>&1 || true
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM

cd "$REPO_ROOT"
ADB="$(mclone_android_tool adb platform-tools/adb)"
EMULATOR="$(mclone_android_tool emulator emulator/emulator)"

mclone_build_apk
"$ADB" start-server >/dev/null

if [[ -z "$SERIAL" ]]; then
    SERIAL="$(mclone_online_devices | awk '/^emulator-/ { print; exit }')"
fi

if [[ -z "$SERIAL" ]]; then
    unauthorized_devices="$(mclone_unauthorized_devices | paste -sd, -)"
    if [[ -n "$unauthorized_devices" ]]; then
        mclone_die "attached Android device(s) are unauthorized: $unauthorized_devices"
    fi

    emulator_args=(-avd "$AVD_NAME" -no-audio -no-boot-anim -no-snapshot-save)
    if [[ "$HEADLESS" == "1" ]]; then
        emulator_args+=(-no-window)
    fi

    mclone_note "Starting AVD $AVD_NAME"
    "$EMULATOR" "${emulator_args[@]}" &
    EMULATOR_PID="$!"
    STARTED_EMULATOR=1
    SERIAL="$(mclone_wait_for_any_emulator "$BOOT_TIMEOUT_SECONDS")"
else
    mclone_note "Using existing device $SERIAL"
fi

mclone_wait_for_boot "$SERIAL" "$BOOT_TIMEOUT_SECONDS"
mclone_install_launch_smoke "$SERIAL" "$SCREENSHOT_PATH" "$LOG_PATH" "$SMOKE_SECONDS"
