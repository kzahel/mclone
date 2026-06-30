# 109: XR Hand-Push Locomotion

Status: active; Slice 3 shared debug visualization landed in code. Quest
standalone headset validation remains the preferred feel gate, especially for
head/body pose sync.

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

## Slice 3 - Shared Debug Visualization

- [x] Add shared engine debug line generation for the local player collision
  box and live hand-push hand collider spheres.
- [x] Render hand collider spheres by default whenever `Hand Push` mode has
  live hand input.
- [x] Add a shared UI option to toggle the player collision box wireframe for
  flat desktop, XR, web, and Android UI state.
- [x] Wire native flat and shared XR scene rendering through the existing
  world-space GUI line renderer instead of app-local debug geometry.
- [x] Add a headless screenshot flag for player-box capture validation.

## Known Gaps

- The sweep helper is a first shared approximation of Unity-style spherecasts:
  it handles radius-expanded block AABBs, but not the full iterative
  slide/precision-step behavior from the C# reference.
- Surface slip/material tuning is not implemented.
- The desktop emulator is intentionally crude. It validates plumbing and some
  collision response, not real arm/controller feel.
- Quest standalone still needs headset testing and tuning.
- The player collision box wireframe is opt-in because it is diagnostic noise;
  hand collider spheres are automatic in `Hand Push` mode because they are part
  of understanding and tuning the movement feel.

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

Additional validation for Slice 3:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-render-session -p mclone-ui -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-player-box-debug.png --width 960 --height 540 --startup-wait frames:2 --screenshot-player-box true --screenshot-camera-view third-person --fullbright true
git diff --check
```

Slice 3 screenshot validation wrote `/tmp/mclone-player-box-debug.png` and
visually confirmed the player collision box wireframe in the world frame. The
hand collider spheres are covered by shared line-builder unit coverage because
the flat headless screenshot path does not synthesize live XR hand poses.

## Next Slice

Run Quest standalone with the new `Hand Push` mode and the debug visuals:
enable `Player Box`, then verify whether the headset eye pose, local collision
box, and rendered hand colliders stay coherent while pushing, colliding, and
unsticking. Use those notes to fix any pose-origin/body-sync issue before
tuning arm length, hand/head radii, unstick distance, and velocity fling
thresholds. Keep desktop emulation as an automated regression lane, but do not
tune the final feel from desktop emulation alone.
