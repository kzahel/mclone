#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORACLE_DIR="${ROOT_DIR}/oracle"
CLASS_DIR="${ORACLE_DIR}/classes"
DEOBF_JAR="${ROOT_DIR}/reference/minecraft-1.17.1/client-deobf.jar"
VERSION_JSON="${ROOT_DIR}/reference/minecraft-1.17.1/1.17.1.json"
LIBRARY_DIR="${ROOT_DIR}/reference/minecraft-1.17.1/libraries"

if [[ ! -f "${DEOBF_JAR}" ]]; then
  echo "missing deobfuscated jar: ${DEOBF_JAR}" >&2
  echo "run scripts/decompile-mc.sh first" >&2
  exit 1
fi

for bin in curl; do
  command -v "${bin}" >/dev/null 2>&1 || {
    echo "missing prereq: ${bin}" >&2
    exit 1
  }
done

mkdir -p "${LIBRARY_DIR}"

if command -v jq >/dev/null 2>&1; then
  mapfile -t LIBRARY_ROWS < <(
    jq -r '.libraries[] | select(.downloads.artifact.url != null and .downloads.artifact.path != null) | [.downloads.artifact.url, .downloads.artifact.path] | @tsv' "${VERSION_JSON}"
  )
elif command -v node >/dev/null 2>&1; then
  mapfile -t LIBRARY_ROWS < <(
    node --input-type=module -e '
      import fs from "node:fs";
      const version = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
      for (const library of version.libraries ?? []) {
        const artifact = library.downloads?.artifact;
        if (artifact?.url && artifact?.path) {
          console.log(`${artifact.url}\t${artifact.path}`);
        }
      }
    ' "${VERSION_JSON}"
  )
else
  echo "missing prereq: jq or node" >&2
  exit 1
fi

for row in "${LIBRARY_ROWS[@]}"; do
  IFS=$'\t' read -r url path <<< "${row}"
  dest="${LIBRARY_DIR}/${path}"
  if [[ ! -f "${dest}" ]]; then
    mkdir -p "$(dirname "${dest}")"
    curl -sSfL -o "${dest}" "${url}"
  fi
done

mapfile -t LIBRARY_JARS < <(find "${LIBRARY_DIR}" -type f -name '*.jar' | sort)
JAVAC_CLASSPATH="${DEOBF_JAR}"

for jar in "${LIBRARY_JARS[@]}"; do
  JAVAC_CLASSPATH="${JAVAC_CLASSPATH}:${jar}"
done

mapfile -t JAVA_SOURCES < <(find "${ORACLE_DIR}/java" -type f -name '*.java' | sort)

if [[ ${#JAVA_SOURCES[@]} -eq 0 ]]; then
  echo "no Java sources found under ${ORACLE_DIR}/java" >&2
  exit 1
fi

mkdir -p "${CLASS_DIR}"

javac \
  --release 17 \
  -cp "${JAVAC_CLASSPATH}" \
  -d "${CLASS_DIR}" \
  "${JAVA_SOURCES[@]}"
