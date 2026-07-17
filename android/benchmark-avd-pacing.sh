#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

AVD_NAME="${MCLONE_ANDROID_AVD:-jstorrent-tablet}"
SMOKE_SECONDS="${MCLONE_ANDROID_PACING_SMOKE_SECONDS:-60}"
WARMUP_SECONDS="${MCLONE_ANDROID_PACING_WARMUP_SECONDS:-5}"
SAMPLE_SECONDS="${MCLONE_ANDROID_PACING_SAMPLE_SECONDS:-15}"
CHURN_INTERVAL_SECONDS="${MCLONE_ANDROID_PACING_CHURN_INTERVAL_SECONDS:-3}"
CHURN_OFFSET_CHUNKS="${MCLONE_ANDROID_PACING_CHURN_OFFSET_CHUNKS:-16}"
RENDER_DISTANCE="${MCLONE_ANDROID_PACING_RENDER_DISTANCE:-5}"

common_args=(
    --avd "$AVD_NAME"
    --no-snapshot
    --reset-app-data
    --smoke-seconds "$SMOKE_SECONDS"
    --seed 12345
    --chunk-x 0
    --chunk-z 0
    --render-distance "$RENDER_DISTANCE"
    --generation-profile overworld
    --day-time 6000
    --freeze-time
    --pacing-perf-warmup-seconds "$WARMUP_SECONDS"
    --pacing-perf-seconds "$SAMPLE_SECONDS"
    --pacing-perf-churn-interval-seconds "$CHURN_INTERVAL_SECONDS"
    --pacing-perf-churn-offset-chunks "$CHURN_OFFSET_CHUNKS"
)

"$SCRIPT_DIR/validate-avd.sh" \
    "${common_args[@]}" \
    --gpu host \
    --pacing-perf-label avd-gpu-host \
    --screenshot /tmp/mclone-android-avd-pacing-host.png \
    --log /tmp/mclone-android-avd-pacing-host-logcat.txt

"$SCRIPT_DIR/validate-avd.sh" \
    "${common_args[@]}" \
    --skip-build \
    --gpu auto \
    --pacing-perf-label avd-gpu-auto \
    --screenshot /tmp/mclone-android-avd-pacing-auto.png \
    --log /tmp/mclone-android-avd-pacing-auto-logcat.txt

grep -F "MCLONE_ANDROID_PACING_PERF_SUMMARY" \
    /tmp/mclone-android-avd-pacing-host-logcat.txt
grep -F "MCLONE_ANDROID_PACING_PERF_SUMMARY" \
    /tmp/mclone-android-avd-pacing-auto-logcat.txt
