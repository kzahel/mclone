#!/usr/bin/env bash
# End-to-end generation creature oracle harness: fetch the 1.17.1 server jar if
# missing, run it headless against a pinned seed, then decode generated entity
# storage into a normalized creature-generation fixture or scan summary.
#
# Usage:
#   ./gen-creature-fixture.sh --seed <long> (--chunks <x,z,x,z,...> | --scan) --out <path>
#                             [--timeout SECONDS] [--keep-world]

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)

SEED=""
CHUNKS=""
OUT=""
TIMEOUT="180"
KEEP_WORLD=0
SCAN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --seed)        SEED="$2"; shift 2 ;;
        --chunks)      CHUNKS="$2"; shift 2 ;;
        --scan)        SCAN=1; shift ;;
        --out)         OUT="$2"; shift 2 ;;
        --timeout)     TIMEOUT="$2"; shift 2 ;;
        --keep-world)  KEEP_WORLD=1; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$SEED" ] || [ -z "$OUT" ]; then
    echo "--seed and --out are required" >&2
    exit 2
fi

if { [ -n "$CHUNKS" ] && [ "$SCAN" -eq 1 ]; } || { [ -z "$CHUNKS" ] && [ "$SCAN" -eq 0 ]; }; then
    echo "provide exactly one of --chunks or --scan" >&2
    exit 2
fi

log() { printf '[gen-creature-fixture] %s\n' "$*"; }

if [ ! -f "${REPO_ROOT}/reference/minecraft-1.17.1/server.jar" ]; then
    log "Fetching server jar..."
    "${REPO_ROOT}/scripts/fetch-server-jar.sh" 1.17.1
fi

log "Running server (seed=${SEED})..."
WORK_DIR=$("${SCRIPT_DIR}/run-server.sh" --seed "$SEED" --timeout "$TIMEOUT" | tail -n 1)

if [ ! -d "$WORK_DIR/world/region" ]; then
    echo "server-runner produced no region dir: $WORK_DIR/world/region" >&2
    exit 1
fi

log "Decoding creature entity data -> ${OUT}"
mkdir -p "$(dirname "$OUT")"

DUMP_ARGS=(
    --world-dir "${WORK_DIR}/world"
    --seed "$SEED"
    --out "$OUT"
)

if [ "$SCAN" -eq 1 ]; then
    DUMP_ARGS+=(--scan)
else
    DUMP_ARGS+=(--chunks "$CHUNKS")
fi

node \
    --disable-warning=ExperimentalWarning \
    --experimental-transform-types \
    --experimental-loader "${REPO_ROOT}/scripts/node-ts-loader.mjs" \
    "${SCRIPT_DIR}/dump-creature-fixture.ts" \
    "${DUMP_ARGS[@]}"

if [ "$KEEP_WORLD" -eq 0 ]; then
    log "Cleaning work dir $WORK_DIR"
    rm -rf "$WORK_DIR"
fi

log "Fixture: $OUT"
