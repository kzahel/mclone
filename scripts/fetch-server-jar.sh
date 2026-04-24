#!/usr/bin/env bash
# Download Mojang's official Minecraft server jar for the given version,
# SHA1-verified against the per-version manifest. Used by the integration
# oracle harness — never decompiled, never remapped, just run headless.
#
# Usage:
#   ./fetch-server-jar.sh [VERSION] [--out DIR] [--force]
#
# Args:
#   VERSION      Minecraft version id. Default: 1.17.1
#
# Flags:
#   --out DIR    Output directory. Default: <repo>/reference/minecraft-<VERSION>
#   --force      Redo download even if the jar already exists.
#
# Prereqs: curl, jq, sha1sum.

set -euo pipefail

MANIFEST_URL="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

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

for bin in curl jq sha1sum; do
    command -v "$bin" >/dev/null 2>&1 || { echo "Missing prereq: $bin" >&2; exit 1; }
done

log()  { printf '[fetch-server-jar] %s\n' "$*"; }
need() { [ "$FORCE" = 1 ] || [ ! -e "$1" ]; }

verify_sha1() {
    local expected="$1"
    local file="$2"
    local actual
    actual=$(sha1sum "$file")
    actual=${actual%% *}
    if [ "$actual" != "$expected" ]; then
        echo "SHA1 mismatch for $file: expected $expected, got $actual" >&2
        exit 1
    fi
    log "$file: OK"
}

mkdir -p "$OUT_DIR"
cd "$OUT_DIR"

if need version_manifest_v2.json; then
    log "Fetching version manifest..."
    curl -sSfL -o version_manifest_v2.json "$MANIFEST_URL"
fi

if need "${VERSION}.json"; then
    log "Locating ${VERSION} in manifest..."
    VERSION_URL=$(jq -r --arg v "$VERSION" \
        '.versions[] | select(.id == $v) | .url' version_manifest_v2.json)
    if [ -z "$VERSION_URL" ] || [ "$VERSION_URL" = "null" ]; then
        echo "Version '$VERSION' not found in Mojang manifest." >&2
        exit 1
    fi
    log "Fetching per-version manifest..."
    curl -sSfL -o "${VERSION}.json" "$VERSION_URL"
fi

JAR_URL=$(jq -r '.downloads.server.url' "${VERSION}.json")
JAR_SHA=$(jq -r '.downloads.server.sha1' "${VERSION}.json")

if [ -z "$JAR_URL" ] || [ "$JAR_URL" = "null" ]; then
    echo "No server jar available for ${VERSION}." >&2
    exit 1
fi

if need server.jar; then
    log "Downloading server.jar..."
    curl -sSfL -o server.jar "$JAR_URL"
fi

verify_sha1 "$JAR_SHA" server.jar

log "Done. Server jar: $OUT_DIR/server.jar"
