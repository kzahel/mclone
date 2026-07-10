#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

apps=(
  native/apps/mclone-native-client/src
  native/apps/mclone-android-xr-client/src
)
forbidden='poll_openxr_events|wait_begin_frame|end_skipped_frame|end_frame_with_layers|end_stereo_projection_frame|end_multiview_projection_frame|frame_wait[[:space:]]*\.[[:space:]]*wait\(|frame_stream[[:space:]]*\.[[:space:]]*begin\('

if rg -n -U "$forbidden" "${apps[@]}"; then
  echo "XR app code bypasses the shared OpenXR frame driver" >&2
  exit 1
fi

echo "XR app frame sequencing is owned by mclone-xr-host"
