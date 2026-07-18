#!/usr/bin/env bash
# Download and decompile a legacy Minecraft Alpha client with Ornithe Feather
# mappings. Generated Mojang code and jars remain under the gitignored
# reference/ tree.
#
# Usage:
#   ./scripts/decompile-alpha-mc.sh [VERSION] [--out DIR] [--force]
#
# Defaults:
#   VERSION  a1.1.2_01
#   DIR      reference/minecraft-<VERSION>
#
# The resulting source is a Feather client/server merge. For a1.1.2_01 the
# matching server protocol build is a0.2.1. Feather obtains the official client
# from Mojang and the preserved server build through its own version metadata.

set -euo pipefail

MANIFEST_URL="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
FEATHER_URL="https://github.com/OrnitheMC/feather.git"
# Pin the mapping graph and tool configuration used for the first Alpha study.
FEATHER_REF="${MCLONE_ALPHA_FEATHER_REF:-f9c6723b76d00cfffd48f10de317a4646919bbfc}"

VERSION=""
OUT_DIR=""
FORCE=0

while [ "$#" -gt 0 ]; do
    case "$1" in
        --out)
            [ "$#" -ge 2 ] || { printf '%s\n' "--out requires a directory" >&2; exit 2; }
            OUT_DIR="$2"
            shift 2
            ;;
        --force)
            FORCE=1
            shift
            ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# *//'
            exit 0
            ;;
        -*)
            printf 'Unknown flag: %s\n' "$1" >&2
            exit 2
            ;;
        *)
            if [ -n "$VERSION" ]; then
                printf 'Unexpected argument: %s\n' "$1" >&2
                exit 2
            fi
            VERSION="$1"
            shift
            ;;
    esac
done

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd -P)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd -P)
VERSION="${VERSION:-a1.1.2_01}"
OUT_DIR="${OUT_DIR:-$REPO_ROOT/reference/minecraft-${VERSION}}"
mkdir -p "$OUT_DIR"
OUT_DIR=$(cd -- "$OUT_DIR" &>/dev/null && pwd -P)

case "$OUT_DIR" in
    /|"$REPO_ROOT")
        printf 'Refusing unsafe output directory: %s\n' "$OUT_DIR" >&2
        exit 2
        ;;
esac

FEATHER_DIR="$OUT_DIR/tools/feather"

log() { printf '[alpha-decompile] %s\n' "$*"; }

find_python() {
    local candidate
    for candidate in python3 python "py -3"; do
        if $candidate --version >/dev/null 2>&1 \
           && $candidate -c 'import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1; then
            printf '%s' "$candidate"
            return 0
        fi
    done
    return 1
}

if printf '%s  %s\n' "da39a3ee5e6b4b0d3255bfef95601890afd80709" /dev/null \
    | sha1sum -c >/dev/null 2>&1; then
    SHA1_CHECK="sha1sum"
else
    SHA1_CHECK="shasum"
fi

for dependency in java curl git jq "$SHA1_CHECK"; do
    command -v "$dependency" >/dev/null 2>&1 \
        || { printf 'Missing prerequisite: %s\n' "$dependency" >&2; exit 1; }
done
PYTHON=$(find_python) \
    || { printf '%s\n' "A working Python 3 interpreter is required." >&2; exit 1; }

if [ -d "$OUT_DIR/src" ] && [ "$FORCE" -eq 0 ]; then
    log "Reference source already exists at $OUT_DIR/src"
    log "Pass --force to rebuild it."
    exit 0
fi

mkdir -p "$OUT_DIR/tools"

if [ "$FORCE" -eq 1 ]; then
    rm -rf -- \
        "$OUT_DIR/src" \
        "$OUT_DIR/src.next" \
        "$OUT_DIR/client-server-named.jar" \
        "$OUT_DIR/feather-named.tiny" \
        "$OUT_DIR/decompile-metadata.txt"
fi

if [ ! -f "$OUT_DIR/version_manifest_v2.json" ] || [ "$FORCE" -eq 1 ]; then
    log "Fetching Mojang version manifest"
    curl -sSfL -o "$OUT_DIR/version_manifest_v2.json" "$MANIFEST_URL"
fi

VERSION_MANIFEST_URL=$(jq -r --arg version "$VERSION" \
    '.versions[] | select(.id == $version) | .url' \
    "$OUT_DIR/version_manifest_v2.json")
if [ -z "$VERSION_MANIFEST_URL" ] || [ "$VERSION_MANIFEST_URL" = "null" ]; then
    printf "Version '%s' is absent from Mojang's manifest.\n" "$VERSION" >&2
    exit 1
fi

if [ ! -f "$OUT_DIR/$VERSION.json" ] || [ "$FORCE" -eq 1 ]; then
    log "Fetching Mojang metadata for $VERSION"
    curl -sSfL -o "$OUT_DIR/$VERSION.json" "$VERSION_MANIFEST_URL"
