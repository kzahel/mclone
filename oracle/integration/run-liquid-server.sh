#!/usr/bin/env bash
# Run the official 1.17.1 server jar with a generated datapack that sets up a
# liquid oracle scenario, counts exact server ticks, saves, and stops.
#
# Usage:
#   ./run-liquid-server.sh --scenario <path> [--work-dir DIR] [--timeout SECONDS]

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)
SERVER_JAR="${REPO_ROOT}/reference/minecraft-1.17.1/server.jar"
PROPS_TEMPLATE="${SCRIPT_DIR}/server/server.properties.template"
EULA_TEMPLATE="${SCRIPT_DIR}/server/eula.txt"

SCENARIO=""
WORK_DIR=""
TIMEOUT="180"

while [ $# -gt 0 ]; do
    case "$1" in
        --scenario) SCENARIO="$2"; shift 2 ;;
        --work-dir) WORK_DIR="$2"; shift 2 ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$SCENARIO" ]; then
    echo "--scenario is required" >&2
    exit 2
fi

if [ ! -f "$SERVER_JAR" ]; then
    echo "Server jar not found at $SERVER_JAR" >&2
    echo "Run scripts/fetch-server-jar.sh 1.17.1 first." >&2
    exit 1
fi

command -v java >/dev/null 2>&1 || { echo "Missing prereq: java (17+)" >&2; exit 1; }

SCENARIO_NAME=$(basename "$SCENARIO" .json)
SCENARIO_NAME=${SCENARIO_NAME//[!A-Za-z0-9_-]/-}
WORK_DIR="${WORK_DIR:-${REPO_ROOT}/reference/minecraft-1.17.1/liquid-server-work-${SCENARIO_NAME}}"

log() { printf '[liquid-oracle-server] %s\n' "$*" >&2; }

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

node \
    --disable-warning=ExperimentalWarning \
    --experimental-transform-types \
    --experimental-loader "${REPO_ROOT}/scripts/node-ts-loader.mjs" \
    "${SCRIPT_DIR}/prepare-liquid-server.ts" \
    --scenario "$SCENARIO" \
    --work-dir "$WORK_DIR" \
    --props-template "$PROPS_TEMPLATE" \
    --eula-template "$EULA_TEMPLATE"

LOG_FILE="${WORK_DIR}/startup.log"
FIFO_IN="${WORK_DIR}/.stdin.fifo"
mkfifo "$FIFO_IN"

log "Starting server for scenario ${SCENARIO}"
exec 9<>"$FIFO_IN"

(
    cd "$WORK_DIR"
    exec java -Xmx2G -Xms1G -jar "$SERVER_JAR" --nogui <"$FIFO_IN" >"$LOG_FILE" 2>&1
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

SAW_DONE=0
START=$(date +%s)

while kill -0 "$SERVER_PID" 2>/dev/null; do
    if [ "$SAW_DONE" -eq 0 ] && grep -qE 'Done \([0-9.]+s\)!' "$LOG_FILE" 2>/dev/null; then
        SAW_DONE=1
        log "Server reached startup marker; waiting for datapack stop"
    fi

    NOW=$(date +%s)
    if [ $((NOW - START)) -ge "$TIMEOUT" ]; then
        log "Timeout after ${TIMEOUT}s waiting for scenario completion"
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
