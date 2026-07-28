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
XR_UNDERWATER_MODE="${MCLONE_ANDROID_XR_UNDERWATER_MODE:-}"
XR_DEBUG_UI="${MCLONE_ANDROID_XR_DEBUG_UI:-}"
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
PERF_SETTLED_ORBIT="${MCLONE_ANDROID_XR_PERF_SETTLED_ORBIT:-0}"
PERF_ORBIT_SPEED="${MCLONE_ANDROID_XR_PERF_ORBIT_SPEED:-}"
PERF_CHUNK_VIEW_CHURN="${MCLONE_ANDROID_XR_PERF_CHUNK_VIEW_CHURN:-0}"
PERF_CHURN_INTERVAL_SECONDS="${MCLONE_ANDROID_XR_PERF_CHURN_INTERVAL_SECONDS:-}"
PERF_CHURN_OFFSET_CHUNKS="${MCLONE_ANDROID_XR_PERF_CHURN_OFFSET_CHUNKS:-}"
PERF_SETTLED_STATIONARY="${MCLONE_ANDROID_XR_PERF_SETTLED_STATIONARY:-0}"
PERF_FROZEN_RENDER="${MCLONE_ANDROID_XR_PERF_FROZEN_RENDER:-0}"
PERF_METRICS="${MCLONE_ANDROID_XR_PERF_METRICS:-0}"
PERF_METRICS_PERIODIC="${MCLONE_ANDROID_XR_PERF_METRICS_PERIODIC:-0}"
PERF_DETAIL="${MCLONE_ANDROID_XR_PERF_DETAIL:-full}"
FRAME_ACCOUNTING="${MCLONE_ANDROID_XR_FRAME_ACCOUNTING:-}"
MULTIVIEW_PROOF="${MCLONE_ANDROID_XR_MULTIVIEW_PROOF:-0}"
TERRAIN_MULTIVIEW_PROOF="${MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF:-0}"
TERRAIN_MULTIVIEW_PERF="${MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF:-0}"
SKY_TERRAIN_MULTIVIEW_PERF="${MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF:-0}"
SKY_TERRAIN_ACTORS_MULTIVIEW_PERF="${MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF:-0}"
XR_SKIP_ACTORS="${MCLONE_ANDROID_XR_SKIP_ACTORS:-0}"
XR_FULL_FRAME_MULTIVIEW="${MCLONE_ANDROID_XR_FULL_FRAME_MULTIVIEW:-0}"
XR_FRAME_OVERLAP="${MCLONE_ANDROID_XR_FRAME_OVERLAP:-0}"
XR_FRAME_SERIAL="${MCLONE_ANDROID_XR_FRAME_SERIAL:-0}"
XR_OVERLAP_EYE_SUBMITS="${MCLONE_ANDROID_XR_OVERLAP_EYE_SUBMITS:-0}"
XR_OVERLAP_RUNTIME_PREFETCH="${MCLONE_ANDROID_XR_OVERLAP_RUNTIME_PREFETCH:-0}"
XR_RENDER_SECTION_UPLOAD_BUDGET="${MCLONE_ANDROID_XR_RENDER_SECTION_UPLOAD_BUDGET:-}"
XR_RENDER_SECTION_ACCEPT_BUDGET="${MCLONE_ANDROID_XR_RENDER_SECTION_ACCEPT_BUDGET:-}"
XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET="${MCLONE_ANDROID_XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET:-}"
XR_FOVEATION="${MCLONE_ANDROID_XR_FOVEATION:-off}"
XR_RENDER_SCALE="${MCLONE_ANDROID_XR_RENDER_SCALE:-}"
XR_DISPLAY_REFRESH_RATE="${MCLONE_ANDROID_XR_DISPLAY_REFRESH_RATE:-}"
if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
    PERF_SETTLED_STATIONARY=1
fi
if [[ "$PERF_METRICS_PERIODIC" == "1" ]]; then
    PERF_METRICS=1
