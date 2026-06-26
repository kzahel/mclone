#!/usr/bin/env bash
set -euo pipefail

ANDROID_XR_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_XR_DIR/.." && pwd)"
ANDROID_DIR="$REPO_ROOT/android"
source "$ANDROID_DIR/validate-common.sh"
source "$ANDROID_XR_DIR/startup-properties.sh"

MCLONE_ANDROID_XR_APP_ID="${MCLONE_ANDROID_XR_APP_ID:-com.kzahel.mclone.xr}"
MCLONE_ANDROID_XR_ACTIVITY="${MCLONE_ANDROID_XR_ACTIVITY:-com.kzahel.mclone.xr.McloneXrActivity}"
APK_PATH="${MCLONE_ANDROID_XR_APK:-}"
BUILD_TYPE="${MCLONE_ANDROID_XR_BUILD_TYPE:-release}"
LOG_PATH="${MCLONE_ANDROID_XR_LOGCAT:-/tmp/mclone-quest-openxr-logcat.txt}"
ACTIVITY_PATH="${MCLONE_ANDROID_XR_ACTIVITY_DUMP:-/tmp/mclone-quest-openxr-activity.txt}"
WAIT_SECONDS="${MCLONE_ANDROID_XR_WAIT_SECONDS:-20}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
STAGE_ASSETS="${MCLONE_ANDROID_XR_STAGE_ASSETS:-1}"
SERIAL=""
LOGCAT_PID=""
SKIP_BUILD=0
SESSION_ONLY=0
START_VIEW_POSE="${MCLONE_ANDROID_XR_VIEW_POSE:-0}"
REMOTE_ADDR="${MCLONE_ANDROID_XR_REMOTE_ADDR:-}"
STARTUP_ARGV=()

usage() {
    cat <<'USAGE'
Usage: android-xr/validate-quest-openxr.sh [options]

Build, install, launch, and smoke-test the standalone Quest Android XR package.
This validator waits for MCLONE_ANDROID_XR_READY, which is logged only after
the app loads runtime assets and submits the first Android OpenXR terrain frame.

Options:
  --release          Build and validate the release APK. This is the default.
  --debug            Build and validate the debug APK.
  --serial SERIAL    Use a specific attached headset serial.
  --skip-build       Reuse the existing APK.
  --log PATH         Local logcat output path.
  --activity-log PATH
                     Local activity-manager dump path on launch failure.
  --wait-seconds N   Seconds to wait for ready/failure log markers.
  --asset-pack PATH   Local packed assets file to stage before launch.
  --skip-assets       Do not stage the packed Minecraft assets before launch.
  --session-only     Accept MCLONE_ANDROID_XR_SESSION_READY instead of waiting
                     for the first submitted stereo frame.
  --view-pose X,Y,Z,YAW_DEGREES
                     Set debug.mclone.xr_view_pose before launch.
  --remote-addr ADDR
                     Set debug.mclone.remote_addr before launch.
  --seed SEED        Add --seed SEED to mclone.startup.argv.
  --chunk-x X        Add --chunk-x X to startup argv.
  --chunk-z Z        Add --chunk-z Z to startup argv.
  --render-distance N
                     Add --render-distance N to startup argv.
  --day-time T       Add --day-time T to startup argv.
  --freeze-time      Add --freeze-time to startup argv.
  -h, --help         Show this help.
USAGE
}

require_arg() {
    local option="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || mclone_die "$option requires a value"
}

stop_logcat_capture() {
    if [[ -n "$LOGCAT_PID" ]]; then
        kill "$LOGCAT_PID" >/dev/null 2>&1 || true
        wait "$LOGCAT_PID" >/dev/null 2>&1 || true
        LOGCAT_PID=""
    fi
}

start_logcat_capture() {
    mkdir -p "$(dirname "$LOG_PATH")"
    : > "$LOG_PATH"
    "$ADB" -s "$SERIAL" logcat -v time > "$LOG_PATH" 2>/dev/null &
    LOGCAT_PID="$!"
}

