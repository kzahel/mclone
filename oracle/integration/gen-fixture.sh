#!/usr/bin/env bash
# End-to-end integration oracle harness: fetch the 1.17.1 server jar if
# missing, run it headless against a pinned seed, then decode the requested
# chunks into a committable JSON fixture.
#
# Usage:
#   ./gen-fixture.sh --seed <long> --chunks <x,z,x,z,...> --out <path>
#                    [--timeout SECONDS] [--keep-world] [--scheduler-pins]
#
# Prereqs: Java 17, Node 22.6+ (for native TS strip-types), curl, jq, sha1sum.

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)

SEED=""
CHUNKS=""
OUT=""
TIMEOUT="180"
KEEP_WORLD=0
SCHEDULER_PINS=0

while [ $# -gt 0 ]; do
    case "$1" in
        --seed)           SEED="$2"; shift 2 ;;
        --chunks)         CHUNKS="$2"; shift 2 ;;
        --out)            OUT="$2"; shift 2 ;;
        --timeout)        TIMEOUT="$2"; shift 2 ;;
        --keep-world)     KEEP_WORLD=1; shift ;;
        --scheduler-pins) SCHEDULER_PINS=1; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$SEED" ] || [ -z "$CHUNKS" ] || [ -z "$OUT" ]; then
    echo "--seed, --chunks, and --out are all required" >&2
    exit 2
fi

log() { printf '[gen-fixture] %s\n' "$*"; }

# 1. Ensure the server jar is present.
if [ ! -f "${REPO_ROOT}/reference/minecraft-1.17.1/server.jar" ]; then
    log "Fetching server jar..."
    "${REPO_ROOT}/scripts/fetch-server-jar.sh" 1.17.1
fi

# 2. Run the server headless against the pinned seed.
log "Running server (seed=${SEED})..."
RUN_SERVER_ARGS=(--seed "$SEED" --timeout "$TIMEOUT")
if [ "$SCHEDULER_PINS" -eq 1 ]; then
    RUN_SERVER_ARGS+=(--scheduler-pins)
fi
WORK_DIR=$("${SCRIPT_DIR}/run-server.sh" "${RUN_SERVER_ARGS[@]}" | tail -n 1)

if [ ! -d "$WORK_DIR/world/region" ]; then
    echo "server-runner produced no region dir: $WORK_DIR/world/region" >&2
    exit 1
fi

# 3. Decode the requested chunks into the fixture JSON.
log "Decoding chunks ${CHUNKS} -> ${OUT}"
mkdir -p "$(dirname "$OUT")"
node \
    --disable-warning=ExperimentalWarning \
    --experimental-transform-types \
    --experimental-loader "${REPO_ROOT}/scripts/node-ts-loader.mjs" \
    "${SCRIPT_DIR}/dump-chunks.ts" \
    --region-dir "${WORK_DIR}/world/region" \
    --seed "$SEED" \
    --chunks "$CHUNKS" \
    --out "$OUT"

# 4. Optionally keep the world directory (useful when iterating).
if [ "$KEEP_WORLD" -eq 0 ]; then
    log "Cleaning work dir $WORK_DIR"
    rm -rf "$WORK_DIR"
fi

log "Fixture: $OUT"