fi
if [[ -n "$PERF_CHURN_INTERVAL_SECONDS" || -n "$PERF_CHURN_OFFSET_CHUNKS" ]]; then
    PERF_CHUNK_VIEW_CHURN=1
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
  --xr-underwater-mode midpoint|per-eye
                     Add --xr-underwater-mode MODE to startup argv.
  --xr-debug-ui none|pause|controls
                     Hold an XR debug UI panel open after startup for headset
                     UI validation. Default: none.
  --remote-addr ADDR
                     Add --remote-addr ADDR to mclone.startup.argv.
  --world-dir PATH   Add --world-dir PATH to mclone.startup.argv.
  --world-root PATH  Add --world-root PATH to mclone.startup.argv.
  --transient        Add --transient to mclone.startup.argv.
  --app-arg TOKEN    Add one raw token to mclone.startup.argv. Repeat for
                     flags and values that are parsed by the app.
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
  --render-compile-workers N
                     Add --render-compile-workers N to startup argv.
  --render-compile-max-pending-jobs N
                     Add --render-compile-max-pending-jobs N to startup argv.
  --render-compile-capacity default|derived
                     Add --render-compile-capacity MODE to startup argv.
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
  --adaptive-chunk-publication-budget true|false
                     Add --adaptive-chunk-publication-budget VALUE to startup
                     argv for local integrated worlds.
  --session-smoke MODE
                     Run a launch-scoped in-headset session replacement smoke.
                     MODE is new-world.
  --perf-seconds N  After the first submitted terrain frame, sample N seconds
                     of headset frame timing and wait for the
                     MCLONE_ANDROID_XR_PERF_SUMMARY/HEADROOM marker block.
                     Compare app_work_* / headroom_* fields for real headroom;
                     frame_avg_ms is compositor-paced cadence, not work cost.
  --perf-flight      During --perf-seconds, fly forward in no-clip at about
                     walking speed instead of sampling a passive headset view.
  --perf-flight-speed N
                     Flight speed in blocks/second. Implies --perf-flight.
                     Default: 4.3.
  --perf-settled-orbit
                     Start the timed sample only after the settled gate, then
                     move in a local no-clip orbit around the settled chunk
                     cluster to exercise streaming with a populated scene.
  --perf-orbit-speed N
                     Orbit speed in blocks/second. Implies
                     --perf-settled-orbit. Default: 4.3.
  --perf-chunk-view-churn
                     Start the timed sample only after the settled gate, then
                     alternate the chunk interest center without headset
                     locomotion to exercise client apply/update pacing.
  --perf-churn-interval-seconds N
                     Seconds between chunk-interest toggles. Implies
                     --perf-chunk-view-churn. Default: 3.
  --perf-churn-offset-chunks N
                     Positive X chunk offset for the churn target. Implies
                     --perf-chunk-view-churn. Default: 16.
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
                     When combined with --perf-seconds, the metrics baseline is
                     snapped at MCLONE_ANDROID_XR_PERF_START.
  --perf-metrics-periodic
                     Keep the XR_META_performance_metrics probe enabled after
                     the first valid sample and emit repeated markers during
                     longer perf lanes. Implies --perf-metrics.
  --perf-detail full|minimal
                     Select perf summary detail. full is the default and emits
                     queue/peer/stage/worst-frame markers. minimal emits only
                     high-level frame/headroom/publication counters and skips
                     retaining per-frame terrain summaries.
  --frame-accounting true|false
                     Enable or disable per-frame shared accounting work during
                     perf samples. The default is true; false is for overhead
                     A/B measurement and still emits compatible summary markers.
  --perf-summary PATH
                     Local file for the compact perf marker block. Default:
                     /tmp/mclone-quest-openxr-perf-summary.txt.
  --multiview-proof  Launch the minimal XR multiview proof path. The app
                     creates one two-layer color swapchain, renders a
                     view_index-colored multiview pass, presents layers 0/1,
                     and waits for MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY.
  --terrain-multiview-proof
                     Launch the chunk-terrain XR multiview proof path. The app
                     creates one two-layer color swapchain, renders terrain
                     with @builtin(view_index), reads both layers back, and
                     waits for MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY.
  --terrain-multiview-perf
                     Launch a terrain-only offscreen A/B microbenchmark. The
                     app compares current two-eye terrain rendering against the
                     chunk-terrain multiview path and waits for
                     MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY.
  --sky-terrain-multiview-perf
                     Launch the same offscreen A/B microbenchmark with sky
                     background plus chunk terrain and wait for
                     MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY.
  --sky-terrain-actors-multiview-perf
                     Launch the same offscreen A/B microbenchmark with sky,
                     chunk terrain, and actors and wait for
                     MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY.
  --xr-skip-actors   Debug/perf probe: skip actor collection and rendering in
                     the normal XR scene.
  --xr-full-frame-multiview
                     Render normal submitted headset frames through the
                     full-frame multiview stack and require
                     MCLONE_ANDROID_XR_FULL_FRAME_MULTIVIEW_READY. Can be
                     combined with ordinary --perf-* probes.
  --xr-frame-overlap
                     Per-eye path only: explicitly use the default frame
                     overlap path: submit both eyes with a deferred GPU wait,
                     prefetch live runtime/render-section work while that
                     submission is in flight, consume it on the next frame,
                     and report render_path=per-eye-frame-overlap.
  --xr-frame-serial
                     Per-eye path only: force the legacy serial frame shape
                     for A/B probes and report render_path=per-eye.
  --xr-overlap-eye-submits
                     Per-eye path only: submit each eye as soon as encoded,
                     defer the GPU wait until both eyes are submitted, and
                     report render_path=per-eye-overlap in perf summaries.
  --xr-overlap-runtime-prefetch
                     Per-eye path only: submit both eyes with a deferred GPU
                     wait, run one runtime/render-section prefetch while that
                     submission is in flight, and report
                     render_path=per-eye-prefetch in perf summaries.
  --xr-render-section-upload-budget N
                     Upload at most N rebuilt render sections per live XR frame.
                     Removals still apply immediately. This is an opt-in probe
                     for smoothing runtime/render-section upload bursts.
  --xr-render-section-accept-budget N
                     Accept at most N rebuilt or removed render sections into
                     XR draw resources per live frame. This is opt-in and
                     leaves the default unbounded behavior unchanged.
  --xr-render-completed-result-accept-budget N
                     Accept at most N completed render compiler results into
                     the shared render-section cache per live frame. This is
                     opt-in and leaves the default unbounded behavior unchanged.
  --xr-foveation off|low|medium|high
                     Apply XR_FB_foveation to submitted eye swapchains. Default:
                     off. Use high for the RD10 fixed-foveated-rendering probe.
  --xr-render-scale SCALE
                     Scale OpenXR eye swapchain dimensions before rendering.
                     Accepts 0.25..1.0 or percent values like 85%.
  --xr-display-refresh-rate HZ
                     Request an XR_FB_display_refresh_rate before validation.
                     Use 90 for a 90 Hz Quest performance probe.
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