cleanup() {
    local status=$?
    stop_logcat_capture
    if [[ -n "$SERIAL" ]]; then
        mclone_xr_clear_all_startup_properties "$SERIAL" >/dev/null 2>&1 || true
        mclone_restore_headset_after_test "$SERIAL" "$MCLONE_ANDROID_XR_APP_ID"
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            BUILD_TYPE=release
            shift
            ;;
        --debug)
            BUILD_TYPE=debug
            shift
            ;;
        --serial)
            require_arg "$1" "${2:-}"
            SERIAL="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --log)
            require_arg "$1" "${2:-}"
            LOG_PATH="$2"
            shift 2
            ;;
        --activity-log)
            require_arg "$1" "${2:-}"
            ACTIVITY_PATH="$2"
            shift 2
            ;;
        --wait-seconds)
            require_arg "$1" "${2:-}"
            WAIT_SECONDS="$2"
            shift 2
            ;;
        --asset-pack)
            require_arg "$1" "${2:-}"
            MCLONE_ANDROID_ASSET_PACK="$2"
            shift 2
            ;;
        --skip-assets)
            STAGE_ASSETS=0
            shift
            ;;
        --session-only)
            SESSION_ONLY=1
            shift
            ;;
        --view-pose)
            require_arg "$1" "${2:-}"
            START_VIEW_POSE="$2"
            shift 2
            ;;
        --remote-addr)
            require_arg "$1" "${2:-}"
            REMOTE_ADDR="$2"
            shift 2
            ;;
        --seed|--chunk-x|--chunk-z|--render-distance|--day-time)
            require_arg "$1" "${2:-}"
            STARTUP_ARGV+=("$1" "$2")
            shift 2
            ;;
        --freeze-time)
            STARTUP_ARGV+=("$1")
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

case "$BUILD_TYPE" in
    release|Release|RELEASE)
        BUILD_TYPE=release
        DEFAULT_APK_PATH="$ANDROID_XR_DIR/app/build/outputs/apk/release/app-release.apk"
        ;;
    debug|Debug|DEBUG)
        BUILD_TYPE=debug
        DEFAULT_APK_PATH="$ANDROID_XR_DIR/app/build/outputs/apk/debug/app-debug.apk"
        ;;
    *)
        mclone_die "MCLONE_ANDROID_XR_BUILD_TYPE must be 'release' or 'debug'"
        ;;
esac
APK_PATH="${APK_PATH:-$DEFAULT_APK_PATH}"

cd "$REPO_ROOT"
ADB="$(mclone_android_tool adb platform-tools/adb)"

if [[ "$SKIP_BUILD" == "1" ]]; then
    mclone_note "Skipping Android XR APK build"
else
    mclone_note "Building Android XR APK ($BUILD_TYPE)"
    bash "$ANDROID_XR_DIR/build-apk.sh" "--$BUILD_TYPE"
fi
[[ -f "$APK_PATH" ]] || mclone_die "APK not found at $APK_PATH"

"$ADB" start-server >/dev/null
if [[ -z "$SERIAL" ]]; then
    SERIAL="$(mclone_detect_quest_serial || true)"
fi
if [[ -z "$SERIAL" ]]; then
    mclone_report_no_quest_found
fi

mclone_wait_for_boot "$SERIAL" "$BOOT_TIMEOUT_SECONDS"
mclone_note "Using $(mclone_device_summary "$SERIAL")"
mclone_wake_headset_for_test "$SERIAL"

mclone_note "Installing $APK_PATH"
"$ADB" -s "$SERIAL" install -r "$APK_PATH"
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" com.oculus.permission.USE_SCENE >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" horizonos.permission.USE_SCENE >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" android.permission.POST_NOTIFICATIONS >/dev/null 2>&1 || true
if [[ "$STAGE_ASSETS" == "1" ]]; then
    MCLONE_ANDROID_APP_ID="$MCLONE_ANDROID_XR_APP_ID" mclone_stage_asset_pack "$SERIAL"
else
    mclone_note "Skipping Android XR asset-pack staging"
fi
"$ADB" -s "$SERIAL" shell am force-stop "$MCLONE_ANDROID_XR_APP_ID" >/dev/null 2>&1 || true
mclone_dismiss_vr_system_dialogs "$SERIAL"
"$ADB" -s "$SERIAL" logcat -c || true

if [[ -n "$START_VIEW_POSE" ]]; then
    mclone_xr_set_startup_property "$SERIAL" "$VIEW_POSE_PROPERTY" "$START_VIEW_POSE" >/dev/null 2>&1 || true
    mclone_note "Configured startup view pose via $VIEW_POSE_PROPERTY=$START_VIEW_POSE"
