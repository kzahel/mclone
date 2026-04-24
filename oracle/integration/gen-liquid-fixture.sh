#!/usr/bin/env bash
# End-to-end dynamic liquid oracle harness: fetch the 1.17.1 server jar if
# missing, run a scripted liquid scenario, then dump a bounded fixture.
#
# Usage:
#   ./gen-liquid-fixture.sh --scenario <path> --out <path>
#                           [--timeout SECONDS] [--keep-world]

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)

SCENARIO=""
OUT=""
TIMEOUT="180"
KEEP_WORLD=0

while [ $# -gt 0 ]; do
    case "$1" in
        --scenario)    SCENARIO="$2"; shift 2 ;;
        --out)         OUT="$2"; shift 2 ;;
        --timeout)     TIMEOUT="$2"; shift 2 ;;
        --keep-world)  KEEP_WORLD=1; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$SCENARIO" ] || [ -z "$OUT" ]; then
    echo "--scenario and --out are required" >&2
    exit 2
fi

log() { printf '[gen-liquid-fixture] %s\n' "$*"; }

if [ ! -f "${REPO_ROOT}/reference/minecraft-1.17.1/server.jar" ]; then
    log "Fetching server jar..."
    "${REPO_ROOT}/scripts/fetch-server-jar.sh" 1.17.1
fi

log "Running liquid scenario ${SCENARIO}"
WORK_DIR=$("${SCRIPT_DIR}/run-liquid-server.sh" --scenario "$SCENARIO" --timeout "$TIMEOUT" | tail -n 1)

if [ ! -d "$WORK_DIR/world/region" ]; then
    echo "liquid server runner produced no region dir: $WORK_DIR/world/region" >&2
    exit 1
fi

log "Decoding liquid fixture -> ${OUT}"
mkdir -p "$(dirname "$OUT")"
node \
    --disable-warning=ExperimentalWarning \
    --experimental-transform-types \
    --experimental-loader "${REPO_ROOT}/scripts/node-ts-loader.mjs" \
    "${SCRIPT_DIR}/dump-liquid-fixture.ts" \
    --region-dir "${WORK_DIR}/world/region" \
    --scenario "$SCENARIO" \
    --out "$OUT"

if [ "$KEEP_WORLD" -eq 0 ]; then
    log "Cleaning work dir $WORK_DIR"
    rm -rf "$WORK_DIR"
fi

log "Fixture: $OUT"