validate_xr_render_scale() {
    local label="$1"
    local value="$2"
    local numeric="$value"
    local scale
    if [[ "$numeric" == *% ]]; then
        numeric="${numeric%\%}"
        [[ "$numeric" =~ ^[0-9]+([.][0-9]+)?$ ]] || mclone_die "$label must be a scale or percent, got '$value'"
        scale="$(awk -v value="$numeric" 'BEGIN { printf "%.6f", value / 100.0 }')"
    elif [[ "$numeric" == *x || "$numeric" == *X ]]; then
        numeric="${numeric%[xX]}"
        [[ "$numeric" =~ ^[0-9]+([.][0-9]+)?$ ]] || mclone_die "$label must be a scale or percent, got '$value'"
        scale="$numeric"
    else
        [[ "$numeric" =~ ^[0-9]+([.][0-9]+)?$ ]] || mclone_die "$label must be a scale or percent, got '$value'"
        scale="$numeric"
    fi
    awk -v value="$scale" 'BEGIN { exit !(value >= 0.25 && value <= 1.0) }' \
        || mclone_die "$label must be between 0.25 and 1.0, got '$value'"
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
        --xr-underwater-mode)
            require_arg "$1" "${2:-}"
            XR_UNDERWATER_MODE="$2"
            shift 2
            ;;
        --xr-debug-ui)
            require_arg "$1" "${2:-}"
            XR_DEBUG_UI="$2"
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
        --chunk-x|--chunk-z|--render-distance|--render-compile-workers|--render-compile-max-pending-jobs|--render-compile-capacity|--movement-speed-multiplier|--day-time|--lighting|--section-occlusion|--fullbright|--adaptive-chunk-publication-budget|--world-dir|--world-root)
            require_arg "$1" "${2:-}"
            STARTUP_ARGV+=("$1" "$2")
            shift 2
            ;;
        --freeze-time|--transient)
            STARTUP_ARGV+=("$1")
            shift
            ;;
        --app-arg)
            require_arg "$1" "${2:-}"
            STARTUP_ARGV+=("$2")
            shift 2
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
        --perf-settled-orbit)
            PERF_SETTLED_ORBIT=1
            shift
            ;;
        --perf-orbit-speed)
            require_arg "$1" "${2:-}"
            PERF_SETTLED_ORBIT=1
            PERF_ORBIT_SPEED="$2"
            shift 2
            ;;
        --perf-chunk-view-churn)
            PERF_CHUNK_VIEW_CHURN=1
            shift
            ;;
        --perf-churn-interval-seconds)
            require_arg "$1" "${2:-}"
            PERF_CHUNK_VIEW_CHURN=1
            PERF_CHURN_INTERVAL_SECONDS="$2"
            shift 2
            ;;
        --perf-churn-offset-chunks)
            require_arg "$1" "${2:-}"
            PERF_CHUNK_VIEW_CHURN=1
            PERF_CHURN_OFFSET_CHUNKS="$2"
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
        --perf-metrics-periodic)
            PERF_METRICS=1
            PERF_METRICS_PERIODIC=1
            shift
            ;;
        --perf-detail)
            require_arg "$1" "${2:-}"
            PERF_DETAIL="$2"
            shift 2
            ;;
        --frame-accounting)
            require_arg "$1" "${2:-}"
            FRAME_ACCOUNTING="$2"
            shift 2
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
        --terrain-multiview-proof)
            TERRAIN_MULTIVIEW_PROOF=1
            shift
            ;;
        --terrain-multiview-perf)
            TERRAIN_MULTIVIEW_PERF=1
            shift
            ;;
        --sky-terrain-multiview-perf)
            SKY_TERRAIN_MULTIVIEW_PERF=1
            shift
            ;;
        --sky-terrain-actors-multiview-perf)
            SKY_TERRAIN_ACTORS_MULTIVIEW_PERF=1
            shift
            ;;
        --xr-skip-actors)
            XR_SKIP_ACTORS=1
            shift
            ;;
        --xr-full-frame-multiview)
            XR_FULL_FRAME_MULTIVIEW=1
            shift
            ;;
        --xr-frame-overlap)
            XR_FRAME_OVERLAP=1
            shift
            ;;
        --xr-frame-serial)
            XR_FRAME_SERIAL=1
            shift
            ;;
        --xr-overlap-eye-submits)
            XR_OVERLAP_EYE_SUBMITS=1
            shift
            ;;
        --xr-overlap-runtime-prefetch)
            XR_OVERLAP_RUNTIME_PREFETCH=1
            shift
            ;;
        --xr-render-section-upload-budget)
            require_arg "$1" "${2:-}"
            XR_RENDER_SECTION_UPLOAD_BUDGET="$2"
            shift 2
            ;;
        --xr-render-section-accept-budget)
            require_arg "$1" "${2:-}"
            XR_RENDER_SECTION_ACCEPT_BUDGET="$2"
            shift 2
            ;;
        --xr-render-completed-result-accept-budget)
            require_arg "$1" "${2:-}"
            XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET="$2"
            shift 2
            ;;
        --xr-foveation)
            require_arg "$1" "${2:-}"
            XR_FOVEATION="$2"
            shift 2
            ;;
        --xr-render-scale)
            require_arg "$1" "${2:-}"
            XR_RENDER_SCALE="$2"
            shift 2
            ;;
        --xr-display-refresh-rate)
            require_arg "$1" "${2:-}"
            XR_DISPLAY_REFRESH_RATE="$2"
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
case "$XR_UNDERWATER_MODE" in
    ""|midpoint|per-eye)
        ;;
    *)
        mclone_die "unsupported --xr-underwater-mode '$XR_UNDERWATER_MODE'; expected midpoint or per-eye"
        ;;
