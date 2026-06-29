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
PERF_SUMMARY_PATH="${MCLONE_ANDROID_XR_PERF_SUMMARY:-/tmp/mclone-quest-openxr-perf-summary.txt}"
ACTIVITY_PATH="${MCLONE_ANDROID_XR_ACTIVITY_DUMP:-/tmp/mclone-quest-openxr-activity.txt}"
WAIT_SECONDS="${MCLONE_ANDROID_XR_WAIT_SECONDS:-20}"
WAIT_SECONDS_EXPLICIT="${MCLONE_ANDROID_XR_WAIT_SECONDS:+1}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
STAGE_ASSETS="${MCLONE_ANDROID_XR_STAGE_ASSETS:-1}"
ADB=""
SERIAL=""
LOGCAT_PID=""
SKIP_BUILD=0
SESSION_ONLY=0
START_VIEW_POSE="${MCLONE_ANDROID_XR_VIEW_POSE:-0}"
REMOTE_ADDR="${MCLONE_ANDROID_XR_REMOTE_ADDR:-}"
START_SERVER=0
SERVER_LISTEN="${MCLONE_ANDROID_XR_SERVER_LISTEN:-0.0.0.0:25565}"
SERVER_LISTEN_EXPLICIT="${MCLONE_ANDROID_XR_SERVER_LISTEN:+1}"
SERVER_LOG="${MCLONE_ANDROID_XR_SERVER_LOG:-/tmp/mclone-android-xr-dedicated-server.log}"
SERVER_SEED="${MCLONE_ANDROID_XR_SERVER_SEED:-12345}"
SERVER_SEED_EXPLICIT="${MCLONE_ANDROID_XR_SERVER_SEED:+1}"
SERVER_PID=""
ADB_REVERSE=0
ADB_REVERSE_PORT="${MCLONE_ANDROID_XR_ADB_REVERSE_PORT:-}"
ADB_REVERSE_INSTALLED=0
STARTUP_ARGV=()
SESSION_SMOKE="${MCLONE_ANDROID_XR_SESSION_SMOKE:-}"
PERF_SECONDS="${MCLONE_ANDROID_XR_PERF_SECONDS:-}"
PERF_FLIGHT="${MCLONE_ANDROID_XR_PERF_FLIGHT:-0}"
PERF_FLIGHT_SPEED="${MCLONE_ANDROID_XR_PERF_FLIGHT_SPEED:-}"
PERF_SETTLED_STATIONARY="${MCLONE_ANDROID_XR_PERF_SETTLED_STATIONARY:-0}"
PERF_FROZEN_RENDER="${MCLONE_ANDROID_XR_PERF_FROZEN_RENDER:-0}"
PERF_METRICS="${MCLONE_ANDROID_XR_PERF_METRICS:-0}"
MULTIVIEW_PROOF="${MCLONE_ANDROID_XR_MULTIVIEW_PROOF:-0}"
if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
    PERF_SETTLED_STATIONARY=1
