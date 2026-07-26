#!/usr/bin/env bash
# Decompile a Minecraft Java Edition version to readable source. Mapped
# releases use Mojang's official obfuscation mappings and SpecialSource;
# current unobfuscated releases are decompiled directly. Vineflower performs
# both decompilation paths. Idempotent: each step skips if its output already
# exists.
#
# Usage:
#   ./decompile-mc.sh [VERSION] [--server] [--out DIR] [--force] [--parchment] [--no-assets] [--only PREFIX]...
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
#   --only PATH  Decompile only classes whose internal path starts with PATH.
#                Repeat for multiple classes/packages. Intended for focused
#                reference research.
#
# Prereqs: java (17+ recommended), curl, jq, sha1sum, unzip. python3 if --parchment.

set -euo pipefail

SPECIAL_SOURCE_URL="https://repo1.maven.org/maven2/net/md-5/SpecialSource/1.11.4/SpecialSource-1.11.4-shaded.jar"
VINEFLOWER_VERSION="1.12.0"
VINEFLOWER_URL="https://github.com/Vineflower/vineflower/releases/download/${VINEFLOWER_VERSION}/vineflower-${VINEFLOWER_VERSION}.jar"
VINEFLOWER_JAR="tools/vineflower-${VINEFLOWER_VERSION}.jar"
MANIFEST_URL="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

VERSION=""
SIDE="client"
OUT_DIR=""
FORCE=0
PARCHMENT=0
EXTRACT_ASSETS=1
ONLY_PATTERNS=()

