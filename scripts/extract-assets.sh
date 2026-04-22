#!/usr/bin/env bash
# Extract renderer-bootstrap assets from a Minecraft client.jar that was
# already downloaded by decompile-mc.sh. Filtered to the subset we actually
# need (see docs/assets-plan.md): textures, block models, blockstates,
# structure NBTs, pack metadata. Idempotent.
#
# Usage:
#   ./extract-assets.sh [VERSION] [--out DIR] [--force]
#
# Args:
#   VERSION    Minecraft version id. Default: 1.17.1
#
# Flags:
#   --out DIR  Directory containing client.jar (same as decompile-mc.sh --out).
#              Default: <repo>/reference/minecraft-<VERSION>
#   --force    Re-extract even if extracted/ already has content.
#
# Prereqs: unzip.

set -euo pipefail

VERSION=""
OUT_DIR=""
FORCE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --out)   OUT_DIR="$2"; shift 2 ;;
        --force) FORCE=1; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0 ;;
        -*)
            echo "Unknown flag: $1" >&2; exit 2 ;;
        *)
            if [ -z "$VERSION" ]; then VERSION="$1"; else
                echo "Unexpected argument: $1" >&2; exit 2
            fi
            shift ;;
    esac
done

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)

VERSION="${VERSION:-1.17.1}"
OUT_DIR="${OUT_DIR:-$REPO_ROOT/reference/minecraft-${VERSION}}"

command -v unzip >/dev/null 2>&1 || { echo "Missing prereq: unzip" >&2; exit 1; }

JAR="$OUT_DIR/client.jar"
DEST="$OUT_DIR/extracted"
MARKER="$DEST/version.json"

log() { printf '[extract-assets] %s\n' "$*"; }

if [ ! -f "$JAR" ]; then
    echo "client.jar not found at $JAR — run decompile-mc.sh first." >&2
    exit 1
fi

if [ -f "$MARKER" ] && [ "$FORCE" != 1 ]; then
    log "Already extracted at $DEST (pass --force to redo)."
    exit 0
fi

if [ "$FORCE" = 1 ] && [ -d "$DEST" ]; then
    log "Removing existing $DEST (forced)..."
    rm -rf "$DEST"
fi

mkdir -p "$DEST"
log "Extracting filtered assets from $JAR..."
unzip -q -o "$JAR" \
    'assets/minecraft/textures/*' \
    'assets/minecraft/models/*' \
    'assets/minecraft/blockstates/*' \
    'data/minecraft/structures/*' \
    'pack.png' 'version.json' \
    -d "$DEST"

SIZE=$(du -sh "$DEST" | cut -f1)
FILES=$(find "$DEST" -type f | wc -l)
log "Extracted $FILES files, $SIZE -> $DEST"
