#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORACLE_DIR="${ROOT_DIR}/oracle"
CLASS_DIR="${ORACLE_DIR}/classes"
DEOBF_JAR="${ROOT_DIR}/reference/minecraft-1.17.1/client-deobf.jar"

if [[ ! -f "${DEOBF_JAR}" ]]; then
  echo "missing deobfuscated jar: ${DEOBF_JAR}" >&2
  echo "run scripts/decompile-mc.sh first" >&2
  exit 1
fi

mapfile -t JAVA_SOURCES < <(find "${ORACLE_DIR}/java" -type f -name '*.java' | sort)

if [[ ${#JAVA_SOURCES[@]} -eq 0 ]]; then
  echo "no Java sources found under ${ORACLE_DIR}/java" >&2
  exit 1
fi

mkdir -p "${CLASS_DIR}"

javac \
  --release 17 \
  -cp "${DEOBF_JAR}" \
  -d "${CLASS_DIR}" \
  "${JAVA_SOURCES[@]}"