while [ $# -gt 0 ]; do
    case "$1" in
        --server)    SIDE="server"; shift ;;
        --client)    SIDE="client"; shift ;;
        --out)       OUT_DIR="$2"; shift 2 ;;
        --force)     FORCE=1; shift ;;
        --parchment) PARCHMENT=1; shift ;;
        --no-assets) EXTRACT_ASSETS=0; shift ;;
        --only)
            if [ $# -lt 2 ] || [ -z "$2" ]; then
                echo "--only requires a class or package path" >&2
                exit 2
            fi
            ONLY_PATTERNS+=("${2%.class}")
            shift 2 ;;
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

# macOS ships a sha1sum that doesn't support -c; prefer shasum (always present on macOS)
if echo "da39a3ee5e6b4b0d3255bfef95601890afd80709  /dev/null" | sha1sum -c >/dev/null 2>&1; then
    SHA1SUM="sha1sum"
else
    SHA1SUM="shasum"
fi

# On Windows, `winget install jqlang.jq` drops jq under the WinGet Packages
# directory but does NOT update the PATH of an already-open shell. If jq isn't
# already resolvable, look for it there and prepend its directory to PATH so the
# rest of this script's jq calls work without a manual PATH edit. No-op on
# Linux/macOS (the search base won't exist).
if ! command -v jq >/dev/null 2>&1; then
    _jq=$(find "$HOME/AppData/Local/Microsoft/WinGet/Packages" -iname 'jq*.exe' 2>/dev/null | head -1) || true
    [ -n "${_jq:-}" ] && PATH="$(dirname "$_jq"):$PATH" && export PATH
fi

for bin in java curl jq "$SHA1SUM"; do
    command -v "$bin" >/dev/null 2>&1 || { echo "Missing prereq: $bin" >&2; exit 1; }
done
command -v unzip >/dev/null 2>&1 || {
    echo "Missing prereq: unzip" >&2
    exit 1
}

log()  { printf '[mc-decompile] %s\n' "$*"; }
need() { [ "$FORCE" = 1 ] || [ ! -e "$1" ]; }

# Echo a working Python 3 launcher, or return 1 if none is found. On Windows,
# `python3` is usually a Microsoft Store stub: it sits on PATH (so `command -v`
# finds it) but exits non-zero with "Python was not found" on any real call. We
# therefore probe each candidate with `--version` and confirm it reports Python
# 3 before trusting it. Candidates may be multi-word (`py -3`), so callers must
# use the result unquoted.
find_python() {
    local cand
    for cand in python3 python "py -3"; do
        if $cand --version >/dev/null 2>&1 \
           && $cand -c 'import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1; then
            printf '%s' "$cand"
            return 0
        fi
    done
    return 1
}

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

# 3. Jar + optional mappings (all downloaded inputs are SHA1-verified)
JAR_URL=$(jq -r ".downloads.${SIDE}.url" "${VERSION}.json")
JAR_SHA=$(jq -r ".downloads.${SIDE}.sha1" "${VERSION}.json")
MAP_URL=$(jq -r ".downloads.${SIDE}_mappings.url // empty" "${VERSION}.json")
MAP_SHA=$(jq -r ".downloads.${SIDE}_mappings.sha1 // empty" "${VERSION}.json")

if need "${SIDE}.jar"; then
    log "Downloading ${SIDE}.jar..."
    curl -sSfL -o "${SIDE}.jar" "$JAR_URL"
    echo "${JAR_SHA}  ${SIDE}.jar" | $SHA1SUM -c
fi

NAMING_MODE="official-mappings"
DECOMPILE_JAR="${SIDE}-deobf.jar"
if [ -n "$MAP_URL" ]; then
    if need "${SIDE}.txt"; then
        log "Downloading ${SIDE} mappings..."
        curl -sSfL -o "${SIDE}.txt" "$MAP_URL"
        echo "${MAP_SHA}  ${SIDE}.txt" | $SHA1SUM -c
    fi
else
    NAMED_CLASS="net/minecraft/client/Minecraft.class"
    if [ "$SIDE" = "server" ]; then
        NAMED_CLASS="net/minecraft/server/MinecraftServer.class"
    fi
    if ! unzip -Z1 "${SIDE}.jar" | grep -Fx "$NAMED_CLASS" >/dev/null; then
        echo "No ${SIDE}_mappings are available for ${VERSION}, and the jar" >&2
        echo "does not contain the expected unobfuscated class ${NAMED_CLASS}." >&2
        echo "Use the dedicated Alpha/Beta Feather scripts for legacy unmapped releases." >&2
        exit 1
    fi
    if [ "$PARCHMENT" = 1 ]; then
        echo "--parchment is not applicable to unobfuscated ${VERSION} jars." >&2
        echo "The official jar already includes original parameter and variable names." >&2
        exit 1
    fi
    NAMING_MODE="official-unobfuscated"
    DECOMPILE_JAR="${SIDE}.jar"
    log "No mappings published; verified an official unobfuscated ${SIDE} jar."
fi

# 4. Tools
if [ "$NAMING_MODE" = "official-mappings" ] && need tools/SpecialSource.jar; then
    log "Downloading SpecialSource..."
    curl -sSfL -o tools/SpecialSource.jar "$SPECIAL_SOURCE_URL"
fi
if need "$VINEFLOWER_JAR"; then
    log "Downloading Vineflower..."
    curl -sSfL -o "$VINEFLOWER_JAR" "$VINEFLOWER_URL"
fi

# 5. Remap mapped jars. Mojang ships Proguard-format mappings;
#    SpecialSource's default direction is what we want, do NOT pass --reverse.
if [ "$NAMING_MODE" = "official-mappings" ] && need "$DECOMPILE_JAR"; then
    log "Remapping ${SIDE}.jar -> ${DECOMPILE_JAR}..."
    java -jar tools/SpecialSource.jar \
        --in-jar "${SIDE}.jar" \
        --out-jar "$DECOMPILE_JAR" \
        --srg-in "${SIDE}.txt" \
        --kill-lvt
fi

# 6. Decompile
SRC_DIR="src"
SELECTION_FILE="decompile-selection.txt"
if [ ${#ONLY_PATTERNS[@]} -gt 0 ]; then
    SELECTION_TEXT=$(printf '%s\n' "${ONLY_PATTERNS[@]}")
    if [ -e "$SRC_DIR/net" ] && [ ! -e "$SELECTION_FILE" ]; then
        echo "Existing source tree has no selective-decompile receipt." >&2
        echo "Choose a fresh --out directory for --only research." >&2
        exit 1
    fi
    if [ -e "$SELECTION_FILE" ] && [ "$(cat "$SELECTION_FILE")" != "$SELECTION_TEXT" ]; then
        echo "Selective-decompile patterns differ from the existing receipt." >&2
        echo "Choose a fresh --out directory rather than mixing source sets." >&2
        exit 1
    fi
fi

if need "$SRC_DIR/net" ; then
    log "Decompiling with Vineflower (this takes a few minutes)..."
    mkdir -p "$SRC_DIR"
    if [ ${#ONLY_PATTERNS[@]} -gt 0 ]; then
        ONLY_ARGS=()
        for pattern in "${ONLY_PATTERNS[@]}"; do
            ONLY_ARGS+=("--only=${pattern}")
        done
        SELECTED_ARCHIVE=".selected-source.$$.jar"
        trap 'rm -f "$SELECTED_ARCHIVE"' EXIT
        java -Xmx4g -jar "$VINEFLOWER_JAR" \
            --skip-extra-files=true \
            "${ONLY_ARGS[@]}" \
            --silent \
            "$DECOMPILE_JAR" "$SELECTED_ARCHIVE"
        unzip -q -o "$SELECTED_ARCHIVE" -d "$SRC_DIR"
        mv "$SELECTED_ARCHIVE" selected-source.jar
        trap - EXIT
        printf '%s\n' "${ONLY_PATTERNS[@]}" > "$SELECTION_FILE"
    else
        java -Xmx4g -jar "$VINEFLOWER_JAR" --silent \
            "$DECOMPILE_JAR" "$SRC_DIR/"
    fi
fi

# 7. Optionally apply Parchment parameter names
if [ "$PARCHMENT" = 1 ]; then
    PYTHON=$(find_python) || {
        echo "python3 required for --parchment, but no working Python 3 was found." >&2
        echo "(A Microsoft Store 'python3' stub on PATH does not count — install real Python 3.)" >&2
        exit 1
    }
    log "Applying Parchment parameter mappings (using: $PYTHON)..."
    $PYTHON "$SCRIPT_DIR/apply-parchment.py" "$OUT_DIR/$SRC_DIR" --mc "$VERSION"
fi

# 8. Extract renderer-bootstrap assets (client only). Idempotent.
if [ "$EXTRACT_ASSETS" = 1 ] && [ "$SIDE" = "client" ]; then
    log "Extracting assets..."
    EXTRACT_ARGS=("$VERSION" "--out" "$OUT_DIR")
    [ "$FORCE" = 1 ] && EXTRACT_ARGS+=("--force")
    "$SCRIPT_DIR/extract-assets.sh" "${EXTRACT_ARGS[@]}"
fi

{
    printf 'version=%s\n' "$VERSION"
    printf 'side=%s\n' "$SIDE"
    printf 'naming=%s\n' "$NAMING_MODE"
    printf 'jar_sha1=%s\n' "$JAR_SHA"
    printf 'mappings_sha1=%s\n' "$MAP_SHA"
    printf 'vineflower=%s\n' "$VINEFLOWER_VERSION"
    printf 'release_time=%s\n' "$(jq -r '.releaseTime' "${VERSION}.json")"
    if [ ${#ONLY_PATTERNS[@]} -gt 0 ]; then
        printf 'selection=focused\n'
    else
        printf 'selection=full\n'
    fi
} > provenance.txt

log ""
log "Done. Source tree: $OUT_DIR/$SRC_DIR"
log "Class count:  $(find "$SRC_DIR" -name '*.java' | wc -l) .java files"
if [ -d "$SRC_DIR/net/minecraft/world/level/levelgen" ]; then
    log "Worldgen entry: $SRC_DIR/net/minecraft/world/level/levelgen/"
fi
