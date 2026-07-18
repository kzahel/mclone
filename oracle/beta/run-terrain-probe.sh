#!/usr/bin/env bash

set -euo pipefail

if [ "${1:-}" = "--" ]; then
    shift
fi

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &>/dev/null && pwd)
VERSION="${MCLONE_BETA_VERSION:-b1.7.3}"
REFERENCE_DIR="$REPO_ROOT/reference/minecraft-$VERSION"
NAMED_JAR="$REFERENCE_DIR/client-server-named.jar"
CLIENT_JAR="$REFERENCE_DIR/client.jar"

if [ ! -f "$NAMED_JAR" ]; then
    "$REPO_ROOT/scripts/decompile-beta-mc.sh" "$VERSION"
fi

command -v javac >/dev/null 2>&1 \
    || { printf '%s\n' "javac is required to run the Beta terrain probe." >&2; exit 1; }

PROBE_BUILD=$(mktemp -d "${TMPDIR:-/tmp}/mclone-beta-probe.XXXXXX")
trap 'rm -rf -- "$PROBE_BUILD"' EXIT

CLASSPATH_SEPARATOR=:
JAVA_NAMED_JAR="$NAMED_JAR"
JAVA_CLIENT_JAR="$CLIENT_JAR"
JAVA_PROBE_BUILD="$PROBE_BUILD"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        CLASSPATH_SEPARATOR=';'
        JAVA_NAMED_JAR=$(cygpath -w "$NAMED_JAR")
        JAVA_CLIENT_JAR=$(cygpath -w "$CLIENT_JAR")
        JAVA_PROBE_BUILD=$(cygpath -w "$PROBE_BUILD")
        ;;
esac

javac -Xlint:none -cp "$JAVA_NAMED_JAR" -d "$JAVA_PROBE_BUILD" \
    "$SCRIPT_DIR/BetaTerrainProbe.java"
java -cp "$JAVA_PROBE_BUILD$CLASSPATH_SEPARATOR$JAVA_NAMED_JAR$CLASSPATH_SEPARATOR$JAVA_CLIENT_JAR" \
    BetaTerrainProbe "$@"
