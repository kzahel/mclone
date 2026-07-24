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
    smoke|perf|gamescope-repro|perf-matrix|perf-matrix-smoke|perf-matrix-attribution|perf-matrix-workers)
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

sample_client_process()
{
    client_pid=$1
    output=$2
    clock_ticks=$(getconf CLK_TCK 2>/dev/null || printf '100')
    cpu_count=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1')
    printf '# clk_tck=%s cpu_count=%s\n' "$clock_ticks" "$cpu_count" >"$output"
    printf \
        'unix_seconds\tprocess_ticks\tsystem_ticks\trss_kib\tdata_kib\tthreads\tgpu_busy_percent\tgpu_clock_mhz\tgpu_temp_millic\n' \
        >>"$output"
    while kill -0 "$client_pid" 2>/dev/null; do
        unix_seconds=$(date +%s.%N)
        process_ticks=$(awk '{print $14 + $15}' "/proc/$client_pid/stat" 2>/dev/null || true)
        system_ticks=$(awk \
            'NR == 1 { total = 0; for (i = 2; i <= NF; i++) total += $i; print total }' \
            /proc/stat 2>/dev/null || true)
        rss_kib=$(awk '/^VmRSS:/ {print $2}' "/proc/$client_pid/status" 2>/dev/null || true)
        data_kib=$(awk '/^VmData:/ {print $2}' "/proc/$client_pid/status" 2>/dev/null || true)
        threads=$(awk '/^Threads:/ {print $2}' "/proc/$client_pid/status" 2>/dev/null || true)
        gpu_busy_percent=
        gpu_clock_mhz=
        gpu_temp_millic=
        for gpu_path in /sys/class/drm/card*/device/gpu_busy_percent; do
            [ -r "$gpu_path" ] || continue
            gpu_busy_percent=$(cat "$gpu_path" 2>/dev/null || true)
            gpu_device=${gpu_path%/gpu_busy_percent}
            if [ -r "$gpu_device/pp_dpm_sclk" ]; then
                gpu_clock_mhz=$(awk '/\*/ {gsub(/Mhz/, "", $2); print $2; exit}' \
                    "$gpu_device/pp_dpm_sclk" 2>/dev/null || true)
            fi
            for temp_path in "$gpu_device"/hwmon/hwmon*/temp1_input; do
                [ -r "$temp_path" ] || continue
                gpu_temp_millic=$(cat "$temp_path" 2>/dev/null || true)
                break
            done
            break
        done
        if [ -n "$process_ticks" ] && [ -n "$system_ticks" ]; then
            printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$unix_seconds" \
                "$process_ticks" \
                "$system_ticks" \
                "${rss_kib:-}" \
                "${data_kib:-}" \
                "${threads:-}" \
                "${gpu_busy_percent:-}" \
                "${gpu_clock_mhz:-}" \
                "${gpu_temp_millic:-}" \
                >>"$output"
        fi
        sleep 1
    done
}

