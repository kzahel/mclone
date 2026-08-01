#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/validate-common.sh"

MCLONE_ANDROID_APP_ID="${MCLONE_ANDROID_APP_ID:-com.kzahel.mclone}"
MCLONE_ANDROID_ACTIVITY="${MCLONE_ANDROID_ACTIVITY:-com.kzahel.mclone.McloneActivity}"
MCLONE_ANDROID_REQUIRE_FOCUS="${MCLONE_ANDROID_REQUIRE_FOCUS:-0}"
APK_PATH="${MCLONE_ANDROID_APK:-$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk}"
SCREENSHOT_PATH="${MCLONE_ANDROID_SCREENSHOT:-/tmp/mclone-quest-flat.png}"
LOG_PATH="${MCLONE_ANDROID_LOGCAT:-/tmp/mclone-quest-flat-logcat.txt}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
SMOKE_SECONDS="${MCLONE_ANDROID_SMOKE_SECONDS:-60}"
STAGE_ASSETS="${MCLONE_ANDROID_STAGE_ASSETS:-1}"
MCLONE_ANDROID_STAGE_INTERNAL_ASSETS="${MCLONE_ANDROID_STAGE_INTERNAL_ASSETS:-1}"
SKIP_BUILD=0
SERIAL=""
MCLONE_QUEST_VALIDATOR_ARGS=("$@")

cleanup() {
    local status=$?
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
  --menu              Explicitly launch at the session-free title menu.
  --start-in-world true|false
                      Add an explicit client entry request to startup argv.
  --remote-addr ADDR  Add --remote-addr ADDR to mclone.startup.argv.
  --seed SEED         Add --seed SEED to startup argv.
  --chunk-x X         Add --chunk-x X to startup argv.
  --chunk-z Z         Add --chunk-z Z to startup argv.
  --render-distance N Add --render-distance N to startup argv.
  --movement-speed-multiplier N
                      Add --movement-speed-multiplier N to startup argv.
  --day-time T        Add --day-time T to startup argv.
  --freeze-time       Add --freeze-time to startup argv.
  --lighting true|false
                      Add --lighting VALUE to startup argv.
  --section-occlusion true|false
                      Add --section-occlusion VALUE to startup argv.
  --fullbright true|false
                      Add --fullbright VALUE to startup argv.
  --render-color-profile PROFILE
                      Add --render-color-profile PROFILE to startup argv.
  --screenshot-eye X,Y,Z
                      Add --screenshot-eye X,Y,Z to startup argv.
  --screenshot-target X,Y,Z
                      Add --screenshot-target X,Y,Z to startup argv.
  --touch-swipe SPEC  Inject a touch swipe before capture: x1,y1,x2,y2,duration_ms.
  -h, --help          Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --serial)
            mclone_require_arg "$1" "${2:-}"
            SERIAL="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --screenshot)
            mclone_require_arg "$1" "${2:-}"
            SCREENSHOT_PATH="$2"
            shift 2
            ;;
        --log)
            mclone_require_arg "$1" "${2:-}"
            LOG_PATH="$2"
            shift 2
            ;;
        --timeout)
            mclone_require_arg "$1" "${2:-}"
            BOOT_TIMEOUT_SECONDS="$2"
            shift 2
            ;;
        --smoke-seconds)
            mclone_require_arg "$1" "${2:-}"
            SMOKE_SECONDS="$2"
            shift 2
            ;;
        --asset-pack)
            mclone_require_arg "$1" "${2:-}"
            MCLONE_ANDROID_ASSET_PACK="$2"
            shift 2
            ;;
        --skip-assets)
            STAGE_ASSETS=0
            shift
            ;;
        --remote-addr)
            mclone_require_arg "$1" "${2:-}"
            MCLONE_ANDROID_REMOTE_ADDR="$2"
            shift 2
            ;;
        --menu)
            MCLONE_ANDROID_STARTUP_ARGV+=("$1")
            shift
            ;;
        --start-in-world)
            mclone_require_arg "$1" "${2:-}"
            MCLONE_ANDROID_STARTUP_ARGV+=("$1" "$2")
            shift 2
            ;;
        --seed|--chunk-x|--chunk-z|--render-distance|--movement-speed-multiplier|--day-time|--lighting|--section-occlusion|--fullbright|--render-color-profile|--screenshot-eye|--screenshot-target)
            mclone_require_arg "$1" "${2:-}"
            MCLONE_ANDROID_STARTUP_ARGV+=("$1" "$2")
            shift 2
            ;;
        --freeze-time)
            MCLONE_ANDROID_STARTUP_ARGV+=("$1")
            shift
            ;;
        --touch-swipe)
            mclone_require_arg "$1" "${2:-}"
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

if [[ "${QUEST_TESTBED_SESSION_ACTIVE:-0}" != "1" ]]; then
    QUEST_TESTBED="$(mclone_quest_testbed_cli)"
    quest_command=("$QUEST_TESTBED" --adb "$ADB")
    if [[ -n "$SERIAL" ]]; then
        quest_command+=(--serial "$SERIAL")
    fi
    exec "${quest_command[@]}" session \
        --stop-package "$MCLONE_ANDROID_APP_ID" \
        -- bash "$ANDROID_DIR/validate-quest-flat.sh" "${MCLONE_QUEST_VALIDATOR_ARGS[@]}"
fi

mclone_build_apk
"$ADB" start-server >/dev/null

if [[ -z "$SERIAL" ]]; then
    SERIAL="$(mclone_quest_serial "$ADB" "${QUEST_TESTBED_SERIAL:-}")"
fi

mclone_wait_for_boot "$SERIAL" "$BOOT_TIMEOUT_SECONDS"
mclone_install_launch_smoke "$SERIAL" "$SCREENSHOT_PATH" "$LOG_PATH" "$SMOKE_SECONDS"
