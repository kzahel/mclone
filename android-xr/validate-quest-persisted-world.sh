#!/usr/bin/env bash
set -euo pipefail

ANDROID_XR_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_XR_DIR/.." && pwd)"
ANDROID_DIR="$REPO_ROOT/android"
source "$ANDROID_DIR/validate-common.sh"
source "$ANDROID_XR_DIR/startup-properties.sh"

MCLONE_ANDROID_XR_APP_ID="${MCLONE_ANDROID_XR_APP_ID:-com.kzahel.mclone.xr}"
BUILD_TYPE="${MCLONE_ANDROID_XR_BUILD_TYPE:-release}"
BOOT_TIMEOUT_SECONDS="${MCLONE_ANDROID_BOOT_TIMEOUT:-60}"
SERIAL=""
SKIP_BUILD=0
STAGE_ASSETS="${MCLONE_ANDROID_XR_STAGE_ASSETS:-1}"
ASSET_PACK="${MCLONE_ANDROID_ASSET_PACK:-}"
WORLD_DIR="${MCLONE_ANDROID_XR_PERSISTED_WORLD_DIR:-/sdcard/Android/data/${MCLONE_ANDROID_XR_APP_ID}/files/persisted-world-guardrail/rd5}"
PREWARM_SECONDS="${MCLONE_ANDROID_XR_PERSISTED_PREWARM_SECONDS:-20}"
PREWARM_WAIT_SECONDS="${MCLONE_ANDROID_XR_PERSISTED_PREWARM_WAIT_SECONDS:-210}"
SAMPLE_SECONDS="${MCLONE_ANDROID_XR_PERSISTED_SAMPLE_SECONDS:-45}"
SAMPLE_WAIT_SECONDS="${MCLONE_ANDROID_XR_PERSISTED_SAMPLE_WAIT_SECONDS:-210}"
PREWARM_LOG="${MCLONE_ANDROID_XR_PERSISTED_PREWARM_LOG:-/tmp/mclone-quest-openxr-persisted-rd5-prewarm-logcat.txt}"
PREWARM_SUMMARY="${MCLONE_ANDROID_XR_PERSISTED_PREWARM_SUMMARY:-/tmp/mclone-quest-openxr-persisted-rd5-prewarm-summary.txt}"
SAMPLE_LOG="${MCLONE_ANDROID_XR_PERSISTED_LOG:-/tmp/mclone-quest-openxr-persisted-rd5-logcat.txt}"
SAMPLE_SUMMARY="${MCLONE_ANDROID_XR_PERSISTED_SUMMARY:-/tmp/mclone-quest-openxr-persisted-rd5-summary.txt}"
VIEW_POSE="${MCLONE_ANDROID_XR_VIEW_POSE:-0,120,-96,180}"
SEED="${MCLONE_ANDROID_XR_SEED:-12345}"
CHUNK_X="${MCLONE_ANDROID_XR_CHUNK_X:-0}"
CHUNK_Z="${MCLONE_ANDROID_XR_CHUNK_Z:-0}"
RENDER_DISTANCE="${MCLONE_ANDROID_XR_RENDER_DISTANCE:-5}"
DAY_TIME="${MCLONE_ANDROID_XR_DAY_TIME:-6000}"
ORBIT_SPEED="${MCLONE_ANDROID_XR_PERF_ORBIT_SPEED:-4.3}"
FLIGHT_SPEED="${MCLONE_ANDROID_XR_PERF_FLIGHT_SPEED:-34.4}"
SAMPLE_MODE="${MCLONE_ANDROID_XR_PERSISTED_SAMPLE_MODE:-orbit}"
GENERATION_PROFILE="${MCLONE_ANDROID_XR_GENERATION_PROFILE:-mclone-overworld-v1}"
TERRAIN_PRESENTATION="${MCLONE_ANDROID_XR_TERRAIN_PRESENTATION:-exact-only}"
RENDER_COMPILE_WORKERS="${MCLONE_ANDROID_XR_RENDER_COMPILE_WORKERS:-2}"
RENDER_COMPLETED_RESULT_ACCEPT_BUDGET="${MCLONE_ANDROID_XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET:-2}"
RENDER_SECTION_UPLOAD_BUDGET="${MCLONE_ANDROID_XR_RENDER_SECTION_UPLOAD_BUDGET:-16}"
RENDER_SECTION_ACCEPT_BUDGET="${MCLONE_ANDROID_XR_RENDER_SECTION_ACCEPT_BUDGET:-64}"

