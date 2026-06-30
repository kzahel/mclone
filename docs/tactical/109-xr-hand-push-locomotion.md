# 109: XR Hand-Push Locomotion

Status: active; Slice 5 head comfort fade landed as the first obstruction
comfort chunk. Quest standalone headset validation remains the preferred feel
gate, especially for head/body pose sync, fade thresholds, and hand-push tuning.

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

## XR Head/Body Reconciliation Policy

Current diagnosis: the XR scene captures an initial OpenXR stage pose and maps
that stage origin to the engine camera/player pose. Later physical room-scale
headset motion affects rendered eye/controller poses, but the engine player
collision box only moves from explicit locomotion input. That lets the headset
view drift away from the body/collision box even when empty space would allow
the body to follow.

The durable invariant is:

> The player body continuously attempts to reconcile to the headset's
> room-scale horizontal position. Any remaining head/body offset must be
> explainable by collision or explicit comfort blocking.

Terminology:

- **Body pose**: the authoritative local player collision box and server-synced
  pose owned by the shared engine/client path.
- **Tracking pose**: OpenXR headset/controllers inside local stage/room space.
- **Stage-to-world transform**: the mapping from tracking space into world
  space for rendering, controller input, UI rays, hand-push input, and debug
  lines.
- **Residual head offset**: the part of physical headset movement that could
  not be consumed by body movement because collision blocked it.

Horizontal policy:

- Physical room-scale X/Z movement is always attempted before stick or
  hand-push locomotion for the frame.
- The engine computes the current headset world X/Z from the current
  stage-to-world transform, compares it to the body/expected-head X/Z, and
  tries to move the body by that delta through normal shared collision.
- If collision allows the move, the body catches up and the stage-to-world
  transform is rebuilt from the post-reconcile body pose before controller/head
  poses feed locomotion, interaction, rendering, and debug overlays.
- If collision blocks some or all of the delta, only that blocked remainder may
  remain as residual head/body separation. If there is visible separation and
  no collision is blocking reconciliation, that is a bug.

Vertical policy:

- Physical HMD Y does not directly lift or drop the player body.
- Sitting, standing, crouching, and leaning are head/view behavior inside the
  body envelope first; dynamic player-height policy is deferred.
- Leaning or walking the headset forward into a one-meter block must not
  implicitly jump, auto-step, or climb the body onto the block.
- Body vertical movement remains explicit engine locomotion: jump, gravity,
  falling, hand-push climbing, and later optional auto-jump/step-assist if we
  intentionally add it.
- Head collision is handled as comfort/diagnostic state first: when the head
  sphere penetrates geometry or residual separation exceeds a threshold, fade
  or gray/black out rather than silently desynchronizing or teleporting.

Longer-term movement experiments can add XR-specific step assist, crouch
height policy, or different world scale, but those should be explicit features
with their own thresholds and validation. They should not fall out of
room-scale reconciliation accidentally.

## XR Comfort Fade Policy

Comfort fade is a communication layer, not locomotion authority. The engine
still tries to reconcile the body to the headset through shared collision
first. Fade communicates the cases where the view is not currently trustworthy:
the room-scale horizontal residual is collision-blocked, or the headset/head
sphere overlaps world collision.

First-pass rules:

- The fade target is computed from the stereo headset center and applied with
  the same alpha to both eyes. Per-eye alpha is forbidden for this effect.
- Vertical HMD offset alone does not fade. Sitting, crouching, and standing are
  view-height behavior until we intentionally add a body-height policy.
- Horizontal residual has a dead zone, ramps smoothly, and caps below full
  blackout so the player never fully loses the world.
- Head-sphere penetration forces a visible baseline fade even when horizontal
  residual is small.
- Alpha is smoothed over short in/out windows so brief frame jitter does not
  flash the display.
- The visual uses `ScreenEffectsRenderer`, including its full-frame multiview
  path. It is not a one-eye or app-local render pass.