fi

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
                     Add --remote-addr ADDR to mclone.startup.argv.
  --adb-reverse      Install adb reverse tcp:PORT tcp:PORT for USB validation.
                     If --remote-addr is omitted, defaults it to
                     127.0.0.1:PORT. With --start-server and no explicit
                     --server-listen, binds the server to 127.0.0.1:PORT.
  --adb-reverse-port PORT
                     Port for --adb-reverse. Defaults from --remote-addr,
                     --server-listen, or 25565.
  --start-server     Build and start local mclone-dedicated-server for this
                     validation. Requires --remote-addr with a headset-routable
                     HOST:PORT, or --adb-reverse.
  --server-listen ADDR
                     Address for --start-server to bind. Default: 0.0.0.0:25565.
  --server-seed SEED Seed for --start-server. Defaults to --seed or 12345.
  --server-log PATH  Local dedicated-server log path.
  --seed SEED        Add --seed SEED to mclone.startup.argv.
  --chunk-x X        Add --chunk-x X to startup argv.
  --chunk-z Z        Add --chunk-z Z to startup argv.
  --render-distance N
                     Add --render-distance N to startup argv.
  --movement-speed-multiplier N
                     Add --movement-speed-multiplier N to startup argv.
  --day-time T       Add --day-time T to startup argv.
  --freeze-time      Add --freeze-time to startup argv.
  --lighting true|false
                     Add --lighting VALUE to startup argv.
  --section-occlusion true|false
                     Add --section-occlusion VALUE to startup argv.
  --fullbright true|false
                     Add --fullbright VALUE to startup argv.
  --session-smoke MODE
                     Run a launch-scoped in-headset session replacement smoke.
                     MODE is new-world.
  --perf-seconds N  After the first submitted terrain frame, sample N seconds
                     of headset frame timing and wait for the
                     MCLONE_ANDROID_XR_PERF_SUMMARY marker block.
  --perf-flight      During --perf-seconds, fly forward in no-clip at about
                     walking speed instead of sampling a passive headset view.
  --perf-flight-speed N
                     Flight speed in blocks/second. Implies --perf-flight.
                     Default: 4.3.
  --perf-settled-stationary
                     During --perf-seconds, disable locomotion and start the
                     timed sample only after terrain generation, render
                     section compilation, and upload counters stay quiet.
  --perf-frozen-render
                     Like --perf-settled-stationary, but freezes runtime
                     polling, section sync, and uploads during the timed
                     sample so the cached mesh render cost can be isolated.
  --perf-metrics     Opt-in XR_META_performance_metrics probe. Enables the
                     Meta perf-metrics extension and, after a steady-state
                     warmup, logs MCLONE_ANDROID_XR_PERF_METRICS markers with
                     app/compositor GPU+CPU frametime and utilization so render
                     cost can be split into CPU, GPU, and compositor buckets.
                     Combine with a perf lane (e.g. --perf-frozen-render).
  --perf-summary PATH
                     Local file for the compact perf marker block. Default:
                     /tmp/mclone-quest-openxr-perf-summary.txt.
  --multiview-proof  Launch the minimal XR multiview proof path. The app
                     creates one two-layer color swapchain, renders a
                     view_index-colored multiview pass, presents layers 0/1,
                     and waits for MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY.
  -h, --help         Show this help.
USAGE
}

require_arg() {
    local option="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || mclone_die "$option requires a value"
}

tcp_port_from_addr() {
    local addr="$1"
    local port="${addr##*:}"
    [[ "$port" =~ ^[0-9]+$ ]] || return 1
    printf '%s' "$port"
}