usage() {
    cat <<'USAGE'
Usage: android-xr/validate-quest-persisted-world.sh [options]

Run the Quest persisted-world guardrail lane. The script removes a device world
directory, prewarms it through validate-quest-openxr.sh, then relaunches the app
against the same --world-dir for the RD5 settled-orbit metrics sample.

Options:
  --release          Build and validate the release APK. This is the default.
  --debug            Build and validate the debug APK.
  --serial SERIAL    Use a specific attached headset serial.
  --skip-build       Reuse the existing APK for the prewarm launch.
  --asset-pack PATH  Local packed assets file to stage before the prewarm launch.
  --skip-assets      Do not stage the packed Minecraft assets.
  --world-dir PATH   Device world directory to clean, prewarm, and reopen.
  --prewarm-seconds N
                     Prewarm settled-stationary perf sample duration.
  --prewarm-wait-seconds N
                     Prewarm launch wait timeout.
  --perf-seconds N   Reopen settled-orbit sample duration.
  --wait-seconds N   Reopen launch wait timeout.
  --prewarm-log PATH Local prewarm logcat output path.
  --prewarm-summary PATH
                     Local prewarm perf summary path.
  --log PATH         Local reopen logcat output path.
  --perf-summary PATH
                     Local reopen perf summary path.
  --view-pose X,Y,Z,YAW_DEGREES
  --seed SEED
  --chunk-x X
  --chunk-z Z
  --render-distance N
  --generation-profile PROFILE
                     Generation profile used for both launches.
  --terrain-presentation MODE
                     Terrain presentation used for both launches.
  --sample-mode MODE Reopen sample mode: orbit or flight.
  --perf-orbit-speed N
                     Orbit speed in blocks per second.
  --perf-flight-speed N
                     Flight speed in blocks per second.
  -h, --help         Show this help.
USAGE
}

require_arg() {
    local option="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || mclone_die "$option requires a value"
}

validate_positive_integer() {
    local label="$1"
    local value="$2"
    [[ "$value" =~ ^[0-9]+$ ]] || mclone_die "$label must be a positive integer, got '$value'"
    (( 10#$value > 0 )) || mclone_die "$label must be greater than zero"
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
        --asset-pack)
            require_arg "$1" "${2:-}"
            ASSET_PACK="$2"
            shift 2
            ;;
        --skip-assets)
            STAGE_ASSETS=0
            shift
            ;;
        --world-dir)
            require_arg "$1" "${2:-}"
            WORLD_DIR="$2"
            shift 2
            ;;
        --prewarm-seconds)
            require_arg "$1" "${2:-}"
            PREWARM_SECONDS="$2"
            shift 2
            ;;
        --prewarm-wait-seconds)
            require_arg "$1" "${2:-}"
            PREWARM_WAIT_SECONDS="$2"
            shift 2
            ;;
        --perf-seconds)
            require_arg "$1" "${2:-}"
            SAMPLE_SECONDS="$2"
            shift 2
            ;;
        --wait-seconds)
            require_arg "$1" "${2:-}"
            SAMPLE_WAIT_SECONDS="$2"
            shift 2
            ;;
        --prewarm-log)
            require_arg "$1" "${2:-}"
            PREWARM_LOG="$2"
            shift 2
            ;;
        --prewarm-summary)
            require_arg "$1" "${2:-}"
            PREWARM_SUMMARY="$2"
            shift 2
            ;;
        --log)
            require_arg "$1" "${2:-}"
            SAMPLE_LOG="$2"
            shift 2
            ;;
        --perf-summary)
            require_arg "$1" "${2:-}"
            SAMPLE_SUMMARY="$2"
            shift 2
            ;;
        --view-pose)
            require_arg "$1" "${2:-}"
            VIEW_POSE="$2"
            shift 2
            ;;
        --seed)
            require_arg "$1" "${2:-}"
            SEED="$2"
            shift 2
            ;;
        --chunk-x)
            require_arg "$1" "${2:-}"
            CHUNK_X="$2"
            shift 2
            ;;
        --chunk-z)
            require_arg "$1" "${2:-}"
            CHUNK_Z="$2"
            shift 2
            ;;
        --render-distance)
            require_arg "$1" "${2:-}"
            RENDER_DISTANCE="$2"
            shift 2
            ;;
        --generation-profile)
            require_arg "$1" "${2:-}"
            GENERATION_PROFILE="$2"
            shift 2
            ;;
        --terrain-presentation)
            require_arg "$1" "${2:-}"
            TERRAIN_PRESENTATION="$2"
            shift 2
            ;;
        --sample-mode)
            require_arg "$1" "${2:-}"
            SAMPLE_MODE="$2"
            shift 2
            ;;
        --perf-orbit-speed)
            require_arg "$1" "${2:-}"
            ORBIT_SPEED="$2"
            shift 2
            ;;
        --perf-flight-speed)
            require_arg "$1" "${2:-}"
            FLIGHT_SPEED="$2"
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
        ;;
    debug|Debug|DEBUG)
        BUILD_TYPE=debug
        ;;
    *)
        mclone_die "MCLONE_ANDROID_XR_BUILD_TYPE must be 'release' or 'debug'"
        ;;
