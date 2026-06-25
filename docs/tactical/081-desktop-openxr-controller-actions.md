# 081: Desktop OpenXR Controller Actions

Status: complete. First desktop action polling diagnostics are validated on
Windows VirtualDesktopXR/Quest 3 over Vulkan.

## Purpose

Add the first desktop OpenXR controller/action path to the XR smoke without
turning it into gameplay input yet.

This slice proves action-set creation, suggested controller bindings, session
attachment, per-frame action sync, pose-space location, and bounded diagnostics.
OpenXR ownership stays inside `mclone-native-client`; shared engine/runtime
crates still see no OpenXR types.

## Playbox References

- `~/code/playbox/src/xr/actions.rs`
  - action set creation and attachment
  - simple-controller and Touch/index/Vive/WMR binding suggestions
  - per-frame `sync_actions`
  - aim/grip pose-space location
- `~/code/playbox/docs/architecture/platforms.md`
  - controller action binding scope and platform ownership

## Landed

- Added `xr_clear_smoke/actions.rs` as an app-local OpenXR input module.
- Created a `mclone_input` action set with per-hand:
  - aim pose
  - grip pose
  - trigger value
  - squeeze value
  - select click
  - thumbstick X/Y and click
- Suggested bindings for:
  - `/interaction_profiles/khr/simple_controller`
  - `/interaction_profiles/oculus/touch_controller`
  - `/interaction_profiles/valve/index_controller`
  - `/interaction_profiles/htc/vive_controller`
  - `/interaction_profiles/microsoft/motion_controller`
- Attached the action set before the session begins.
- Synced actions once per renderable XR frame.
- Located active aim/grip spaces in `STAGE` space.
- Printed bounded controller input diagnostics after smoke completion:
  - frames polled
  - left/right active frames
  - left/right tracked frames
  - max trigger/squeeze/thumbstick values
  - select pressed frames
  - latest per-hand aim/grip positions

## Validation

Required gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
pnpm native:web:build
git diff --check
```

Live Windows validation:

```powershell
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --frames 120 --no-pause
```

Observed on June 25, 2026:

- Runtime: `VirtualDesktopXR v1.0.10`
- System: `Meta Quest 3`
- Backend: Vulkan through `XR_KHR_vulkan_enable2`
- Controller action setup:
  - simple-controller and Oculus Touch binding suggestions were accepted
  - Valve Index, Vive, and WMR profiles reported `ERROR_PATH_UNSUPPORTED` on
    this runtime, which is non-fatal
- Frame result: `submitted=600 runtime_frames=600 skipped=0`
- Controller diagnostics:
  - `frames_polled=600`
  - `left_active=600 right_active=600`
  - `left_tracked=0 right_tracked=0`
  - max trigger/squeeze/thumbstick values remained `0.000`

The unattended desk run proved action sync and active action state, but the
controllers were not presenting tracked aim/grip poses. A follow-up should
validate nonzero pose/button data with controllers awake and held in view.

Visual validation:

```powershell
# Captured after the OpenXR session reached FOCUSED.
C:\tmp\mclone-xr-actions-validation.png
```

The inspected screenshot showed the mclone stereo world still rendering
correctly with controller action polling enabled.

## Next Step

Use these controller snapshots for an explicit interaction slice: either XR
locomotion/recenter controls or first block-selection diagnostics. Keep action
polling in `mclone-native-client` and translate to platform-neutral input facts
before touching shared engine crates.
