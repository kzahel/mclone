#!/usr/bin/env bash
# Run the official 1.17.1 server jar headless with a pinned seed so the
# integration oracle can read the generated region files.
#
# Usage:
#   ./run-server.sh --seed <long> [--work-dir DIR] [--timeout SECONDS] [--scheduler-pins]
#
# The script writes server.properties (from the template) + eula.txt into an
# isolated working directory, spawns `java -jar server.jar --nogui`, waits for
# the "Done (" startup marker, sends "stop" on stdin, then waits for clean
# shutdown. Region files end up under <work-dir>/world/region/*.mca.
#
# Exits non-zero if the server never reaches "Done (" within --timeout seconds.

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)
SERVER_JAR="${REPO_ROOT}/reference/minecraft-1.17.1/server.jar"
PROPS_TEMPLATE="${SCRIPT_DIR}/server/server.properties.template"
EULA_TEMPLATE="${SCRIPT_DIR}/server/eula.txt"

SEED=""
WORK_DIR=""
TIMEOUT="180"
SCHEDULER_PINS=0

while [ $# -gt 0 ]; do
    case "$1" in
        --seed)           SEED="$2"; shift 2 ;;
        --work-dir)       WORK_DIR="$2"; shift 2 ;;
        --timeout)        TIMEOUT="$2"; shift 2 ;;
        --scheduler-pins) SCHEDULER_PINS=1; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        *)
            echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$SEED" ]; then
    echo "--seed is required" >&2
    exit 2
fi

if [ ! -f "$SERVER_JAR" ]; then
    echo "Server jar not found at $SERVER_JAR" >&2
    echo "Run scripts/fetch-server-jar.sh 1.17.1 first." >&2
    exit 1
fi

if [ ! -f "$PROPS_TEMPLATE" ]; then
    echo "Missing server.properties template at $PROPS_TEMPLATE" >&2
    exit 1
fi

command -v java >/dev/null 2>&1 || { echo "Missing prereq: java (17+)" >&2; exit 1; }

WORK_DIR="${WORK_DIR:-${REPO_ROOT}/reference/minecraft-1.17.1/server-work-${SEED}}"

log() { printf '[oracle-server] %s\n' "$*" >&2; }

# Region files accumulate state across runs, so a stale world would defeat the
# pinned-seed guarantee. Always start from a clean directory.
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
sed "s/__SEED__/${SEED}/" "$PROPS_TEMPLATE" > "$WORK_DIR/server.properties"
cp "$EULA_TEMPLATE" "$WORK_DIR/eula.txt"

LOG_FILE="${WORK_DIR}/startup.log"
FIFO_IN="${WORK_DIR}/.stdin.fifo"
mkfifo "$FIFO_IN"

log "Starting server headless for seed ${SEED}"

# Hold the write end of the FIFO open in this parent shell so the server's
# stdin doesn't EOF before we send "stop". Open for read+write so the call
# doesn't block waiting for the reader (the server) to attach first.
exec 9<>"$FIFO_IN"

(
    cd "$WORK_DIR"
    JAVA_ARGS=()
    if [ "$SCHEDULER_PINS" -eq 1 ]; then
        JAVA_ARGS+=("-XX:+UnlockExperimentalVMOptions" "-XX:hashCode=3" "-XX:ActiveProcessorCount=2")
    fi
    exec java "${JAVA_ARGS[@]}" -Xmx2G -Xms1G -jar "$SERVER_JAR" --nogui <"$FIFO_IN" >"$LOG_FILE" 2>&1
) &
SERVER_PID=$!

cleanup() {
    exec 9>&- 2>/dev/null || true
    if kill -0 "$SERVER_PID" 2>/dev/null; then
        log "Forcing server shutdown"
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$FIFO_IN"
}
trap cleanup EXIT INT TERM

STOP_SENT=0
SAW_DONE=0
START=$(date +%s)

while kill -0 "$SERVER_PID" 2>/dev/null; do
    if [ "$STOP_SENT" -eq 0 ]; then
        if grep -qE 'Done \([0-9.]+s\)!' "$LOG_FILE" 2>/dev/null; then
            SAW_DONE=1
            log "Server finished startup; sending stop"
            printf 'stop\n' >&9
            STOP_SENT=1
        fi
    fi

    NOW=$(date +%s)
    if [ $((NOW - START)) -ge "$TIMEOUT" ]; then
        log "Timeout after ${TIMEOUT}s waiting for startup / shutdown"
        cat "$LOG_FILE" >&2 || true
        exit 1
    fi

    sleep 1
done

wait "$SERVER_PID" 2>/dev/null || true

if [ "$SAW_DONE" -eq 0 ]; then
    log "Server never reached 'Done (' startup marker"
    cat "$LOG_FILE" >&2 || true
    exit 1
fi

REGION_DIR="${WORK_DIR}/world/region"
if [ ! -d "$REGION_DIR" ] || [ -z "$(ls -A "$REGION_DIR" 2>/dev/null)" ]; then
    log "No region files produced in $REGION_DIR"
    cat "$LOG_FILE" >&2 || true
    exit 1
fi

log "Region files: $REGION_DIR"
printf '%s\n' "$WORK_DIR"