- World debug lines, selection outlines, and menu panels render after the fade
  so the player can still see diagnostics and recover.

Thresholds are intentionally conservative until Quest validation: residual
dead zone `0.15` blocks, full ramp by `0.6` blocks, penetration alpha `0.65`,
and max opacity `0.9`.

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

## Slice 4 - Room-Scale Body Reconciliation

- [x] Add a shared engine room-scale reconciliation API that accepts the
  desired headset/world position, consumes horizontal X/Z offset through normal
  player collision, and reports the consumed and residual offsets.
- [x] Call reconciliation in `mclone-xr-scene` before stick/hand-push
  locomotion, then rebuild the stage-to-world transform from the updated body
  pose before deriving hand-push input, controller rays, render views, and
  debug lines.
- [x] Keep vertical HMD motion out of body movement in this slice; preserve
  existing jump/gravity/hand-push vertical behavior.
- [x] Extend the debug overlay with a body-eye-to-headset residual line or
  marker so headset/body separation is inspectable alongside the player box
  and hand colliders.
- [x] Add focused tests for clear-space room-scale catch-up, blocked horizontal
  reconciliation, transform rebuild ordering, and "vertical HMD offset does
  not auto-step" behavior.

## Slice 5 - Head Comfort Fade

- [x] Add a shared collision helper to test whether a head sphere intersects
  solid world collision without over-detecting from the sphere probe AABB
  corners.
- [x] Add a reusable solid-color `ScreenFadeOverlay` to `ScreenEffectsRenderer`
  with both per-eye and full-frame multiview render paths.
- [x] Add XR comfort state that derives target alpha from blocked horizontal
  residual offset and head-sphere penetration, then smooths the visible alpha.
- [x] Apply the same comfort fade alpha to both eyes; expose the state in
  `XrTerrainFrameSummary` for non-visual diagnostics.
- [x] Keep fade ordering before world overlays/menu panels so debug lines and
  recovery UI remain visible.
- [x] Add focused tests for residual threshold/cap behavior, vertical offset
  not fading, penetration alpha, smoothing, same-eye alpha, screen-effect
  multiview uniforms, and shared sphere/world collision.

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
- Head comfort fade is implemented, but the thresholds and subjective comfort
  need Quest standalone validation. It currently fades to black, not a
  Half-Life-style gray or vignette.
- Dynamic body height, crouch policy, step assist, and optional auto-jump are
  intentionally deferred.

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

Additional validation for Slice 4:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-render-session -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-room-scale-debug.png --width 960 --height 540 --startup-wait frames:2 --screenshot-player-box true --screenshot-camera-view third-person --fullbright true
```

Slice 4 unit coverage validates clear-space room-scale body catch-up, blocked
horizontal residual offset, vertical HMD offset not auto-stepping the body, and
tracking-origin consumption without double-counting the headset pose. The
headless screenshot visually confirmed the existing player-box world overlay
still renders after the residual-line path was added.

Additional validation for Slice 5:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render -p mclone-render-session -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-head-comfort-fade-sanity.png --width 960 --height 540 --startup-wait frames:2 --screenshot-player-box true --screenshot-camera-view third-person --fullbright true
```

Slice 5 unit coverage validates shared sphere/head obstruction checks,
comfort-fade target thresholds, smoothing, same-alpha stereo overlays, and
screen-effect multiview uniform serialization. The screenshot sanity check
visually confirmed the normal native frame still renders with the player
collision-box world overlay after the screen-effect renderer extension.

## Next Slice

Run Quest standalone with `Hand Push` and `Player Box` enabled. Validate that
physical room-scale walking in empty space keeps the collision box magnetized
to the headset, while leaning into blocked geometry leaves only
collision-explained residual offset and fades both eyes evenly. Tune fade
thresholds before changing arm length, hand/head radii, unstick distance, or
velocity fling thresholds.
