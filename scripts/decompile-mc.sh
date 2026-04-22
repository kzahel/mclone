#!/usr/bin/env bash
# Decompile a Minecraft Java Edition version to readable source using Mojang's
# official obfuscation mappings, SpecialSource for remapping, and Vineflower
# for decompilation. Idempotent: each step skips if its output already exists.
#
# Usage:
#   ./decompile-mc.sh [VERSION] [--server] [--out DIR] [--force] [--parchment] [--no-assets]
#
# Args:
#   VERSION      Minecraft version id, as listed in Mojang's version_manifest_v2.
#                Default: 1.17.1
#
# Flags:
#   --server     Use server.jar + server mappings instead of client.
#   --out DIR    Output directory. Default: <repo>/reference/minecraft-<VERSION>
#   --force      Redo steps even if outputs exist.
#   --parchment  After decompile, apply Parchment parameter names (community
#                mapping) via apply-parchment.py. Idempotent.
#   --no-assets  Skip the asset extraction step (extract-assets.sh).
#
# Prereqs: java (17+ recommended), curl, jq, sha1sum, unzip. python3 if --parchment.

set -euo pipefail

SPECIAL_SOURCE_URL="https://repo1.maven.org/maven2/net/md-5/SpecialSource/1.11.4/SpecialSource-1.11.4-shaded.jar"
VINEFLOWER_URL="https://github.com/Vineflower/vineflower/releases/download/1.11.2/vineflower-1.11.2.jar"
MANIFEST_URL="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

VERSION=""
SIDE="client"
OUT_DIR=""
FORCE=0
PARCHMENT=0
EXTRACT_ASSETS=1

while [ $# -gt 0 ]; do
    case "$1" in
        --server)    SIDE="server"; shift ;;
        --client)    SIDE="client"; shift ;;
        --out)       OUT_DIR="$2"; shift 2 ;;
        --force)     FORCE=1; shift ;;
        --parchment) PARCHMENT=1; shift ;;
        --no-assets) EXTRACT_ASSETS=0; shift ;;
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

for bin in java curl jq sha1sum; do
    command -v "$bin" >/dev/null 2>&1 || { echo "Missing prereq: $bin" >&2; exit 1; }
done
if [ "$EXTRACT_ASSETS" = 1 ] && [ "$SIDE" = "client" ]; then
    command -v unzip >/dev/null 2>&1 || { echo "Missing prereq: unzip (needed for asset extraction — pass --no-assets to skip)" >&2; exit 1; }
fi

log()  { printf '[mc-decompile] %s\n' "$*"; }
need() { [ "$FORCE" = 1 ] || [ ! -e "$1" ]; }

mkdir -p "$OUT_DIR/tools"
cd "$OUT_DIR"

# 1. Top-level manifest
if need version_manifest_v2.json; then
    log "Fetching version manifest..."
    curl -sSfL -o version_manifest_v2.json "$MANIFEST_URL"
fi

# 2. Per-version manifest
if need "${VERSION}.json"; then
    log "Locating ${VERSION} in manifest..."
    VERSION_URL=$(jq -r --arg v "$VERSION" \
        '.versions[] | select(.id == $v) | .url' version_manifest_v2.json)
    if [ -z "$VERSION_URL" ] || [ "$VERSION_URL" = "null" ]; then
        echo "Version '$VERSION' not found in Mojang manifest." >&2
        echo "Try one of:" >&2
        jq -r '.versions[] | select(.type == "release") | .id' \
            version_manifest_v2.json | head -20 >&2
        exit 1
    fi
    log "Fetching per-version manifest..."
    curl -sSfL -o "${VERSION}.json" "$VERSION_URL"
fi

# 3. Jar + mappings (both SHA1-verified)
JAR_URL=$(jq -r ".downloads.${SIDE}.url" "${VERSION}.json")
JAR_SHA=$(jq -r ".downloads.${SIDE}.sha1" "${VERSION}.json")
MAP_URL=$(jq -r ".downloads.${SIDE}_mappings.url // empty" "${VERSION}.json")
MAP_SHA=$(jq -r ".downloads.${SIDE}_mappings.sha1 // empty" "${VERSION}.json")

if [ -z "$MAP_URL" ]; then
    echo "No ${SIDE}_mappings available for ${VERSION}." >&2
    echo "Official mappings are only published for 1.14.4+." >&2
    exit 1
fi

if need "${SIDE}.jar"; then
    log "Downloading ${SIDE}.jar..."
    curl -sSfL -o "${SIDE}.jar" "$JAR_URL"
    echo "${JAR_SHA}  ${SIDE}.jar" | sha1sum -c
fi
if need "${SIDE}.txt"; then
    log "Downloading ${SIDE} mappings..."
    curl -sSfL -o "${SIDE}.txt" "$MAP_URL"
    echo "${MAP_SHA}  ${SIDE}.txt" | sha1sum -c
fi

# 4. Tools
if need tools/SpecialSource.jar; then
    log "Downloading SpecialSource..."
    curl -sSfL -o tools/SpecialSource.jar "$SPECIAL_SOURCE_URL"
fi
if need tools/vineflower.jar; then
    log "Downloading Vineflower..."
    curl -sSfL -o tools/vineflower.jar "$VINEFLOWER_URL"
fi

# 5. Remap (Mojang ships Proguard-format mappings; SpecialSource's default
#    direction is what we want, do NOT pass --reverse)
DEOBF_JAR="${SIDE}-deobf.jar"
if need "$DEOBF_JAR"; then
    log "Remapping ${SIDE}.jar -> ${DEOBF_JAR}..."
    java -jar tools/SpecialSource.jar \
        --in-jar "${SIDE}.jar" \
        --out-jar "$DEOBF_JAR" \
        --srg-in "${SIDE}.txt" \
        --kill-lvt
fi

# 6. Decompile
SRC_DIR="src"
if need "$SRC_DIR/net" ; then
    log "Decompiling with Vineflower (this takes a few minutes)..."
    mkdir -p "$SRC_DIR"
    java -Xmx4g -jar tools/vineflower.jar --silent \
        "$DEOBF_JAR" "$SRC_DIR/"
fi

# 7. Optionally apply Parchment parameter names
if [ "$PARCHMENT" = 1 ]; then
    command -v python3 >/dev/null 2>&1 || { echo "python3 required for --parchment" >&2; exit 1; }
    log "Applying Parchment parameter mappings..."
    python3 "$SCRIPT_DIR/apply-parchment.py" "$OUT_DIR/$SRC_DIR" --mc "$VERSION"
fi

# 8. Extract renderer-bootstrap assets (client only). Idempotent.
if [ "$EXTRACT_ASSETS" = 1 ] && [ "$SIDE" = "client" ]; then
    log "Extracting assets..."
    EXTRACT_ARGS=("$VERSION" "--out" "$OUT_DIR")
    [ "$FORCE" = 1 ] && EXTRACT_ARGS+=("--force")
    "$SCRIPT_DIR/extract-assets.sh" "${EXTRACT_ARGS[@]}"
fi

log ""
log "Done. Source tree: $OUT_DIR/$SRC_DIR"
log "Class count:  $(find "$SRC_DIR" -name '*.java' | wc -l) .java files"
log "Worldgen entry: $SRC_DIR/net/minecraft/world/level/levelgen/"
