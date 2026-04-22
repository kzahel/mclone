#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORACLE_DIR="${ROOT_DIR}/oracle"
CLASS_DIR="${ORACLE_DIR}/classes"
DEOBF_JAR="${ROOT_DIR}/reference/minecraft-1.17.1/client-deobf.jar"

if [[ "${1:-}" == "--" ]]; then
  shift
fi

"${ORACLE_DIR}/build.sh"

exec java \
  -cp "${CLASS_DIR}:${DEOBF_JAR}" \
  OracleDumper \
  "$@"