run_matrix_case()
{
    case_name=$1
    kind=$2
    view=$3
    render_distance=$4
    eye=$5
    target=$6
    velocity=$7
    adaptive_publication=$8
    adaptive_admission=$9
    world_scale=${MATRIX_WORLD_SCALE:-100}
    freeze_fluids=${MATRIX_FREEZE_FLUIDS:-false}
    compile_capacity=${MATRIX_COMPILE_CAPACITY:-manual}
    compile_workers=${MATRIX_COMPILE_WORKERS:-1}
    compile_max_pending=${MATRIX_COMPILE_MAX_PENDING:-4}
    report="$RESULT_DIR/$case_name.json"
    system_samples="$RESULT_DIR/$case_name.system.tsv"

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$case_name" \
        "$kind" \
        "$view" \
        "$render_distance" \
        "$velocity" \
        "$adaptive_publication" \
        "$adaptive_admission" \
        "$world_scale" \
        "$freeze_fluids" \
        "$compile_capacity" \
        "$compile_workers" \
        "$compile_max_pending" \
        "$case_name.json" \
        >>"$RESULT_DIR/matrix.tsv"

    set -- \
        --platform-profile steamos \
        --start-in-world true \
        --startup-wait idle \
        --transient \
        --seed 12345 \
        --chunk-x 0 \
        --chunk-z 0 \
        --render-distance "$render_distance" \
        --simulation-cadence 60/20/60 \
        --adaptive-chunk-publication-budget "$adaptive_publication" \
        --adaptive-render-admission-budget "$adaptive_admission" \
        --window-world-render-scale "$world_scale" \
        --debug-passive-showcase false \
        --window-camera-eye "$eye" \
        --window-camera-target "$target" \
        --window-frame-report "$report"
    if [ "$freeze_fluids" = true ]; then
        set -- "$@" --window-freeze-scheduled-fluid-ticks
    fi
    if [ "$compile_capacity" = derived ]; then
        set -- "$@" --render-compile-capacity derived
    else
        set -- \
            "$@" \
            --render-compile-workers "$compile_workers" \
            --render-compile-max-pending-jobs "$compile_max_pending"
    fi
    if [ -n "$MATRIX_SECONDS" ]; then
        set -- "$@" --window-frame-report-seconds "$MATRIX_SECONDS"
    else
        set -- "$@" --window-frame-report-frames "$MATRIX_FRAMES"
    fi
    if [ "$velocity" != stationary ]; then
        set -- "$@" --window-camera-velocity "$velocity"
    fi

    "$BIN" "$@" \
        >"$RESULT_DIR/$case_name.stdout.log" \
        2>"$RESULT_DIR/$case_name.stderr.log" &
    client_pid=$!
    sample_client_process "$client_pid" "$system_samples" &
    sampler_pid=$!
    case_status=0
    wait "$client_pid" || case_status=$?
    kill "$sampler_pid" 2>/dev/null || true
    wait "$sampler_pid" 2>/dev/null || true
    if [ ! -s "$report" ]; then
        echo "steam-deck: matrix case $case_name did not write $report" >&2
        return 1
    fi
    return "$case_status"
}

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
elif [ "$MODE" = perf-matrix ] ||
    [ "$MODE" = perf-matrix-smoke ] ||
    [ "$MODE" = perf-matrix-attribution ] ||
    [ "$MODE" = perf-matrix-workers ]; then
    printf \
        'case\tkind\tview\trender_distance\tvelocity\tadaptive_publication\tadaptive_admission\tworld_scale\tfreeze_fluids\tcompile_capacity\tcompile_workers\tcompile_max_pending\treport\n' \
        >"$RESULT_DIR/matrix.tsv"

    MATRIX_WORLD_SCALE=100
    MATRIX_FREEZE_FLUIDS=false
    MATRIX_COMPILE_CAPACITY=manual
    MATRIX_COMPILE_WORKERS=1
    MATRIX_COMPILE_MAX_PENDING=4

    if [ "$MODE" = perf-matrix-smoke ]; then
        MATRIX_FRAMES=300
        MATRIX_SECONDS=
        run_matrix_case \
            stationary-topdown-rd5 \
            stationary \
            topdown \
            5 \
            8,196,8 \
            8,64,8 \
            stationary \
            true \
            false
        run_matrix_case \
            traversal-topdown-rd5 \
            traversal \
            topdown \
            5 \
            8,196,8 \
            8,64,8 \
            16,0,0 \
            true \
            false
        exit 0
    fi

    if [ "$MODE" = perf-matrix-attribution ]; then
        MATRIX_FRAMES=1800
        MATRIX_SECONDS=20
        run_matrix_case \
            stationary-topdown-rd13-native \
            stationary topdown 13 8,196,8 8,64,8 stationary true false
        MATRIX_WORLD_SCALE=50
        run_matrix_case \
            stationary-topdown-rd13-half \
            stationary topdown 13 8,196,8 8,64,8 stationary true false
        MATRIX_WORLD_SCALE=100
        MATRIX_FREEZE_FLUIDS=true
        run_matrix_case \
            stationary-topdown-rd13-frozen-fluid \
            stationary topdown 13 8,196,8 8,64,8 stationary true false
        MATRIX_FREEZE_FLUIDS=false
        run_matrix_case \
            traversal-topdown-rd13-native \
            traversal topdown 13 8,196,8 8,64,8 16,0,0 true false
        MATRIX_WORLD_SCALE=50
        run_matrix_case \
            traversal-topdown-rd13-half \
            traversal topdown 13 8,196,8 8,64,8 16,0,0 true false
        exit 0
    fi

    if [ "$MODE" = perf-matrix-workers ]; then
        MATRIX_FRAMES=1800
        MATRIX_SECONDS=20
        for render_distance in 10 13; do
            MATRIX_COMPILE_CAPACITY=manual
            MATRIX_COMPILE_WORKERS=1
            MATRIX_COMPILE_MAX_PENDING=4
            run_matrix_case \
                "traversal-topdown-rd${render_distance}-workers1" \
                traversal topdown "$render_distance" 8,196,8 8,64,8 16,0,0 true false
            MATRIX_COMPILE_WORKERS=2
            run_matrix_case \
                "traversal-topdown-rd${render_distance}-workers2" \
                traversal topdown "$render_distance" 8,196,8 8,64,8 16,0,0 true false
            MATRIX_COMPILE_CAPACITY=derived
            run_matrix_case \
                "traversal-topdown-rd${render_distance}-derived" \
                traversal topdown "$render_distance" 8,196,8 8,64,8 16,0,0 true false
        done
        exit 0
    fi

    MATRIX_FRAMES=1800
    MATRIX_SECONDS=20
    for render_distance in 5 8 10 13; do
        run_matrix_case \
            "stationary-oblique-rd$render_distance" \
            stationary \
            oblique \
            "$render_distance" \
            8,88,8 \
            72,64,8 \
            stationary \
            true \
            false
        run_matrix_case \
            "stationary-topdown-rd$render_distance" \
            stationary \
            topdown \
            "$render_distance" \
            8,196,8 \
            8,64,8 \
            stationary \
            true \
            false
    done

    for render_distance in 5 8 10 13; do
        run_matrix_case \
            "traversal-topdown-rd$render_distance" \
            traversal \
            topdown \
            "$render_distance" \
            8,196,8 \
            8,64,8 \
            16,0,0 \
            true \
            false
    done

    run_matrix_case \
        traversal-oblique-rd13 \
        traversal \
        oblique \
        13 \
        8,88,8 \
        72,64,8 \
        16,0,0 \
        true \
        false
    run_matrix_case \
        traversal-topdown-rd13-admission \
        traversal \
        topdown \
        13 \
        8,196,8 \
        8,64,8 \
        16,0,0 \
        true \
        true
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
