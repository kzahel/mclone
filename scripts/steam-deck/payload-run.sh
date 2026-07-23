#!/bin/sh
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MODE=${1:-play}
BIN="$HERE/mclone-native-client"

export MCLONE_ASSET_MODE=pack-only
export MCLONE_ASSET_PACK="$HERE/assets/extracted.zip"
export MCLONE_WORLD_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/mclone-deck/worlds"

RUN_ID=${2:-${MCLONE_DECK_RESULT_ID:-"manual-$(date -u +%Y%m%dT%H%M%SZ)"}}
RESULT_ROOT=${3:-${MCLONE_DECK_RESULT_ROOT:-"${XDG_STATE_HOME:-$HOME/.local/state}/mclone-deck/results"}}
RESULT_DIR="$RESULT_ROOT/$RUN_ID"

case "$MODE" in
    play)
        exec "$BIN" --menu
        ;;
    smoke|perf)
        ;;
    *)
        echo "unknown Steam Deck payload mode: $MODE" >&2
        exit 2
        ;;
esac

mkdir -p "$RESULT_DIR"

finish()
{
    status=$?
    trap - EXIT
    completed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    printf '%s\n' "$status" >"$RESULT_DIR/status"
    printf \
        '{"schema":1,"mode":"%s","runId":"%s","exitStatus":%s,"completedAt":"%s"}\n' \
        "$MODE" "$RUN_ID" "$status" "$completed_at" \
        >"$RESULT_DIR/result.json"
    exit "$status"
}
trap finish EXIT

if [ "$MODE" = smoke ]; then
    "$BIN" \
        --screenshot "$RESULT_DIR/screenshot.png" \
        --width 1280 \
        --height 800 \
        --seed 12345 \
        --chunk-x 0 \
        --chunk-z 0 \
        --render-distance 2 \
        --day-time 6000 \
        --freeze-time \
        --transient \
        --startup-wait idle \
        --debug-passive-showcase false \
        --screenshot-eye 24,96,24 \
        --screenshot-target 8,64,8 \
        >"$RESULT_DIR/screenshot.stdout.log" \
        2>"$RESULT_DIR/screenshot.stderr.log"

    "$BIN" \
        --window-frame-report "$RESULT_DIR/present.json" \
        --window-frame-report-frames 300 \
        --start-in-world true \
        --startup-wait playable \
        --width 1280 \
        --height 800 \
        --seed 12345 \
        --render-distance 5 \
        --transient \
        --debug-passive-showcase false \
        >"$RESULT_DIR/present.stdout.log" \
        2>"$RESULT_DIR/present.stderr.log"
else
    "$BIN" \
        --timedemo \
        --timedemo-frames 600 \
        --width 1280 \
        --height 800 \
        --seed 12345 \
        --render-distance 5 \
        --debug-passive-showcase false \
        >"$RESULT_DIR/timedemo.json" \
        2>"$RESULT_DIR/timedemo.stderr.log"

    "$BIN" \
        --window-frame-report "$RESULT_DIR/present.json" \
        --window-frame-report-frames 3600 \
        --start-in-world true \
        --startup-wait playable \
        --width 1280 \
        --height 800 \
        --seed 12345 \
        --render-distance 10 \
        --transient \
        --simulation-cadence 60/20/60 \
        --debug-passive-showcase false \
        >"$RESULT_DIR/present.stdout.log" \
        2>"$RESULT_DIR/present.stderr.log"
fi
