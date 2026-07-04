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
BUILD_ABIS="${MCLONE_ANDROID_ABIS:-}"
SKIP_BUILD=0
KEEP_EMULATOR=0
HEADLESS=1
SERIAL=""
STARTED_EMULATOR=0
EMULATOR_PID=""
EXPLICIT_ABIS=()

usage() {
    cat <<'USAGE'
Usage: android/validate-avd.sh [options]

Build, install, launch, and smoke-test the flat Mclone Android APK on an AVD.

Options:
  --avd NAME          AVD name to boot when no emulator is already online.
  --serial SERIAL     Use an already-running emulator/device serial.
  --skip-build        Reuse the existing APK.
  --abi ABI           Build for one Android ABI. May be repeated.
                      Defaults to the attached device or AVD ABI when
                      discoverable, otherwise x86_64.
  --abis LIST         Build for comma- or space-separated Android ABIs.
  --keep-emulator     Leave an emulator started by this script running.
  --window            Show the emulator window instead of using -no-window.
  --screenshot PATH   Local screenshot output path.
  --log PATH          Local logcat output path.
  --timeout SECONDS   Boot/device wait timeout.
  --smoke-seconds N   Seconds to wait after launch before validation.
  --asset-pack PATH   Local packed assets file to stage before launch.
  --skip-assets       Do not stage the packed Minecraft assets before launch.
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
  --session-smoke MODE
                      Inject a shared UI session flow before capture.
                      MODE is new-world or join-remote.
  -h, --help          Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --avd)
            mclone_require_arg "$1" "${2:-}"
            AVD_NAME="$2"
            shift 2
            ;;
        --serial)
            mclone_require_arg "$1" "${2:-}"
            SERIAL="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --abi)
            mclone_require_arg "$1" "${2:-}"
            EXPLICIT_ABIS+=("$2")
            shift 2
            ;;
        --abis)
            mclone_require_arg "$1" "${2:-}"
            BUILD_ABIS="$2"
            shift 2
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
        --session-smoke)
            mclone_require_arg "$1" "${2:-}"
            MCLONE_ANDROID_SESSION_SMOKE="$2"
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

if [[ "${#EXPLICIT_ABIS[@]}" -gt 0 ]]; then
    BUILD_ABIS="${EXPLICIT_ABIS[*]}"
fi

infer_avd_config_abi() {
    local avd_name="$1"
    local avd_home="${ANDROID_AVD_HOME:-$HOME/.android/avd}"
    local config="$avd_home/$avd_name.avd/config.ini"
    local abi

    [[ -f "$config" ]] || return 1
    abi="$(sed -n 's/^[[:space:]]*abi\.type[[:space:]]*=[[:space:]]*//p' "$config" | head -1 | tr -d '\r')"
    case "$abi" in
        arm64-v8a|x86_64)
            printf '%s\n' "$abi"
            return 0
            ;;
    esac
    return 1
}

infer_device_abi() {
    local serial="$1"
    local abi

    [[ -n "$serial" ]] || return 1
    abi="$("$ADB" -s "$serial" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r' || true)"
    case "$abi" in
        arm64-v8a|x86_64)
            printf '%s\n' "$abi"
            return 0
            ;;
    esac
    return 1
}

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

if [[ -z "$BUILD_ABIS" ]]; then
    if [[ -n "$SERIAL" ]] && BUILD_ABIS="$(infer_device_abi "$SERIAL")"; then
        :
    elif BUILD_ABIS="$(infer_avd_config_abi "$AVD_NAME")"; then
        :
    else
        BUILD_ABIS="x86_64"
    fi
    mclone_note "Inferred Android ABI(s) for AVD validation: $BUILD_ABIS"
fi
export MCLONE_ANDROID_ABIS="$BUILD_ABIS"

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
