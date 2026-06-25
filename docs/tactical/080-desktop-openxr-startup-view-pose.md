# 080: Desktop OpenXR Startup View Pose

Status: complete. Windows VirtualDesktopXR/Quest 3 over Vulkan was validated on
June 25, 2026.

## Purpose

Make the desktop mclone XR smoke launch pose predictable and scriptable.

The first mclone-frame smoke used the fixed overview camera as the startup
alignment. That was useful for proving rendering, but not good enough as a
repeatable headset recenter contract. This slice adds a Playbox-shaped startup
view pose:

```text
X,Y,Z,YAW_DEGREES
```

The first tracked headset center maps to the requested mclone world position,
and the first headset yaw maps to the requested mclone world yaw. Head pitch and
roll remain runtime-tracked head motion.

## Playbox References

- `~/code/playbox/src/startup.rs`
  - `XrStartupViewPose`
  - `parse_xr_view_pose`
  - `xr_view_pose_from_scene_camera`
- `~/code/playbox/src/xr/locomotion.rs`
  - `align_world_view_to_stage_view`
  - `yaw_from_forward`
- `~/code/playbox/android-xr/install-quest-openxr.sh`
  - `--view-pose X,Y,Z,YAW_DEGREES`
- `~/code/playbox/src/startup_args.rs`
  - one structured startup argument surface with platform applicability instead
    of one-off environment-only switches.

## Landed

- Added `XrViewPose` to `mclone-native-client` CLI state.
- Added app flags:
  - `--xr-view-pose X,Y,Z,YAW_DEGREES`
  - `--view-pose X,Y,Z,YAW_DEGREES` as a launch-script-friendly alias.
- Kept default mclone XR smoke behavior compatible: without a view pose, the
  smoke still uses the fixed overview/full-orientation alignment.
- When a view pose is supplied, `XrStageToWorld` uses Playbox-style yaw-only
  alignment.
- Logged the active alignment mode, requested world eye, requested world yaw,
  and initial stage center.
- Added tests for CLI parsing/rejection and stage-to-world view-pose mapping.
- Standardized launcher runtime selection:
  - PowerShell: `-Runtime virtual-desktop|active|json|environment`
  - Bash: `--runtime wivrn|active|json|environment`
  - JSON override remains available as `-RuntimeJson` / `--runtime-json`.
- Added launcher-level pose forwarding:
  - PowerShell: `-ViewPose "X,Y,Z,YAW_DEGREES"`
  - Batch: `--view-pose X,Y,Z,YAW_DEGREES`, delegated to PowerShell to keep
    Quest wake/proximity setup and restore centralized
  - Bash: `--view-pose X,Y,Z,YAW_DEGREES`
- Added a Playbox-patterned Windows batch launcher at `scripts/start-xr.bat`
  for common Windows XR smoke commands.
- Kept Windows Quest startup aligned with Playbox validation helpers by logging
  battery state, refusing low unpowered wake runs, dismissing known Quest
  system panels before launch, and logging the focused headset activity.

## Validation

Required gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr startup_view_pose_maps_stage_center_to_requested_world_pose
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr symmetric_xr_fov_matches_chunk_projection_convention
pnpm native:web:build
bash -n scripts/start-xr.sh
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start-xr.ps1 -CheckOnly -Runtime environment
cmd /c scripts\start-xr.bat --check-only --runtime environment --mclone --view-pose 0,78,-96,180 --frames 5 --no-pause
cmd /c scripts\start-xr.bat --check-only --runtime environment --mclone --view-pose "0,78,-96,180" --frames 5 --no-pause
pnpm native:xr:windows:check
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --frames 1200 --no-pause
```

Live Windows validation:

```powershell
pnpm native:xr:windows:prepare
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start-xr.ps1 `
  -Runtime virtual-desktop `
  -Smoke mclone `
  -Frames 1500 `
  -NoQuestLaunch `
  -ViewPose "0,78,-96,180"
```

Observed runtime:

- Runtime: `VirtualDesktopXR v1.0.10`
- System: `Meta Quest 3`
- Backend: Vulkan through `XR_KHR_vulkan_enable2`
- GPU: `NVIDIA GeForce RTX 4090`, Vulkan API `1.4.341`
- Swapchains: `Rgba8UnormSrgb`, `1728x1824` per eye, `3/3` images

Observed view-pose result:

- Alignment log:
  `mode=view-pose world_eye=(0.00, 78.00, -96.00) world_yaw_degrees=180.0`
- Initial stage center:
  approximately `(0.001, 1.640, -0.000)`
- Frame result:
  `submitted=1500 runtime_frames=1500 skipped=0`
- Render summary:
  `sections=166 drawn_sections=166 indices=780174 drawn_indices=780174 actors=1 drawn_actors=1`
- Cleanup restored Quest wake/proximity settings and slept the headset.
- Batch launcher validation on June 25, 2026 reported
  `submitted=1200 runtime_frames=1200 skipped=0`, with Quest battery
  `94%` on AC power and `VirtualDesktop.Android` focused before mclone launch.

Visual validation:

```powershell
adb shell screencap -p /sdcard/mclone-xr-view-pose.png
adb pull /sdcard/mclone-xr-view-pose.png C:\tmp\mclone-xr-view-pose.png
```

The inspected headset screenshot at `C:\tmp\mclone-xr-view-pose.png` showed
the mclone world rendered closer in both eyes with the Virtual Desktop overlay
composited above it.

The later BAT-driven screenshot at `C:\tmp\mclone-xr-bat-validation-3.png`
also showed the mclone stereo world with Virtual Desktop overlays. Capture
should wait for the `OpenXR session state: FOCUSED` log marker; a fixed early
delay can capture the Quest overlay before the mclone scene starts submitting
frames.

## Next Step

Add controller/action input as a separate slice. Keep the same launcher/startup
option pattern: stable app-owned XR behavior flags, script-owned host/runtime
bootstrap, and no OpenXR types in shared engine crates.
