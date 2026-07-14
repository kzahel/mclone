#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Client rims may own lifecycle, raw input, targets, transport setup, browser
# operations, and diagnostics sinks. Shared session/settings/input/render
# orchestration belongs in mclone-scene or its lower-level shared crates. The
# browser has service implementations of some neutral render/session traits, so
# its Rust wrapper and TypeScript driver use the more precise adoption inventory
# composed below instead of this native-app pattern set.
native_apps=(
  native/apps/mclone-native-client/src
  native/apps/mclone-android-client/src
  native/apps/mclone-android-xr-client/src
)

forbidden='ClientExperienceSettingEffect|sync_render_sections|upload_runtime_sections|wait_begin_frame|poll_openxr_events|end_skipped_frame|end_frame_with_layers|end_stereo_projection_frame|end_multiview_projection_frame|frame_wait[[:space:]]*\.[[:space:]]*wait\(|frame_stream[[:space:]]*\.[[:space:]]*begin\(|EngineCameraInput[[:space:]]*\{|SessionStartRequest::(CreateLocalWorld|OpenLocalWorld|JoinRemote|Unknown)[^=]{0,200}=>'

if rg -n -U "$forbidden" "${native_apps[@]}"; then
  echo "native client adapter regrew shared scene/session/input orchestration" >&2
  exit 1
fi

# Player-pose selection lives below the scene and its publication cadence lives
# in the scene. Interactive rims may apply raw look immediately for local
# responsiveness, but they must advance the shared frame entry instead of
# sequencing movement selection and publication themselves. The offscreen host
# retains one explicitly named force path for deterministic capture scripts.
pose_publication_forbidden='apply_mono_movement_frame|commit_mono_player_pose|force_mono_player_pose_reconcile_for_diagnostics'

if rg -n -U --glob '!offscreen_scene_host.rs' \
  "$pose_publication_forbidden" "${native_apps[@]}"; then
  echo "native client adapter regrew player-pose publication policy" >&2
  exit 1
fi

required_pose_entries=(
  'native/apps/mclone-native-client/src/winit_frame_driver.rs:advance_mono_input_frame'
  'native/apps/mclone-native-client/src/offscreen_scene_host.rs:advance_mono_input_frame'
  'native/apps/mclone-native-client/src/desktop_xr.rs:apply_frame_locomotion'
  'native/apps/mclone-android-client/src/surface_driver.rs:advance_mono_input_frame'
  'native/apps/mclone-android-xr-client/src/lib.rs:apply_frame_locomotion'
)

for requirement in "${required_pose_entries[@]}"; do
  path="${requirement%%:*}"
  entry="${requirement#*:}"
  if ! rg -q "$entry" "$path"; then
    echo "$path stopped advancing shared player-pose publication" >&2
    exit 1
  fi
done

# Native rims may select an endpoint and instantiate the shared remote session,
# but socket ownership, stream polling, response bookkeeping, and pump policy
# stay in mclone-net/mclone-app-runtime. The desktop visual-smoke fixture is an
# intentional in-process protocol server and is excluded from this production
# adapter lock.
network_forbidden='NativeClientIoSession|ClientConnection|pump_client_connection_updates_report|pending_response_batches|drain_command_updates|mclone_net|std::net::[^;]*(TcpListener|TcpStream)'

if rg -n -U --glob '!remote_player_visual_smoke.rs' "$network_forbidden" "${native_apps[@]}"; then
  echo "native client adapter regrew transport or update-stream policy" >&2
  exit 1
fi

./scripts/check-scene-host-purity.sh
./scripts/check-xr-frame-driver-purity.sh
node ./scripts/check-web-scene-host-adoption.mjs
echo "native and browser client adapters remain thin"
