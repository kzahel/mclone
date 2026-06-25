#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/validate-common.sh"

MCLONE_ANDROID_APP_ID="${MCLONE_ANDROID_APP_ID:-com.kzahel.mclone}"
MCLONE_ANDROID_ACTIVITY="${MCLONE_ANDROID_ACTIVITY:-android.app.NativeActivity}"
MCLONE_ANDROID_REQUIRE_FOCUS="${MCLONE_ANDROID_REQUIRE_FOCUS:-0}"
APK_PATH="${MCLONE_ANDROID_APK:-$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk}"
SCREENSHOT_PATH="${MCLONE_ANDROID_SCREENSHOT:-/tmp/mclone-quest-flat.png}"
LOG_PATH="${MCLONE_ANDROID_LOGCAT:-/tmp/mclone-quest-flat-logcat.txt}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
SMOKE_SECONDS="${MCLONE_ANDROID_SMOKE_SECONDS:-3}"
STAGE_ASSETS="${MCLONE_ANDROID_STAGE_ASSETS:-1}"
SKIP_BUILD=0
SERIAL=""

cleanup() {
    local status=$?
    if [[ -n "$SERIAL" ]]; then
        mclone_restore_headset_after_test "$SERIAL" "$MCLONE_ANDROID_APP_ID"
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM

usage() {
    cat <<'USAGE'
Usage: android/validate-quest-flat.sh [options]

Build, install, launch, and smoke-test the flat Mclone Android APK on an
attached Quest headset. This validates the non-XR NativeActivity shell on Quest
before a standalone OpenXR target exists.

Options:
  --serial SERIAL     Use a specific attached headset serial.
  --skip-build        Reuse the existing APK.
  --screenshot PATH   Local screenshot output path.
  --log PATH          Local logcat output path.
  --timeout SECONDS   Device boot wait timeout.
  --smoke-seconds N   Seconds to wait after launch before validation.
  --asset-pack PATH   Local packed assets file to stage before launch.
  --skip-assets       Do not stage the packed Minecraft assets before launch.
  --touch-swipe SPEC  Inject a touch swipe before capture: x1,y1,x2,y2,duration_ms.
  -h, --help          Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --serial)
            SERIAL="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
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
        --)
            shift
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

cd "$REPO_ROOT"
ADB="$(mclone_android_tool adb platform-tools/adb)"

mclone_build_apk
"$ADB" start-server >/dev/null

if [[ -z "$SERIAL" ]]; then
    SERIAL="$(mclone_detect_quest_serial || true)"
fi

if [[ -z "$SERIAL" ]]; then
    mclone_report_no_quest_found
fi

mclone_wait_for_boot "$SERIAL" "$BOOT_TIMEOUT_SECONDS"
mclone_wake_headset_for_test "$SERIAL"
mclone_install_launch_smoke "$SERIAL" "$SCREENSHOT_PATH" "$LOG_PATH" "$SMOKE_SECONDS"