fi

CLIENT_URL=$(jq -r '.downloads.client.url // empty' "$OUT_DIR/$VERSION.json")
CLIENT_SHA1=$(jq -r '.downloads.client.sha1 // empty' "$OUT_DIR/$VERSION.json")
CLIENT_SIZE=$(jq -r '.downloads.client.size // empty' "$OUT_DIR/$VERSION.json")
if [ -z "$CLIENT_URL" ] || [ -z "$CLIENT_SHA1" ]; then
    printf "Mojang metadata for '%s' has no client download.\n" "$VERSION" >&2
    exit 1
fi

if [ ! -f "$OUT_DIR/client.jar" ] || [ "$FORCE" -eq 1 ]; then
    log "Downloading official $VERSION client jar"
    curl -sSfL -o "$OUT_DIR/client.jar.part" "$CLIENT_URL"
    mv "$OUT_DIR/client.jar.part" "$OUT_DIR/client.jar"
fi
printf '%s  %s\n' "$CLIENT_SHA1" "$OUT_DIR/client.jar" | $SHA1_CHECK -c

if [ ! -d "$FEATHER_DIR/.git" ]; then
    log "Preparing pinned Ornithe Feather checkout"
    mkdir -p "$FEATHER_DIR"
    git -C "$FEATHER_DIR" init -q
    git -C "$FEATHER_DIR" remote add origin "$FEATHER_URL"
    git -C "$FEATHER_DIR" fetch --depth 1 origin "$FEATHER_REF"
    git -C "$FEATHER_DIR" checkout --detach -q FETCH_HEAD
elif [ "$(git -C "$FEATHER_DIR" rev-parse HEAD)" != "$FEATHER_REF" ]; then
    log "Updating Feather checkout to pinned revision $FEATHER_REF"
    git -C "$FEATHER_DIR" fetch --depth 1 origin "$FEATHER_REF"
    git -C "$FEATHER_DIR" checkout --detach -q FETCH_HEAD
fi

log "Mapping and decompiling $VERSION with Feather and Vineflower"
(
    cd "$FEATHER_DIR"
    $PYTHON feather.py decompileWithVineflower "$VERSION"
)

shopt -s nullglob
SOURCE_CANDIDATES=("$FEATHER_DIR/${VERSION}&"*-decompiledSrc)
shopt -u nullglob
if [ -d "$FEATHER_DIR/$VERSION-decompiledSrc" ]; then
    SOURCE_CANDIDATES+=("$FEATHER_DIR/$VERSION-decompiledSrc")
fi

if [ "${#SOURCE_CANDIDATES[@]}" -ne 1 ]; then
    printf 'Expected one decompiled source directory for %s; found %s.\n' \
        "$VERSION" "${#SOURCE_CANDIDATES[@]}" >&2
    exit 1
fi

MERGED_BUILD=$(basename "${SOURCE_CANDIDATES[0]}" -decompiledSrc)
NAMED_JAR="$FEATHER_DIR/build/ornithe-keratin/gen2/$MERGED_BUILD-processed-named.jar"
NAMED_MAPPING="$FEATHER_DIR/build/ornithe-keratin/gen2/$MERGED_BUILD-named.tiny"
if [ ! -f "$NAMED_JAR" ] || [ ! -f "$NAMED_MAPPING" ]; then
    printf 'Could not locate Feather named outputs for %s.\n' "$MERGED_BUILD" >&2
    exit 1
fi

rm -rf -- "$OUT_DIR/src.next"
cp -R "${SOURCE_CANDIDATES[0]}" "$OUT_DIR/src.next"
mv "$OUT_DIR/src.next" "$OUT_DIR/src"
cp "$NAMED_JAR" "$OUT_DIR/client-server-named.jar"
cp "$NAMED_MAPPING" "$OUT_DIR/feather-named.tiny"

{
    printf 'minecraft_version=%s\n' "$VERSION"
    printf 'mojang_client_url=%s\n' "$CLIENT_URL"
    printf 'mojang_client_sha1=%s\n' "$CLIENT_SHA1"
    printf 'mojang_client_size=%s\n' "$CLIENT_SIZE"
    printf 'feather_url=%s\n' "$FEATHER_URL"
    printf 'feather_ref=%s\n' "$FEATHER_REF"
    printf 'feather_merged_build=%s\n' "$MERGED_BUILD"
    printf 'mapping_license=CC0-1.0\n'
} > "$OUT_DIR/decompile-metadata.txt"

log "Decompiled source: $OUT_DIR/src"
log "Named merged jar: $OUT_DIR/client-server-named.jar"
log "Provenance: $OUT_DIR/decompile-metadata.txt"