esac
case "$XR_DEBUG_UI" in
    ""|none|pause|controls)
        ;;
    *)
        mclone_die "unsupported --xr-debug-ui '$XR_DEBUG_UI'; expected none, pause, or controls"
        ;;
esac
case "$FRAME_ACCOUNTING" in
    ""|true|false|1|0|yes|no|on|off)
        ;;
    *)
        mclone_die "unsupported --frame-accounting '$FRAME_ACCOUNTING'; expected true or false"
        ;;
esac
case "$PERF_DETAIL" in
    full|minimal)
        ;;
    *)
        mclone_die "unsupported --perf-detail '$PERF_DETAIL'; expected full or minimal"
        ;;
esac
if [[ "$PERF_DETAIL" == "minimal" && "$FRAME_ACCOUNTING" =~ ^(false|0|no|off)$ ]]; then
    mclone_die "--perf-detail minimal requires frame accounting"
fi
if [[ -n "$SESSION_SMOKE" && "$SESSION_ONLY" == "1" ]]; then
    mclone_die "--session-smoke requires submitted-frame validation; remove --session-only"
fi
if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
    if [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "--multiview-proof cannot be combined with --terrain-multiview-proof"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--multiview-proof has its own proof marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--multiview-proof cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--multiview-proof cannot be combined with performance probes"
    fi
fi
if [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
    if [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--terrain-multiview-proof cannot be combined with --terrain-multiview-perf"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--terrain-multiview-proof has its own proof marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--terrain-multiview-proof cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--terrain-multiview-proof cannot be combined with performance probes"
    fi
fi
if [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    if [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--terrain-multiview-perf cannot be combined with other multiview perf modes"
    fi
    if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "--terrain-multiview-perf cannot be combined with --multiview-proof"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--terrain-multiview-perf has its own summary marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--terrain-multiview-perf cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--terrain-multiview-perf cannot be combined with frame performance probes"
    fi
fi
if [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    if [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--sky-terrain-multiview-perf cannot be combined with --sky-terrain-actors-multiview-perf"
    fi
    if [[ "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "--sky-terrain-multiview-perf cannot be combined with multiview proof modes"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--sky-terrain-multiview-perf has its own summary marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--sky-terrain-multiview-perf cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--sky-terrain-multiview-perf cannot be combined with frame performance probes"
    fi
fi
if [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
    if [[ "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "--sky-terrain-actors-multiview-perf cannot be combined with multiview proof modes"
    fi
    if [[ "$SESSION_ONLY" == "1" ]]; then
        mclone_die "--sky-terrain-actors-multiview-perf has its own summary marker; remove --session-only"
    fi
    if [[ -n "$SESSION_SMOKE" ]]; then
        mclone_die "--sky-terrain-actors-multiview-perf cannot be combined with --session-smoke"
    fi
    if [[ -n "$PERF_SECONDS" || "$PERF_FLIGHT" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" || "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_FROZEN_RENDER" == "1" || "$PERF_METRICS" == "1" ]]; then
        mclone_die "--sky-terrain-actors-multiview-perf cannot be combined with frame performance probes"
    fi
fi
if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" ]]; then
    if [[ "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--xr-full-frame-multiview cannot be combined with multiview proof or microbenchmark modes"
    fi
fi
if [[ "$XR_FRAME_OVERLAP" == "1" ]]; then
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" || "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--xr-frame-overlap only applies to the per-eye full-frame path"
    fi
    if [[ "$XR_FRAME_SERIAL" == "1" ]]; then
        mclone_die "--xr-frame-overlap cannot be combined with --xr-frame-serial"
    fi
    if [[ "$XR_OVERLAP_EYE_SUBMITS" == "1" || "$XR_OVERLAP_RUNTIME_PREFETCH" == "1" ]]; then
        mclone_die "--xr-frame-overlap cannot be combined with older overlap probe flags"
    fi
fi
if [[ "$XR_FRAME_SERIAL" == "1" ]]; then
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" || "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--xr-frame-serial only applies to the per-eye full-frame path"
    fi
    if [[ "$XR_OVERLAP_EYE_SUBMITS" == "1" || "$XR_OVERLAP_RUNTIME_PREFETCH" == "1" ]]; then
        mclone_die "--xr-frame-serial cannot be combined with older overlap probe flags"
    fi
fi
if [[ "$XR_OVERLAP_EYE_SUBMITS" == "1" ]]; then
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" || "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--xr-overlap-eye-submits only applies to the per-eye full-frame path"
    fi
fi
if [[ "$XR_OVERLAP_RUNTIME_PREFETCH" == "1" ]]; then
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" || "$MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PROOF" == "1" || "$TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" || "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "--xr-overlap-runtime-prefetch only applies to the per-eye full-frame path"
    fi
    if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
        mclone_die "--xr-overlap-runtime-prefetch cannot be combined with --perf-frozen-render"
    fi
fi
if [[ -n "$XR_RENDER_SECTION_UPLOAD_BUDGET" ]]; then
    validate_positive_integer "--xr-render-section-upload-budget" "$XR_RENDER_SECTION_UPLOAD_BUDGET"
fi
if [[ -n "$XR_RENDER_SECTION_ACCEPT_BUDGET" ]]; then
    validate_positive_integer "--xr-render-section-accept-budget" "$XR_RENDER_SECTION_ACCEPT_BUDGET"
fi
if [[ -n "$XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET" ]]; then
    validate_positive_integer "--xr-render-completed-result-accept-budget" "$XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET"
fi
case "$XR_FOVEATION" in
    off|none|false|0|disabled|low|medium|med|high)
        ;;
    *)
        mclone_die "unsupported --xr-foveation '$XR_FOVEATION'; expected off, low, medium, or high"
        ;;
esac
if [[ -n "$XR_RENDER_SCALE" ]]; then
    validate_xr_render_scale "--xr-render-scale" "$XR_RENDER_SCALE"
fi
if [[ -n "$XR_DISPLAY_REFRESH_RATE" ]]; then
    validate_positive_number "--xr-display-refresh-rate" "$XR_DISPLAY_REFRESH_RATE"
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
        if [[ "$PERF_SETTLED_STATIONARY" == "1" || "$PERF_SETTLED_ORBIT" == "1" || "$PERF_CHUNK_VIEW_CHURN" == "1" ]]; then
            WAIT_SECONDS=$((10#$PERF_SECONDS + 180))
        else
            WAIT_SECONDS=$((10#$PERF_SECONDS + 30))
        fi
    fi
fi
if [[ "$PERF_FLIGHT" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-flight requires --perf-seconds"
fi
if [[ "$PERF_SETTLED_ORBIT" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-settled-orbit requires --perf-seconds"
fi
if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-chunk-view-churn requires --perf-seconds"
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-settled-stationary requires --perf-seconds"
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-settled-stationary cannot be combined with --perf-flight"
fi
if [[ "$PERF_SETTLED_ORBIT" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-settled-orbit cannot be combined with --perf-flight"
fi
if [[ "$PERF_SETTLED_ORBIT" == "1" && "$PERF_SETTLED_STATIONARY" == "1" ]]; then
    mclone_die "--perf-settled-orbit cannot be combined with --perf-settled-stationary"
fi
if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-chunk-view-churn cannot be combined with --perf-flight"
fi
if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" && "$PERF_SETTLED_ORBIT" == "1" ]]; then
    mclone_die "--perf-chunk-view-churn cannot be combined with --perf-settled-orbit"
fi
if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" && "$PERF_SETTLED_STATIONARY" == "1" ]]; then
    mclone_die "--perf-chunk-view-churn cannot be combined with --perf-settled-stationary"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && -z "$PERF_SECONDS" ]]; then
    mclone_die "--perf-frozen-render requires --perf-seconds"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && "$PERF_FLIGHT" == "1" ]]; then
    mclone_die "--perf-frozen-render cannot be combined with --perf-flight"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && "$PERF_CHUNK_VIEW_CHURN" == "1" ]]; then
    mclone_die "--perf-frozen-render cannot be combined with --perf-chunk-view-churn"
fi
if [[ "$PERF_FROZEN_RENDER" == "1" && "$START_VIEW_POSE" == "0" ]]; then
    mclone_die "--perf-frozen-render requires --view-pose X,Y,Z,YAW_DEGREES"
fi
if [[ -n "$PERF_FLIGHT_SPEED" ]]; then
    validate_positive_number "--perf-flight-speed" "$PERF_FLIGHT_SPEED"
fi
if [[ -n "$PERF_ORBIT_SPEED" ]]; then
    validate_positive_number "--perf-orbit-speed" "$PERF_ORBIT_SPEED"
fi
if [[ -n "$PERF_CHURN_INTERVAL_SECONDS" ]]; then
    validate_positive_number "--perf-churn-interval-seconds" "$PERF_CHURN_INTERVAL_SECONDS"
fi
if [[ -n "$PERF_CHURN_OFFSET_CHUNKS" ]]; then
    validate_positive_integer "--perf-churn-offset-chunks" "$PERF_CHURN_OFFSET_CHUNKS"
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
if [[ -n "$XR_UNDERWATER_MODE" ]]; then
    STARTUP_ARGV+=(--xr-underwater-mode "$XR_UNDERWATER_MODE")
fi
if [[ -n "$XR_DEBUG_UI" ]]; then
    STARTUP_ARGV+=(--xr-debug-ui "$XR_DEBUG_UI")
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
if [[ "$PERF_SETTLED_ORBIT" == "1" ]]; then
    STARTUP_ARGV+=(--perf-settled-orbit)
    if [[ -n "$PERF_ORBIT_SPEED" ]]; then
        STARTUP_ARGV+=(--perf-orbit-speed "$PERF_ORBIT_SPEED")
    fi
fi
if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" ]]; then
    STARTUP_ARGV+=(--perf-chunk-view-churn)
    if [[ -n "$PERF_CHURN_INTERVAL_SECONDS" ]]; then
        STARTUP_ARGV+=(--perf-churn-interval-seconds "$PERF_CHURN_INTERVAL_SECONDS")
    fi
    if [[ -n "$PERF_CHURN_OFFSET_CHUNKS" ]]; then
        STARTUP_ARGV+=(--perf-churn-offset-chunks "$PERF_CHURN_OFFSET_CHUNKS")
    fi
fi
if [[ "$PERF_SETTLED_STATIONARY" == "1" ]]; then
    STARTUP_ARGV+=(--perf-settled-stationary)
fi
if [[ "$PERF_FROZEN_RENDER" == "1" ]]; then
    STARTUP_ARGV+=(--perf-frozen-render)
fi
if [[ "$PERF_METRICS_PERIODIC" == "1" ]]; then
    STARTUP_ARGV+=(--perf-metrics-periodic)
elif [[ "$PERF_METRICS" == "1" ]]; then
    STARTUP_ARGV+=(--perf-metrics)
fi
if [[ "$PERF_DETAIL" != "full" ]]; then
    STARTUP_ARGV+=(--perf-detail "$PERF_DETAIL")
fi
if [[ -n "$FRAME_ACCOUNTING" ]]; then
    STARTUP_ARGV+=(--frame-accounting "$FRAME_ACCOUNTING")
fi
if [[ "$MULTIVIEW_PROOF" == "1" ]]; then
    STARTUP_ARGV+=(--multiview-proof)
fi
if [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
    STARTUP_ARGV+=(--terrain-multiview-proof)
fi
if [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    STARTUP_ARGV+=(--terrain-multiview-perf)
fi
if [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    STARTUP_ARGV+=(--sky-terrain-multiview-perf)
fi
if [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
    STARTUP_ARGV+=(--sky-terrain-actors-multiview-perf)
fi
if [[ "$XR_SKIP_ACTORS" == "1" ]]; then
    STARTUP_ARGV+=(--xr-skip-actors)
fi
if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" ]]; then
    STARTUP_ARGV+=(--xr-full-frame-multiview)
fi
if [[ "$XR_FRAME_OVERLAP" == "1" ]]; then
    STARTUP_ARGV+=(--xr-frame-overlap)
fi
if [[ "$XR_FRAME_SERIAL" == "1" ]]; then
    STARTUP_ARGV+=(--xr-frame-serial)
fi
if [[ "$XR_OVERLAP_EYE_SUBMITS" == "1" ]]; then
    STARTUP_ARGV+=(--xr-overlap-eye-submits)
fi
if [[ "$XR_OVERLAP_RUNTIME_PREFETCH" == "1" ]]; then
    STARTUP_ARGV+=(--xr-overlap-runtime-prefetch)
fi
if [[ -n "$XR_RENDER_SECTION_UPLOAD_BUDGET" ]]; then
    STARTUP_ARGV+=(--xr-render-section-upload-budget "$XR_RENDER_SECTION_UPLOAD_BUDGET")
fi
if [[ -n "$XR_RENDER_SECTION_ACCEPT_BUDGET" ]]; then
    STARTUP_ARGV+=(--xr-render-section-accept-budget "$XR_RENDER_SECTION_ACCEPT_BUDGET")
fi
if [[ -n "$XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET" ]]; then
    STARTUP_ARGV+=(--xr-render-completed-result-accept-budget "$XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET")
fi
if [[ "$XR_FOVEATION" != "off" ]]; then
    STARTUP_ARGV+=(--xr-foveation "$XR_FOVEATION")
fi
if [[ -n "$XR_RENDER_SCALE" ]]; then
    STARTUP_ARGV+=(--xr-render-scale "$XR_RENDER_SCALE")
fi
if [[ -n "$XR_DISPLAY_REFRESH_RATE" ]]; then
    STARTUP_ARGV+=(--xr-display-refresh-rate "$XR_DISPLAY_REFRESH_RATE")
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
    if [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]] && grep -F "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]] && grep -F "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" ]] && grep -F "MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]] && grep -F "MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" && -z "$PERF_SECONDS" && -z "$SESSION_SMOKE" ]] && grep -F "MCLONE_ANDROID_XR_FULL_FRAME_MULTIVIEW_READY" "$LOG_PATH" >/dev/null 2>&1; then
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
    if [[ "$MULTIVIEW_PROOF" != "1" && "$TERRAIN_MULTIVIEW_PROOF" != "1" && "$TERRAIN_MULTIVIEW_PERF" != "1" && "$SKY_TERRAIN_MULTIVIEW_PERF" != "1" && "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" != "1" && "$XR_FULL_FRAME_MULTIVIEW" != "1" && -z "$SESSION_SMOKE" && -z "$PERF_SECONDS" ]] && grep -F "MCLONE_ANDROID_XR_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if [[ "$SESSION_ONLY" == "1" ]] && grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        success=1
        break
    fi
    if grep -E "MCLONE_ANDROID_XR_FAILURE|FATAL EXCEPTION|Fatal signal|thread .* panicked|panicked at" "$LOG_PATH" >/dev/null 2>&1; then
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
    elif [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
        mclone_die "Android XR terrain multiview proof marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "Android XR terrain multiview perf summary marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "Android XR sky+terrain multiview perf summary marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
        mclone_die "Android XR sky+terrain+actors multiview perf summary marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
    elif [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" && -z "$PERF_SECONDS" && -z "$SESSION_SMOKE" ]]; then
        mclone_die "Android XR full-frame multiview ready marker was not seen within ${WAIT_SECONDS}s; see $LOG_PATH"
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
elif [[ "$TERRAIN_MULTIVIEW_PROOF" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR session-ready marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR terrain multiview proof marker was not seen; see $LOG_PATH"
    fi
elif [[ "$TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR session-ready marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR terrain multiview perf summary marker was not seen; see $LOG_PATH"
    fi
elif [[ "$SKY_TERRAIN_MULTIVIEW_PERF" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR session-ready marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR sky+terrain multiview perf summary marker was not seen; see $LOG_PATH"
    fi
elif [[ "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_SESSION_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR session-ready marker was not seen; see $LOG_PATH"
    fi
    if ! grep -F "MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR sky+terrain+actors multiview perf summary marker was not seen; see $LOG_PATH"
    fi
elif ! grep -F "MCLONE_ANDROID_XR_CONTROLLERS_READY" "$LOG_PATH" >/dev/null 2>&1; then
    mclone_die "Android XR controllers-ready marker was not seen; see $LOG_PATH"
fi
if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" ]]; then
    if ! grep -F "MCLONE_ANDROID_XR_FULL_FRAME_MULTIVIEW_READY" "$LOG_PATH" >/dev/null 2>&1; then
        mclone_die "Android XR full-frame multiview ready marker was not seen; see $LOG_PATH"
    fi
fi
if [[ "$MULTIVIEW_PROOF" != "1" && "$TERRAIN_MULTIVIEW_PROOF" != "1" && "$TERRAIN_MULTIVIEW_PERF" != "1" && "$SKY_TERRAIN_MULTIVIEW_PERF" != "1" && "$SKY_TERRAIN_ACTORS_MULTIVIEW_PERF" != "1" && "$SESSION_ONLY" != "1" ]] && ! grep -F "MCLONE_ANDROID_XR_TERRAIN_READY" "$LOG_PATH" >/dev/null 2>&1; then
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
    if [[ "$XR_FULL_FRAME_MULTIVIEW" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*render_path=multiview" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR full-frame multiview perf-start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*render_path=multiview" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR full-frame multiview perf summary marker was not seen; see $LOG_PATH"
        fi
    elif [[ "$XR_OVERLAP_RUNTIME_PREFETCH" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*render_path=per-eye-prefetch" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR runtime-prefetch perf-start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*render_path=per-eye-prefetch" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR runtime-prefetch perf summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ -n "$XR_RENDER_SECTION_UPLOAD_BUDGET" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*render_section_upload_budget=${XR_RENDER_SECTION_UPLOAD_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render-section upload budget perf-start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*render_section_upload_budget=${XR_RENDER_SECTION_UPLOAD_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render-section upload budget perf summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ -n "$XR_RENDER_SECTION_ACCEPT_BUDGET" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*render_section_accept_budget=${XR_RENDER_SECTION_ACCEPT_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render-section accept budget perf-start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*render_section_accept_budget=${XR_RENDER_SECTION_ACCEPT_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render-section accept budget perf summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ -n "$XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*render_completed_result_accept_budget=${XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render completed-result accept budget perf-start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*render_completed_result_accept_budget=${XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR render completed-result accept budget perf summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ "$PERF_DETAIL" == "minimal" ]]; then
        for marker in \
            MCLONE_ANDROID_XR_PERF_SUMMARY \
            MCLONE_ANDROID_XR_PERF_PUBLICATION \
            MCLONE_ANDROID_XR_PERF_HEADROOM \
            MCLONE_ANDROID_XR_PERF_CPU_BLOCKED \
            MCLONE_ANDROID_XR_PERF_DETAIL \
            MCLONE_ANDROID_XR_PERF_DRAW
        do
            if ! grep -E "$marker([[:space:]]|$)" "$LOG_PATH" >/dev/null 2>&1; then
                mclone_die "Android XR minimal perf marker $marker was not seen; see $LOG_PATH"
            fi
        done
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*detail=minimal" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR minimal perf summary detail marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_DETAIL .*frame_details=disabled" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR minimal perf detail marker was not seen; see $LOG_PATH"
        fi
        if grep -E "MCLONE_ANDROID_XR_PERF_(FRAME_PIPELINE|QUEUE|PEER|STAGE|STAGES|LOCOMOTION|LOCOMOTION_COMMAND|LOCOMOTION_INTEREST_COMMAND|TERRAIN|TERRAIN_RUNTIME|GPU_SYNC_MAX|UPLOAD_APPLY_MAX|TERRAIN_PREP|OVERLAP|MULTIVIEW|UPLOAD_MAX|UPLOAD_PHASE_MAX|RECORD_CACHE|RUNTIME_MAX|UPDATE_APPLY_MAX|QUEUE_MAX|COMPILE_MAX|UPLOAD_LAST|WORST_FRAME)([[:space:]]|$)" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR minimal perf emitted full-detail markers; see $LOG_PATH"
        fi
    else
        for marker in \
            MCLONE_ANDROID_XR_PERF_SUMMARY \
            MCLONE_ANDROID_XR_PERF_FRAME_PIPELINE \
            MCLONE_ANDROID_XR_PERF_QUEUE \
            MCLONE_ANDROID_XR_PERF_PEER \
            MCLONE_ANDROID_XR_PERF_STAGE \
            MCLONE_ANDROID_XR_PERF_HEADROOM \
            MCLONE_ANDROID_XR_PERF_STAGES \
            MCLONE_ANDROID_XR_PERF_TERRAIN \
            MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME \
            MCLONE_ANDROID_XR_PERF_GPU_SYNC_MAX \
            MCLONE_ANDROID_XR_PERF_UPLOAD_APPLY_MAX \
            MCLONE_ANDROID_XR_PERF_TERRAIN_PREP \
            MCLONE_ANDROID_XR_PERF_OVERLAP \
            MCLONE_ANDROID_XR_PERF_MULTIVIEW \
            MCLONE_ANDROID_XR_PERF_UPLOAD_MAX \
            MCLONE_ANDROID_XR_PERF_UPLOAD_PHASE_MAX \
            MCLONE_ANDROID_XR_PERF_RECORD_CACHE \
            MCLONE_ANDROID_XR_PERF_RUNTIME_MAX \
            MCLONE_ANDROID_XR_PERF_UPDATE_APPLY_MAX \
            MCLONE_ANDROID_XR_PERF_QUEUE_MAX \
            MCLONE_ANDROID_XR_PERF_COMPILE_MAX \
            MCLONE_ANDROID_XR_PERF_UPLOAD_LAST \
            MCLONE_ANDROID_XR_PERF_DRAW
        do
            if ! grep -E "$marker([[:space:]]|$)" "$LOG_PATH" >/dev/null 2>&1; then
                mclone_die "Android XR perf marker $marker was not seen; see $LOG_PATH"
            fi
        done
    fi
    if [[ "$PERF_FLIGHT" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*mode=flight" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf flight start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*mode=flight .*flight_distance_blocks=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf flight summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ "$PERF_SETTLED_ORBIT" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SETTLED .*mode=settled-orbit" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled orbit marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*mode=settled-orbit .*settle_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled orbit start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*mode=settled-orbit .*settle_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf settled orbit summary marker was not seen; see $LOG_PATH"
        fi
    fi
    if [[ "$PERF_CHUNK_VIEW_CHURN" == "1" ]]; then
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SETTLED .*mode=chunk-view-churn" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf chunk-view churn settled marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_START .*mode=chunk-view-churn .*chunk_view_churn_interval_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf chunk-view churn start marker was not seen; see $LOG_PATH"
        fi
        if ! grep -E "MCLONE_ANDROID_XR_PERF_SUMMARY .*mode=chunk-view-churn .*chunk_view_churn_interval_seconds=" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf chunk-view churn summary marker was not seen; see $LOG_PATH"
        fi
        if ! grep -F "MCLONE_ANDROID_XR_CHUNK_VIEW_CHURN" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR chunk-view churn command marker was not seen; see $LOG_PATH"
        fi
        if [[ -n "$PERF_CHURN_OFFSET_CHUNKS" ]] && ! grep -E "MCLONE_ANDROID_XR_PERF_START .*chunk_view_churn_offset_chunks=${PERF_CHURN_OFFSET_CHUNKS}" "$LOG_PATH" >/dev/null 2>&1; then
            mclone_die "Android XR perf chunk-view churn offset marker was not seen; see $LOG_PATH"
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
    grep -E "MCLONE_ANDROID_XR_PERF_(START|SETTLED|SUMMARY|FRAME_PIPELINE|QUEUE|PEER|STAGE|HEADROOM|CPU_BLOCKED|HORIZON|PERSISTENCE|DETAIL|PUBLICATION|STAGES|LOCOMOTION|LOCOMOTION_COMMAND|LOCOMOTION_INTEREST_COMMAND|TERRAIN|TERRAIN_RUNTIME|TERRAIN_EYE_SPLIT|GPU_SYNC_MAX|UPLOAD_APPLY_MAX|TERRAIN_PREP|OVERLAP|MULTIVIEW|UPLOAD_MAX|UPLOAD_PHASE_MAX|RECORD_CACHE|RUNTIME_MAX|UPDATE_APPLY_MAX|QUEUE_MAX|COMPILE_MAX|UPLOAD_LAST|DRAW|WORST_FRAME|WORST_FRAME_LOCOMOTION|WORST_FRAME_LOCOMOTION_COMMAND|WORST_FRAME_LOCOMOTION_INTEREST_COMMAND|WORST_FRAME_BUDGET|WORST_FRAME_TERRAIN|WORST_FRAME_RUNTIME|WORST_FRAME_GPU_SYNC|WORST_FRAME_UPLOAD_APPLY|WORST_FRAME_UPDATE_APPLY|WORST_FRAME_UPLOAD)([[:space:]]|$)|MCLONE_ANDROID_XR_PERF_METRICS(_WINDOW_START)?[[:space:]]" "$LOG_PATH" \
        | tail -n 160 > "$PERF_SUMMARY_PATH"
    mclone_note "Perf summary: $PERF_SUMMARY_PATH"
fi

pid="$("$ADB" -s "$SERIAL" shell pidof "$MCLONE_ANDROID_XR_APP_ID" 2>/dev/null | tr -d '\r' || true)"
if [[ -z "$pid" ]]; then
    mclone_die "Android XR process exited after launch; see $LOG_PATH"
fi
mclone_note "$MCLONE_ANDROID_XR_APP_ID pid: $pid"
mclone_note "Logcat: $LOG_PATH"
mclone_note "Android XR package launch validation passed"
