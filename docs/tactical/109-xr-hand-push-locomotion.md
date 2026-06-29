# 109: XR Hand-Push Locomotion

Status: active; Slice 2 shared swept-sphere collision and head validation
landed in code. Quest standalone headset validation remains the preferred feel
gate.

## Purpose

Add a Gorilla Tag-style hand-push locomotion option without making it desktop,
Quest, or OpenXR app glue. The movement technique must live behind shared
engine/client contracts so desktop OpenXR and Android XR feed the same code,
and desktop flat can provide a deterministic no-headset emulation mode for
regression testing.

## References

- `reference/GorillaLocomotion/Player.cs` (`bc42e95`) is the closest C#
  algorithm reference: arm-length clamping, hand collision, head validation,
  unstick checks, surface slip, and velocity-history fling.
- `reference/GodotGorillaTagMovement/` (`c718512`) is a smaller Godot sample
  that separates hand pushers from a movement provider.
- Both references are MIT licensed and should guide behavior, not create a
  platform-shaped copy.

## Target Shape

- `mclone-client` owns the hand-push movement controller and block collision
  interaction.
- `mclone-render-session` exposes a real `EngineCameraMovementMode::HandPush`
  and keeps pose sync on the existing local-player command path.
- `mclone-xr-scene` converts OpenXR views/controller grip poses into shared
  hand-push input for both desktop OpenXR and Android XR.
- `mclone-ui` exposes a movement-mode selector rather than a fly-only checkbox.
- Desktop flat has a basic emulated-hand path where normal movement input drives
  a synthetic two-hand cycle through the same engine controller.
- Quest standalone validation is the correctness bar for real feel. Desktop
  emulation is for deterministic coverage and bring-up only.

## Slice 1 - Shared Baseline

- [x] Add a shared `HandPushLocomotionController` in `mclone-client`.
- [x] Implement first-pass AABB hand probes, two-hand averaging, arm-length
  clamping, unstick reset, and velocity-history fling.
- [x] Add `EngineCameraMovementMode::HandPush` in `mclone-render-session`.
- [x] Add engine `EngineHandPushInput` and desktop flat hand-cycle emulation.
- [x] Replace the options fly checkbox with a movement selector:
  `Walk / Fly / Hand Push`.
- [x] Wire OpenXR grip/head poses into `EngineHandPushInput` in shared
  `mclone-xr-scene`, with no desktop/Android XR app-local locomotion fork.
- [x] Add focused unit coverage for client hand push, engine mode/emulation,
  UI action, and XR pose conversion.

## Slice 2 - Shared Collision Refinement

- [x] Replace the first-pass hand AABB movement probe with a swept sphere
  helper that raycasts the hand center against radius-expanded block AABBs.
- [x] Add separate head-sphere validation before body movement is submitted to
  the normal player collision path.
- [x] Preserve the existing shared `HandPushLocomotionController` boundary; no
  desktop, Android, or XR app owns gameplay locomotion policy.
- [x] Add focused coverage for hand sphere sweeps and head motion clamping.

## Known Gaps

- The sweep helper is a first shared approximation of Unity-style spherecasts:
  it handles radius-expanded block AABBs, but not the full iterative
  slide/precision-step behavior from the C# reference.
- Surface slip/material tuning is not implemented.
- The desktop emulator is intentionally crude. It validates plumbing and some
  collision response, not real arm/controller feel.
- Quest standalone still needs headset testing and tuning.

## Validation

Passed in this slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render-session -p mclone-ui -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
git diff --check
```

Additional validation for Slice 2:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render-session -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
git diff --check
```

## Next Slice

Run Quest standalone with the new `Hand Push` mode and capture headset notes.
Use those notes to tune arm length, hand/head radii, unstick distance, and
velocity fling thresholds before adding surface slip/material behavior. Keep
desktop emulation as an automated regression lane, but do not tune the final
feel from desktop emulation alone.