validate_tcp_port() {
    local port="$1"
    [[ "$port" =~ ^[0-9]+$ ]] || mclone_die "invalid TCP port: $port"
    (( 10#$port >= 1 && 10#$port <= 65535 )) || mclone_die "TCP port out of range: $port"
}

validate_positive_integer() {
    local label="$1"
    local value="$2"
    [[ "$value" =~ ^[0-9]+$ ]] || mclone_die "$label must be a positive integer, got '$value'"
    (( 10#$value > 0 )) || mclone_die "$label must be greater than zero"
}

validate_positive_number() {
    local label="$1"
    local value="$2"
    [[ "$value" =~ ^[0-9]+([.][0-9]+)?$ ]] || mclone_die "$label must be a positive number, got '$value'"
    awk -v value="$value" 'BEGIN { exit !(value > 0) }' || mclone_die "$label must be greater than zero"
}

derive_adb_reverse_port() {
    if [[ -n "$ADB_REVERSE_PORT" ]]; then
        validate_tcp_port "$ADB_REVERSE_PORT"
        return
    fi
    if [[ -n "$REMOTE_ADDR" ]]; then
        ADB_REVERSE_PORT="$(tcp_port_from_addr "$REMOTE_ADDR" || true)"
    fi
    if [[ -z "$ADB_REVERSE_PORT" && -n "$SERVER_LISTEN" ]]; then
        ADB_REVERSE_PORT="$(tcp_port_from_addr "$SERVER_LISTEN" || true)"
    fi
    ADB_REVERSE_PORT="${ADB_REVERSE_PORT:-25565}"
    validate_tcp_port "$ADB_REVERSE_PORT"
}

configure_adb_reverse_defaults() {
    [[ "$ADB_REVERSE" == "1" ]] || return 0
    derive_adb_reverse_port
    if [[ -z "$REMOTE_ADDR" ]]; then
        REMOTE_ADDR="127.0.0.1:$ADB_REVERSE_PORT"
    fi
    if [[ "$START_SERVER" == "1" && -z "$SERVER_LISTEN_EXPLICIT" && "$REMOTE_ADDR" == "127.0.0.1:$ADB_REVERSE_PORT" ]]; then
        SERVER_LISTEN="127.0.0.1:$ADB_REVERSE_PORT"
    fi
}

install_adb_reverse() {
    [[ "$ADB_REVERSE" == "1" ]] || return 0
    "$ADB" -s "$SERIAL" reverse "tcp:$ADB_REVERSE_PORT" "tcp:$ADB_REVERSE_PORT" >/dev/null
    ADB_REVERSE_INSTALLED=1
    mclone_note "Installed adb reverse tcp:$ADB_REVERSE_PORT -> tcp:$ADB_REVERSE_PORT"
}

remove_adb_reverse() {
    if [[ "$ADB_REVERSE_INSTALLED" == "1" && -n "$ADB" && -n "$SERIAL" ]]; then
        "$ADB" -s "$SERIAL" reverse --remove "tcp:$ADB_REVERSE_PORT" >/dev/null 2>&1 || true
        ADB_REVERSE_INSTALLED=0
    fi
}

stop_logcat_capture() {
    if [[ -n "$LOGCAT_PID" ]]; then
        kill "$LOGCAT_PID" >/dev/null 2>&1 || true
        wait "$LOGCAT_PID" >/dev/null 2>&1 || true
        LOGCAT_PID=""
    fi
}

stop_dedicated_server() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" >/dev/null 2>&1 || true
        wait "$SERVER_PID" >/dev/null 2>&1 || true
        SERVER_PID=""
    fi
}

start_dedicated_server() {
    [[ -n "$REMOTE_ADDR" ]] || {
        mclone_die "--start-server requires --remote-addr HOST:PORT or --adb-reverse"
    }

    mclone_note "Building native dedicated server"
    cargo build --manifest-path native/Cargo.toml -p mclone-dedicated-server

    local server_bin="$REPO_ROOT/native/target/debug/mclone-dedicated-server"
    if [[ -x "$server_bin.exe" ]]; then
        server_bin="$server_bin.exe"
    fi
    [[ -x "$server_bin" ]] || mclone_die "dedicated server binary not found at $server_bin"

    mkdir -p "$(dirname "$SERVER_LOG")"
    : > "$SERVER_LOG"
    mclone_note "Starting dedicated server on $SERVER_LISTEN seed=$SERVER_SEED; headset connects to $REMOTE_ADDR"
    "$server_bin" --listen "$SERVER_LISTEN" --seed "$SERVER_SEED" > "$SERVER_LOG" 2>&1 &
    SERVER_PID="$!"

    local deadline=$((SECONDS + 30))
    while (( SECONDS < deadline )); do
        if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
            mclone_die "dedicated server exited before readiness; see $SERVER_LOG"
        fi
        if grep -F "mclone dedicated server listening" "$SERVER_LOG" >/dev/null 2>&1; then
            mclone_note "Dedicated server ready; log: $SERVER_LOG"
            return 0
        fi
        sleep 1
    done

    mclone_die "dedicated server did not report readiness within 30s; see $SERVER_LOG"
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
    stop_dedicated_server
    remove_adb_reverse
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
            WAIT_SECONDS_EXPLICIT=1
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
        --adb-reverse)
            ADB_REVERSE=1
            shift
            ;;
        --adb-reverse-port)
            require_arg "$1" "${2:-}"
            ADB_REVERSE=1
            ADB_REVERSE_PORT="$2"
            shift 2
            ;;
        --start-server)
            START_SERVER=1
            shift
            ;;
        --server-listen)
            require_arg "$1" "${2:-}"
            SERVER_LISTEN="$2"
            SERVER_LISTEN_EXPLICIT=1
            shift 2
            ;;
        --server-seed)
            require_arg "$1" "${2:-}"
            SERVER_SEED="$2"
            SERVER_SEED_EXPLICIT=1
            shift 2
            ;;
        --server-log)
            require_arg "$1" "${2:-}"
            SERVER_LOG="$2"
            shift 2
            ;;
        --seed)
            require_arg "$1" "${2:-}"
            STARTUP_ARGV+=("$1" "$2")
            if [[ -z "$SERVER_SEED_EXPLICIT" ]]; then
                SERVER_SEED="$2"
            fi
            shift 2
            ;;
        --chunk-x|--chunk-z|--render-distance|--movement-speed-multiplier|--day-time|--lighting|--section-occlusion|--fullbright)
            require_arg "$1" "${2:-}"
            STARTUP_ARGV+=("$1" "$2")
            shift 2
            ;;
        --freeze-time)
            STARTUP_ARGV+=("$1")
            shift
            ;;
        --session-smoke)
            require_arg "$1" "${2:-}"
            SESSION_SMOKE="$2"
            shift 2
            ;;
        --perf-seconds)
            require_arg "$1" "${2:-}"
            PERF_SECONDS="$2"
            shift 2
            ;;
        --perf-flight)
            PERF_FLIGHT=1
            shift
            ;;
        --perf-flight-speed)
            require_arg "$1" "${2:-}"
            PERF_FLIGHT=1
            PERF_FLIGHT_SPEED="$2"
            shift 2
            ;;
        --perf-settled-stationary)
            PERF_SETTLED_STATIONARY=1
            shift
            ;;
        --perf-frozen-render)
            PERF_SETTLED_STATIONARY=1
            PERF_FROZEN_RENDER=1
            shift
            ;;
        --perf-metrics)
            PERF_METRICS=1
            shift
            ;;
        --perf-summary)
            require_arg "$1" "${2:-}"
            PERF_SUMMARY_PATH="$2"
            shift 2
            ;;
        --multiview-proof)
            MULTIVIEW_PROOF=1
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
case "$SESSION_SMOKE" in
    ""|new-world)
        ;;
    *)
        mclone_die "unsupported --session-smoke '$SESSION_SMOKE'; expected new-world"
        ;;
