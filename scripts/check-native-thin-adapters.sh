#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Native client rims may own lifecycle, raw input, targets, transport setup,
# and diagnostics sinks. Shared session/settings/input/render orchestration
# belongs in mclone-scene or its lower-level shared crates. Web is deliberately
# excluded until the deferred web host-adoption tactical lands.
apps=(
  native/apps/mclone-native-client/src
  native/apps/mclone-android-client/src
  native/apps/mclone-android-xr-client/src
)

forbidden='ClientExperienceSettingEffect|sync_render_sections|upload_runtime_sections|wait_begin_frame|poll_openxr_events|end_skipped_frame|end_frame_with_layers|end_stereo_projection_frame|end_multiview_projection_frame|frame_wait[[:space:]]*\.[[:space:]]*wait\(|frame_stream[[:space:]]*\.[[:space:]]*begin\(|EngineCameraInput[[:space:]]*\{|SessionStartRequest::(CreateLocalWorld|OpenLocalWorld|JoinRemote|Unknown)[^=]{0,200}=>'

if rg -n -U "$forbidden" "${apps[@]}"; then
  echo "native client adapter regrew shared scene/session/input orchestration" >&2
  exit 1
fi

./scripts/check-scene-host-purity.sh
./scripts/check-xr-frame-driver-purity.sh
echo "native client adapters remain thin"