fi
if [[ -n "$REMOTE_ADDR" ]]; then
    mclone_xr_set_startup_property "$SERIAL" "$REMOTE_ADDR_PROPERTY" "$REMOTE_ADDR" >/dev/null 2>&1 || true
    mclone_note "Configured Android XR remote dedicated address via $REMOTE_ADDR_PROPERTY=$REMOTE_ADDR"
else
    mclone_xr_clear_startup_property "$SERIAL" "$REMOTE_ADDR_PROPERTY" >/dev/null 2>&1 || true
    mclone_note "Cleared Android XR remote dedicated address via $REMOTE_ADDR_PROPERTY"
fi

STARTUP_ARGV_JSON=""
if ((${#STARTUP_ARGV[@]} > 0)); then
    STARTUP_ARGV_JSON="$(mclone_xr_startup_argv_json "${STARTUP_ARGV[@]}")"
    mclone_note "Startup argv intent extra: $STARTUP_ARGV_JSON"
fi

start_logcat_capture

mclone_note "Launching $MCLONE_ANDROID_XR_APP_ID/$MCLONE_ANDROID_XR_ACTIVITY"
launch_component="$MCLONE_ANDROID_XR_APP_ID/$MCLONE_ANDROID_XR_ACTIVITY"
remote_launch_command="am start"
remote_launch_command+=" -a android.intent.action.MAIN"
remote_launch_command+=" -c com.oculus.intent.category.VR"
remote_launch_command+=" -n $(mclone_xr_shell_quote "$launch_component")"
if [[ -n "$STARTUP_ARGV_JSON" ]]; then
    remote_launch_command+=" --es $(mclone_xr_shell_quote "$STARTUP_ARGV_INTENT_EXTRA")"
    remote_launch_command+=" $(mclone_xr_shell_quote "$STARTUP_ARGV_JSON")"
fi
launch_output="$("$ADB" -s "$SERIAL" shell "$remote_launch_command" 2>&1 | tr -d '\r')"
echo "$launch_output"
if ! printf '%s\n' "$launch_output" | grep -E "Starting: Intent|Warning: Activity not started" >/dev/null; then
    mclone_die "activity launch command did not start an intent"
fi

deadline=$((SECONDS + WAIT_SECONDS))
success=0
failure=0
while (( SECONDS < deadline )); do
    if grep -F "MCLONE_ANDROID_XR_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$SESSION_ONLY" == "1" ]] && grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if grep -E "MCLONE_ANDROID_XR_FAILURE|FATAL EXCEPTION|Fatal signal|SIGSEGV|thread .* panicked|panicked at" "$LOG_PATH" >/dev/null 2>&1; then
        failure=1
        break
    fi
    sleep 1
done

stop_logcat_capture
if [[ "$failure" == "1" ]]; then
    mclone_die "Android XR failure or fatal logcat entry found in $LOG_PATH"
fi
if [[ "$success" != "1" ]]; then
    mkdir -p "$(dirname "$ACTIVITY_PATH")"
    "$ADB" -s "$SERIAL" shell dumpsys activity activities > "$ACTIVITY_PATH" 2>/dev/null || true
    if grep -F "LaunchCheckControllerRequiredDialogActivity" "$ACTIVITY_PATH" >/dev/null 2>&1; then
        mclone_die "OpenXR launch was blocked by the Oculus controller-required launch check; activity dump: $ACTIVITY_PATH; logcat: $LOG_PATH"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "Android XR session-ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    else
        mclone_die "Android XR submitted-frame ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    fi
fi

if ! grep -F "MCLONE_ANDROID_XR_ASSETS_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR assets-ready marker was not seen; see $LOG_PATH"
fi
if ! grep -F "MCLONE_ANDROID_XR_CONTROLLERS_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR controllers-ready marker was not seen; see $LOG_PATH"
fi
if [[ "$SESSION_ONLY" != "1" ]] && ! grep -F "MCLONE_ANDROID_XR_TERRAIN_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR terrain-ready marker was not seen; see $LOG_PATH"
fi

pid="$("$ADB" -s "$SERIAL" shell pidof "$MCLONE_ANDROID_XR_APP_ID" 2>/dev/null | tr -d '\r' || true)"
if [[ -z "$pid" ]]; then
    mclone_die "Android XR process exited after launch; see $LOG_PATH"
fi
mclone_note "$MCLONE_ANDROID_XR_APP_ID pid: $pid"
mclone_note "Logcat: $LOG_PATH"
mclone_note "Android XR package launch validation passed"
