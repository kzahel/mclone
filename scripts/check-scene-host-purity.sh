#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

forbidden='openxr|mclone-xr-host|winit|android-activity|jni'
tree="$(cargo tree --manifest-path native/Cargo.toml -p mclone-scene -e normal)"
if printf '%s\n' "$tree" | rg -i "$forbidden"; then
  echo "mclone-scene dependency graph contains a forbidden platform edge" >&2
  exit 1
fi

echo "mclone-scene dependency graph is platform-rim free"
