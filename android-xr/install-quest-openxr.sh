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
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
SERIAL=""
SKIP_BUILD=0
LAUNCH_APP=0
START_VIEW_POSE="${MCLONE_ANDROID_XR_VIEW_POSE:-}"
STARTUP_ARGV=()

usage() {
    cat <<'USAGE'
Usage: android-xr/install-quest-openxr.sh [options]

Build and install the standalone Quest Android XR APK on an attached Quest.
By default this installs only. Use --launch to start the VR activity.

Options:
  --release        Build and install the release APK. This is the default.
  --debug          Build and install the debug APK.
  --serial SERIAL  Use a specific attached headset serial.
  --skip-build     Reuse the existing APK.
  --launch         Launch Mclone XR after installing.
  --view-pose X,Y,Z,YAW_DEGREES
                  Set debug.mclone.xr_view_pose before launch.
  --seed SEED      Add --seed SEED to the launch-scoped mclone.startup.argv.
  --chunk-x X      Add --chunk-x X to startup argv.
  --chunk-z Z      Add --chunk-z Z to startup argv.
  --render-distance N
                  Add --render-distance N to startup argv.
  --day-time T     Add --day-time T to startup argv.
  --freeze-time    Add --freeze-time to startup argv.
  -h, --help       Show this help.
USAGE
}

require_arg() {
    local option="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || mclone_die "$option requires a value"
}

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
        --launch)
            LAUNCH_APP=1
            shift
            ;;
        --view-pose)
            require_arg "$1" "${2:-}"
            START_VIEW_POSE="$2"
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
mclone_note "Installing $APK_PATH"
"$ADB" -s "$SERIAL" install -r "$APK_PATH"
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" com.oculus.permission.USE_SCENE >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" horizonos.permission.USE_SCENE >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell pm grant "$MCLONE_ANDROID_XR_APP_ID" android.permission.POST_NOTIFICATIONS >/dev/null 2>&1 || true

if [[ -n "$START_VIEW_POSE" ]]; then
    mclone_xr_set_startup_property "$SERIAL" "$VIEW_POSE_PROPERTY" "$START_VIEW_POSE" >/dev/null 2>&1 || true
    mclone_note "Configured startup view pose via $VIEW_POSE_PROPERTY=$START_VIEW_POSE"
fi

STARTUP_ARGV_JSON=""
if ((${#STARTUP_ARGV[@]} > 0)); then
    STARTUP_ARGV_JSON="$(mclone_xr_startup_argv_json "${STARTUP_ARGV[@]}")"
    mclone_note "Startup argv intent extra: $STARTUP_ARGV_JSON"
fi

launch_component="$MCLONE_ANDROID_XR_APP_ID/$MCLONE_ANDROID_XR_ACTIVITY"
remote_launch_command="am start"
remote_launch_command+=" -a android.intent.action.MAIN"
remote_launch_command+=" -c com.oculus.intent.category.VR"
remote_launch_command+=" -n $(mclone_xr_shell_quote "$launch_component")"
if [[ -n "$STARTUP_ARGV_JSON" ]]; then
    remote_launch_command+=" --es $(mclone_xr_shell_quote "$STARTUP_ARGV_INTENT_EXTRA")"
    remote_launch_command+=" $(mclone_xr_shell_quote "$STARTUP_ARGV_JSON")"
fi
launch_command=("$ADB" -s "$SERIAL" shell "$remote_launch_command")

if [[ "$LAUNCH_APP" == "1" ]]; then
    mclone_note "Force-stopping any running $MCLONE_ANDROID_XR_APP_ID before launch"
    "$ADB" -s "$SERIAL" shell am force-stop "$MCLONE_ANDROID_XR_APP_ID" >/dev/null 2>&1 || true
    mclone_note "Launching $MCLONE_ANDROID_XR_APP_ID/$MCLONE_ANDROID_XR_ACTIVITY"
    launch_output="$("${launch_command[@]}" 2>&1 | tr -d '\r')"
    echo "$launch_output"
    if ! printf '%s\n' "$launch_output" | grep -E "Starting: Intent|Warning: Activity not started" >/dev/null; then
        mclone_die "activity launch command did not start an intent"
    fi
else
    mclone_note "Launch from the headset as Mclone XR, or run:"
    printf '%q ' "${launch_command[@]}"
    printf '\n'
fi

mclone_note "Installed $MCLONE_ANDROID_XR_APP_ID"
