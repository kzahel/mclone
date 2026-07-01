# 124: XR Input Sanity And Snap Turn

Status: complete. Shared input/UI implementation, automated gates, and headset
spot validation have passed.

## Purpose

Fix the current XR controller mapping and add comfort snap-turn before tuning
more locomotion. The immediate user problem is that jump did not work reliably
in headset testing, and the current right-stick mapping combines jump/descend
on Y with smooth yaw on X, which is disorienting and easy to cross-talk.

This is a shared XR scene/input slice. Desktop OpenXR and Android XR must both
consume the same policy through `mclone-xr-scene`; app crates should only
provide OpenXR snapshots and frame timing.

## Current Wiring

- `mclone-xr-host::XrControllerSnapshot` already exposes `a_pressed` for the
  right controller and `y_pressed` for the left controller.
- `mclone-xr-scene::xr_locomotion_input_from_controllers(...)` currently maps:
  - left stick X/Y to continuous movement,
  - right stick X to smooth yaw,
  - right stick Y up/down to jump/descend.
- `mclone-render-session` receives jump through `EngineCameraInput` and feeds
  it into the shared player controller. In `HandPush` mode, walking physics is
  still ticked after hand movement, so jump should be possible once the input
  is sane. If it still fails after this slice, the likely bug is hand-push
  ground state or movement ordering, not OpenXR button plumbing.

## Target Shape

- Right A is the default XR jump button.
- Right-stick Y no longer drives jump in normal walking/hand-push locomotion.
  If no-clip vertical control remains useful, bind it explicitly to no-clip
  policy rather than mixing it into all XR locomotion.
- Right-stick X defaults to snap turn, not smooth turn.
- Snap turn is a stateful input policy with hysteresis: one turn per stick
  deflection, then the stick must return near center before another turn.
- Default snap angle is `15` degrees. The implementation should make `30` and
  smooth-turn options easy to expose later, but the first headset-feel target
  is the user's requested `15` degree snap.
- Snap turn rotates the authoritative player/body yaw and keeps the tracked
  headset world position stable. The user should perceive the world snapping
  around their current head position, not the head sliding around the body.
- The implementation stays shared. No desktop-only or Android-only input fork.

## Design Notes

Snap turn is an artificial locomotion event, not camera-only render math. It
must update the same engine pose that continuous yaw updates today, then update
the XR tracking origin so the current HMD world position remains fixed across
the turn.

The important invariant after a snap turn is:

> The player's yaw changes by the snap amount, but the current headset world
> position does not jump laterally.

This keeps room-scale reconciliation understandable. The body can still
magnetize horizontally to the headset on later frames, but snap turn itself
should not inject an unexplained head/body residual.

## Slice 1 - Jump Binding Cleanup

- [x] Move XR jump to right A (`XrControllerSnapshot::a_pressed`).
- [x] Stop using right-stick Y as jump/descend in walking and hand-push modes.
- [x] Decide whether no-clip keeps a separate explicit vertical mapping in XR,
  or whether no-clip descend remains unbound until a controls UI exists. No-clip
  descend remains unbound for XR until a dedicated binding/remapping UI exists.
- [x] Add focused tests in `mclone-xr-scene`:
  - right A maps to `EngineCameraInput.jump`,
  - right-stick up no longer maps to jump in normal XR locomotion,
  - right-stick X does not accidentally create jump/descend,
  - `HandPush` mode still receives jump through the same engine input path.
- [x] Validate on Quest that jumping works from the ground in `Walk` and
  `Hand Push` modes.

## Slice 2 - Shared Snap Turn

- [x] Add an XR turn policy enum in `mclone-xr-scene`, for example
  `Snap { degrees }` and `Smooth`.
- [x] Add `XrSnapTurnState` with press/recenter thresholds. Suggested first
  thresholds:
  - engage at `abs(x) >= 0.65`,
  - recenter at `abs(x) <= 0.25`.
- [x] Replace default right-stick smooth yaw with `15` degree snap turn.
- [x] Apply snap turn through the shared engine camera/player yaw path.
- [x] Recompute or consume the XR tracking origin so the current headset world
  position is stable across the snap.
- [x] Add unit tests for:
  - one snap per deflection,
  - no repeat while the stick remains held,
  - recenter permits the next snap,
  - left/right signs match headset feel,
  - headset world position is preserved across a snap turn.

## Slice 3 - Options And Validation

- [x] Expose a minimal XR turn mode option in the shared UI/options path:
  `Snap 15`, `Snap 30`, and `Smooth`.
- [x] Keep `Snap 15` as the Quest default until headset validation says
  otherwise.
- [x] Add desktop OpenXR and Android XR compile gates.
- [x] Run Quest standalone/headset validation with imperfect frame pacing and
  hand-push enabled, since that is the comfort-sensitive case.

## Validation Plan

Required automated gates for implementation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-render-session
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
git diff --check
```

Manual headset validation:

- Right A jumps in `Walk`.
- Right A jumps in `Hand Push`.
- Right stick left/right snaps by the configured angle with no smooth drift.
- Holding right stick to one side produces exactly one snap until recentered.
- Snap turn does not create a visible headset/body residual line in open space.
- Comfort fade still works when the head/body residual is collision-blocked.

## Out Of Scope

- Blink/Shift teleport locomotion. Track that in
  [`125-xr-teleport-comfort-locomotion.md`](125-xr-teleport-comfort-locomotion.md).
- Full controls-remapping UI.
- Hand-relative continuous locomotion.
- Reworking the room-scale head/body reconciliation policy from
  [`109`](109-xr-hand-push-locomotion.md).