esac
if [[ -n "$SESSION_SMOKE" && "$SESSION_ONLY" == "1" ]]; then
    mclone_die "--session-smoke requires submitted-frame validation; remove --session-only"
fi
if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--multiview-proof has its own proof marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--multiview-proof cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--multiview-proof cannot be combined with performance probes"
    fi
fi
if [[ -n "$PERF_SECONDS" ]]; then
    validate_positive_integer "--perf-seconds" "$PERF_SECONDS"
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--perf-seconds requires submitted-frame validation; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--perf-seconds cannot be combined with --session-smoke in the first perf probe"
    fi
    if [[ -z "$WAIT_SECONDS_EXPLICIT" ]]; then
        if [[ "$PERF_SETTLED_STATIONARY" == "1" ]]; then
            WAIT_SECONDS=$((10#$PERF_SECONDS + 180))
        else
            WAIT_SECONDS=$((10#$PERF_SECONDS + 30))
        fi
    fi
fi
if [[ "$PERF_FLIGHT" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-flight requires --perf-seconds"
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-settled-stationary requires --perf-seconds"
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-settled-stationary cannot be combined with --perf-flight"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-frozen-render requires --perf-seconds"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-frozen-render cannot be combined with --perf-flight"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && "$START_VIEW_POSE" == "0" ]]; then
    mclone_die "--perf-frozen-render requires --view-pose X,Y,Z,YAW_DEGREES"
fi
if [[ -n "$PERF_FLIGHT_SPEED" ]]; then
    validate_positive_number "--perf-flight-speed" "$PERF_FLIGHT_SPEED"
fi

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

configure_adb_reverse_defaults
install_adb_reverse

if [[ -n "$START_VIEW_POSE" ]]; then
    mclone_xr_set_startup_property "$SERIAL" "$VIEW_POSE_PROPERTY" "$START_VIEW_POSE" >/dev/null 2>&1 || true
    mclone_note "Configured startup view pose via $VIEW_POSE_PROPERTY=$START_VIEW_POSE"
fi
if [[ -n "$REMOTE_ADDR" ]]; then
    STARTUP_ARGV+=(--remote-addr "$REMOTE_ADDR")
fi
if [[ -n "$SESSION_SMOKE" ]]; then
    STARTUP_ARGV+=(--session-smoke "$SESSION_SMOKE")
fi
if [[ -n "$PERF_SECONDS" ]]; then
    STARTUP_ARGV+=(--perf-seconds "$PERF_SECONDS")
fi
if [[ "$PERF_FLIGHT" == "1" ]]; then
    STARTUP_ARGV+=(--perf-flight)
    if [[ -n "$PERF_FLIGHT_SPEED" ]]; then
        STARTUP_ARGV+=(--perf-flight-speed "$PERF_FLIGHT_SPEED")
    fi
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" ]]; then
    STARTUP_ARGV+=(--perf-settled-stationary)
fi
if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
    STARTUP_ARGV+=(--perf-frozen-render)
fi
if [[ "$PERF_METRICS" == "1" ]]; then
    STARTUP_ARGV+=(--perf-metrics)
fi
if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
    STARTUP_ARGV+=(--multiview-proof)
fi
mclone_xr_clear_startup_property "$SERIAL" "$REMOTE_ADDR_PROPERTY" >/dev/null 2>&1 || true
mclone_note "Cleared legacy Android XR remote dedicated property $REMOTE_ADDR_PROPERTY"

STARTUP_ARGV_JSON=""
if ((${#STARTUP_ARGV[@]} > 0)); then
    STARTUP_ARGV_JSON="$(mclone_xr_startup_argv_json "${STARTUP_ARGV[@]}")"
    mclone_note "Startup argv intent extra: $STARTUP_ARGV_JSON"
fi

start_logcat_capture

if [[ "$START_SERVER" == "1" ]]; then
    start_dedicated_server
fi

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
    if [[ "$MULTIVIEW_PROOF" == "1" ]] && grep -F "MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ -n "$PERF_SECONDS" ]] && grep -F "MCLONE_ANDROID_XR_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ -n "$SESSION_SMOKE" ]] && grep -F "MCLONE_ANDROID_XR_REPLACEMENT_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$MULTIVIEW_PROOF" != "1" && -z "$SESSION_SMOKE" && -z "$PERF_SECONDS" ]] && grep -F "MCLONE_ANDROID_XR_READY" "$LOG_PATH" >/dev/null 2>&1; then
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
    if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "Android XR multiview proof marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "Android XR session-ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ -n "$PERF_SECONDS" ]]; then
        mclone_die "Android XR perf summary marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "Android XR replacement-ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    else
        mclone_die "Android XR submitted-frame ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    fi
fi

if ! grep -F "MCLONE_ANDROID_XR_ASSETS_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR assets-ready marker was not seen; see $LOG_PATH"
fi
if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR session-ready marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR multiview proof marker was not seen; see $LOG_PATH"
    fi
elif ! grep -F "MCLONE_ANDROID_XR_CONTROLLERS_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR controllers-ready marker was not seen; see $LOG_PATH"
fi
if [[ "$MULTIVIEW_PROOF" != "1" && "$SESSION_ONLY" != "1" ]] && ! grep -F "MCLONE_ANDROID_XR_TERRAIN_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR terrain-ready marker was not seen; see $LOG_PATH"
fi
if [[ -n "$SESSION_SMOKE" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_REPLACEMENT_STARTED" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR replacement-started marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_REPLACEMENT_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR replacement-ready marker was not seen; see $LOG_PATH"
    fi
fi
if [[ -n "$PERF_SECONDS" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_PERF_START" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR perf-start marker was not seen; see $LOG_PATH"
    fi
    for marker in \
        MCLONE_ANDROID_XR_PERF_SUMMARY \
        MCLONE_ANDROID_XR_PERF_STAGES \
        MCLONE_ANDROID_XR_PERF_TERRAIN \
        MCLONE_ANDROID_XR_PERF_TERRAIN_PREP \
        MCLONE_ANDROID_XR_PERF_UPLOAD_MAX \
        MCLONE_ANDROID_XR_PERF_RUNTIME_MAX \
        MCLONE_ANDROID_XR_PERF_QUEUE_MAX \
        MCLONE_ANDROID_XR_PERF_COMPILE_MAX \
        MCLONE_ANDROID_XR_PERF_UPLOAD_LAST \
        MCLONE_ANDROID_XR_PERF_DRAW
    do
        if ! grep -F "$marker" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf marker $marker was not seen; see $LOG_PATH"
        fi
    done
    if [[ "$PERF_FLIGHT" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*mode=flight" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf flight start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*mode=flight .*flight_distance_blocks=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf flight summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ "$PERF_SETTLED_STATIONARY" == "1" ]]; then
        perf_stationary_mode="stationary-settled"
        if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
            perf_stationary_mode="stationary-frozen-render"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SETTLED .*mode=${perf_stationary_mode}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*mode=${perf_stationary_mode} .*settle_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*mode=${perf_stationary_mode} .*settle_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled summary marker was not seen; see $LOG_PATH"
        fi
    fi
    mkdir -p "$(dirname "$PERF_SUMMARY_PATH")"
    grep -E "MCLONE_ANDROID_XR_PERF_(SUMMARY|STAGES|TERRAIN|UPLOAD_MAX|RUNTIME_MAX|QUEUE_MAX|COMPILE_MAX|UPLOAD_LAST|DRAW)" "$LOG_PATH" \
        | tail -n 10 > "$PERF_SUMMARY_PATH"
    mclone_note "Perf summary: $PERF_SUMMARY_PATH"
fi

pid="$("$ADB" -s "$SERIAL" shell pidof "$MCLONE_ANDROID_XR_APP_ID" 2>/dev/null | tr -d '\r' || true)"
if [[ -z "$pid" ]]; then
    mclone_die "Android XR process exited after launch; see $LOG_PATH"
fi
mclone_note "$MCLONE_ANDROID_XR_APP_ID pid: $pid"
mclone_note "Logcat: $LOG_PATH"
mclone_note "Android XR package launch validation passed"
