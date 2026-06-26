# 092: XR Headset-Relative Locomotion

Status: active; Slice 1 code landed on June 26, 2026. Automated shared XR,
render-session, desktop XR, and Android XR compile gates passed. Manual headset
feel validation remains.

## Purpose

Make XR left-stick movement understandable by default: pushing forward moves
toward the headset's current horizontal facing direction, and pushing sideways
strafes perpendicular to that headset direction. Desktop OpenXR and Android XR
must keep using one shared implementation in `mclone-xr-scene`.

## Current Problem

The shared XR path already owns controller locomotion for both desktop OpenXR
and Android XR, but the left-stick movement vector is interpreted through the
engine/player body yaw. The tracked HMD pose is applied later as a render-only
offset, so physically turning your head does not redefine forward or strafe.

The current left-stick helper also carries the old Quest/VirtualDesktopXR axis
swap. OpenXR bindings already expose thumbstick X and Y separately, so the
default mapping should be named and straightforward: X is lateral, Y is
forward/back.

## Target Shape

- `mclone-xr-scene` owns the default XR locomotion policy.
- The default policy is headset-yaw-relative movement.
- The previous player/body-yaw-relative path remains available as an explicit
  mode for deterministic desktop-style behavior and future comfort options.
- The shared engine movement/collision code accepts an optional movement-yaw
  override instead of mutating the player's actual view/root yaw for XR.
- App/platform crates only pass the current OpenXR stereo views into the shared
  scene input method; they do not implement their own movement transforms.
- Future left-controller-facing locomotion can add another named policy without
  forking desktop and Android XR callers.

## Slice 1 - Shared HMD-Yaw Default

- [x] Add a locomotion-frame policy enum in `mclone-xr-scene`.
- [x] Pass the current stereo views into `apply_locomotion_input(...)` from
  desktop OpenXR and Android XR.
- [x] Compute HMD horizontal yaw from the averaged stereo view pose.
- [x] Feed that yaw as a movement override into the shared engine camera
  movement path.
- [x] Replace the left-stick mapping with standard X/Y semantics.
- [x] Add focused tests for headset-yaw movement override and XR stick mapping.
- [x] Validate with focused Rust tests and XR compile checks.

## Landed

- Added `XrLocomotionMode` with the default set to `HeadsetYaw` and retained
  `PlayerYaw` as the explicit body/root-yaw-relative mode.
- Desktop OpenXR and Android XR now both pass the current stereo views into the
  shared `mclone-xr-scene` locomotion application point. There is still one
  shared movement implementation for both XR lanes.
- `mclone-xr-scene` resolves the current HMD horizontal world-forward vector
  from the averaged per-eye OpenXR poses and converts it to the engine movement
  yaw convention before creating `EngineCameraInput`.
- `mclone-render-session::EngineCameraInput` now carries an optional
  `movement_yaw_radians` override. Walking and no-clip movement use it for
  locomotion only; it does not mutate the actual camera/player view yaw.
- Left-stick mapping now uses standard OpenXR thumbstick semantics:
  X is lateral and Y is forward/back. Since the engine impulse field is named
  `left`, physical right maps to a negative left impulse.

## Validation

Passed on June 26, 2026:

```powershell
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
git diff --check
```

No app-local desktop/Android XR locomotion fork was introduced. The only app
changes are the required stereo-view arguments passed into the shared scene.

## Deferred

- Runtime UI/options for locomotion mode selection.
- Left-controller-facing locomotion.
- Snap-turn/comfort tuning beyond the existing right-stick smooth yaw path.
- Device headset validation of the exact feel after this automated chunk.
