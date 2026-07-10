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

assembly="native/crates/mclone-app-runtime/src/native_service_assembly.rs"
assembly_policy_forbidden='ClientExperienceSettingEffect|GameUiAction|EngineCameraInput[[:space:]]*\{|WorldCatalog(Request|Response)::[^=]{0,200}=>|SessionStartRequest::(CreateLocalWorld|OpenLocalWorld|JoinRemote|Unknown)[^=]{0,200}=>'
if rg -n -U "$assembly_policy_forbidden" "$assembly"; then
  echo "native service assembly contains shared scene/session/UI policy" >&2
  exit 1
fi

if rg -n 'runtime:[[:space:]]+Option<Native|NativeWorldCatalog|NativeTeleport' \
  native/crates/mclone-scene/src; then
  echo "mclone-scene regained a concrete native service owner" >&2
  exit 1
fi

echo "mclone-scene is platform-rim free; native assembly remains policy-free"
