#!/bin/sh
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MODE=${1:-play}
BIN="$HERE/mclone-native-client"
INSTANCE_ROOT=${MCLONE_DECK_INSTANCE_ROOT:-"$HOME/.local/state/mclone-deck/instances"}
INSTANCE_LOCK="$INSTANCE_ROOT/interactive.lock"
INSTANCE_OWNER="$INSTANCE_ROOT/interactive.owner"

export MCLONE_ASSET_MODE=pack-only
export MCLONE_ASSET_PACK="$HERE/assets/extracted.zip"
export MCLONE_WORLD_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/mclone-deck/worlds"

RUN_ID=${2:-${MCLONE_DECK_RESULT_ID:-"manual-$(date -u +%Y%m%dT%H%M%SZ)"}}
RESULT_ROOT=${3:-${MCLONE_DECK_RESULT_ROOT:-"${XDG_STATE_HOME:-$HOME/.local/state}/mclone-deck/results"}}
RESULT_DIR="$RESULT_ROOT/$RUN_ID"

process_start_ticks()
{
    pid=$1
    [ -r "/proc/$pid/stat" ] || return 1
    awk '{print $22}' "/proc/$pid/stat" 2>/dev/null
}

read_instance_owner()
{
    OWNER_PID=
    OWNER_START_TICKS=
    [ -r "$INSTANCE_OWNER" ] || return 1
    IFS=' ' read -r OWNER_PID OWNER_START_TICKS <"$INSTANCE_OWNER" || return 1
    case "$OWNER_PID:$OWNER_START_TICKS" in
        *[!0-9:]*|:*|*:)
            return 1
            ;;
    esac
}

owned_process_is_alive()
{
    [ -n "${OWNER_PID:-}" ] || return 1
    [ -n "${OWNER_START_TICKS:-}" ] || return 1
    current_start_ticks=$(process_start_ticks "$OWNER_PID") || return 1
    [ "$current_start_ticks" = "$OWNER_START_TICKS" ]
}

remove_owner_if_unchanged()
{
    expected_pid=$1
    expected_start_ticks=$2
    read_instance_owner || return 0
    if [ "$OWNER_PID" = "$expected_pid" ] &&
        [ "$OWNER_START_TICKS" = "$expected_start_ticks" ]; then
        rm -f -- "$INSTANCE_OWNER"
    fi
}

claim_interactive_instance()
{
    mkdir -p "$INSTANCE_ROOT"
    exec 9>"$INSTANCE_LOCK"
    if ! /usr/bin/flock -n 9; then
        if read_instance_owner && owned_process_is_alive; then
            echo \
                "mclone Steam Deck interactive instance already owns PID $OWNER_PID" \
                >&2
        else
            echo "mclone Steam Deck interactive instance is already starting" >&2
        fi
        return 75
    fi

    owner_start_ticks=$(process_start_ticks "$$") || {
        echo "cannot read Steam Deck launcher process identity" >&2
        return 1
    }
    owner_temp="$INSTANCE_OWNER.$$"
    printf '%s %s\n' "$$" "$owner_start_ticks" >"$owner_temp"
    mv -f -- "$owner_temp" "$INSTANCE_OWNER"
}

stop_interactive_instance()
{
    if ! read_instance_owner; then
        rm -f -- "$INSTANCE_OWNER"
        echo "No owned mclone Steam Deck interactive instance is running"
        return 0
    fi
    if ! owned_process_is_alive; then
        remove_owner_if_unchanged "$OWNER_PID" "$OWNER_START_TICKS"
        echo "Removed stale mclone Steam Deck interactive ownership"
        return 0
    fi

    stopped_pid=$OWNER_PID
    stopped_start_ticks=$OWNER_START_TICKS
    echo "Stopping owned mclone Steam Deck interactive PID $stopped_pid"
    kill -TERM "$stopped_pid"
    attempts=0
    while owned_process_is_alive && [ "$attempts" -lt 50 ]; do
        sleep 0.1
        attempts=$((attempts + 1))
    done
    if owned_process_is_alive; then
        echo \
            "Owned mclone PID $stopped_pid ignored TERM; sending KILL" \
            >&2
        kill -KILL "$stopped_pid"
        attempts=0
        while owned_process_is_alive && [ "$attempts" -lt 20 ]; do
            sleep 0.1
            attempts=$((attempts + 1))
        done
    fi
    if owned_process_is_alive; then
        echo "Owned mclone PID $stopped_pid did not exit" >&2
        return 1
    fi
    remove_owner_if_unchanged "$stopped_pid" "$stopped_start_ticks"
}

case "$MODE" in
    play)
        claim_interactive_instance
        exec "$BIN" --platform-profile steamos --menu
        ;;
    smoke|perf|gamescope-repro)
        ;;
    stop)
        stop_interactive_instance
        exit 0
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
        --platform-profile steamos \
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
elif [ "$MODE" = gamescope-repro ]; then
    "$BIN" \
        --platform-profile steamos \
        --start-in-world true \
        --startup-wait idle \
        --transient \
        --seed 12345 \
        --chunk-x 0 \
        --chunk-z 0 \
        --render-distance 13 \
        --debug-passive-showcase false \
        --window-camera-eye 8,196,8 \
        --window-camera-target 8,64,8 \
        --window-frame-report "$RESULT_DIR/window.json" \
        --window-frame-report-frames 72000 \
        >"$RESULT_DIR/window.stdout.log" \
        2>"$RESULT_DIR/window.stderr.log"
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
        --platform-profile steamos \
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