esac
validate_positive_integer "--prewarm-seconds" "$PREWARM_SECONDS"
validate_positive_integer "--prewarm-wait-seconds" "$PREWARM_WAIT_SECONDS"
validate_positive_integer "--perf-seconds" "$SAMPLE_SECONDS"
validate_positive_integer "--wait-seconds" "$SAMPLE_WAIT_SECONDS"
validate_positive_integer "--render-distance" "$RENDER_DISTANCE"
case "$SAMPLE_MODE" in
    orbit|flight)
        ;;
    *)
        mclone_die "--sample-mode must be 'orbit' or 'flight'"
        ;;
esac
case "$TERRAIN_PRESENTATION" in
    exact-only|composed)
        ;;
    *)
        mclone_die "--terrain-presentation must be 'exact-only' or 'composed'"
        ;;
esac

cd "$REPO_ROOT"
ADB="$(mclone_android_tool adb platform-tools/adb)"
"$ADB" start-server >/dev/null
if [[ -z "$SERIAL" ]]; then
    SERIAL="$(mclone_detect_quest_serial || true)"
fi
if [[ -z "$SERIAL" ]]; then
    mclone_report_no_quest_found
fi
mclone_wait_for_boot "$SERIAL" "$BOOT_TIMEOUT_SECONDS"
mclone_note "Using $(mclone_device_summary "$SERIAL")"

mclone_note "Removing Quest persisted guardrail world dir: $WORLD_DIR"
"$ADB" -s "$SERIAL" shell "rm -rf $(mclone_xr_shell_quote "$WORLD_DIR")"

build_args=("--$BUILD_TYPE" --serial "$SERIAL")
if [[ "$SKIP_BUILD" == "1" ]]; then
    build_args+=(--skip-build)
fi
if [[ "$STAGE_ASSETS" == "0" ]]; then
    build_args+=(--skip-assets)
elif [[ -n "$ASSET_PACK" ]]; then
    build_args+=(--asset-pack "$ASSET_PACK")
fi

scene_args=(
    --world-dir "$WORLD_DIR"
    --view-pose "$VIEW_POSE"
    --seed "$SEED"
    --chunk-x "$CHUNK_X"
    --chunk-z "$CHUNK_Z"
    --render-distance "$RENDER_DISTANCE"
    --app-arg --generation-profile
    --app-arg "$GENERATION_PROFILE"
    --app-arg --terrain-presentation
    --app-arg "$TERRAIN_PRESENTATION"
    --render-compile-workers "$RENDER_COMPILE_WORKERS"
    --day-time "$DAY_TIME"
    --freeze-time
    --xr-render-completed-result-accept-budget "$RENDER_COMPLETED_RESULT_ACCEPT_BUDGET"
    --xr-render-section-upload-budget "$RENDER_SECTION_UPLOAD_BUDGET"
    --xr-render-section-accept-budget "$RENDER_SECTION_ACCEPT_BUDGET"
)

mclone_note "Prewarming persisted Quest world"
bash "$ANDROID_XR_DIR/validate-quest-openxr.sh" \
    "${build_args[@]}" \
    "${scene_args[@]}" \
    --perf-seconds "$PREWARM_SECONDS" \
    --perf-settled-stationary \
    --wait-seconds "$PREWARM_WAIT_SECONDS" \
    --perf-summary "$PREWARM_SUMMARY" \
    --log "$PREWARM_LOG"

sample_args=()
case "$SAMPLE_MODE" in
    orbit)
        sample_args+=(--perf-settled-orbit --perf-orbit-speed "$ORBIT_SPEED")
        ;;
    flight)
        sample_args+=(--perf-flight --perf-flight-speed "$FLIGHT_SPEED")
        ;;
esac

mclone_note "Reopening persisted Quest world for RD${RENDER_DISTANCE} ${SAMPLE_MODE} guardrail sample"
bash "$ANDROID_XR_DIR/validate-quest-openxr.sh" \
    --"$BUILD_TYPE" \
    --serial "$SERIAL" \
    --skip-build \
    --skip-assets \
    "${scene_args[@]}" \
    --perf-seconds "$SAMPLE_SECONDS" \
    "${sample_args[@]}" \
    --perf-metrics \
    --wait-seconds "$SAMPLE_WAIT_SECONDS" \
    --perf-summary "$SAMPLE_SUMMARY" \
    --log "$SAMPLE_LOG"

mclone_note "Quest persisted-world guardrail complete"
mclone_note "Prewarm summary: $PREWARM_SUMMARY"
mclone_note "Reopen summary: $SAMPLE_SUMMARY"
