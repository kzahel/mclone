#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORACLE_DIR="${ROOT_DIR}/oracle"
CLASS_DIR="${ORACLE_DIR}/classes"
DEOBF_JAR="${ROOT_DIR}/reference/minecraft-1.17.1/client-deobf.jar"
VERSION_JSON="${ROOT_DIR}/reference/minecraft-1.17.1/1.17.1.json"
LIBRARY_DIR="${ROOT_DIR}/reference/minecraft-1.17.1/libraries"

if [[ "${1:-}" == "--" ]]; then
  shift
fi

"${ORACLE_DIR}/build.sh"

mkdir -p "${LIBRARY_DIR}"

mapfile -t LIBRARY_ROWS < <(
  jq -r '.libraries[] | select(.downloads.artifact.url != null and .downloads.artifact.path != null) | [.downloads.artifact.url, .downloads.artifact.path] | @tsv' "${VERSION_JSON}"
)

for row in "${LIBRARY_ROWS[@]}"; do
  IFS=$'\t' read -r url path <<< "${row}"
  dest="${LIBRARY_DIR}/${path}"
  if [[ ! -f "${dest}" ]]; then
    mkdir -p "$(dirname "${dest}")"
    curl -sSfL -o "${dest}" "${url}"
  fi
done

mapfile -t LIBRARY_JARS < <(find "${LIBRARY_DIR}" -type f -name '*.jar' | sort)
CLASSPATH="${CLASS_DIR}:${DEOBF_JAR}"

for jar in "${LIBRARY_JARS[@]}"; do
  CLASSPATH="${CLASSPATH}:${jar}"
done

exec java \
  -cp "${CLASSPATH}" \
  OracleDumper \
  "$@"
