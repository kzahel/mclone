# 152: XR Room-Scale Body Follow Hysteresis

Status: active; Slices 1-2 code landed on 2026-07-06 after headset feel
investigation. Headset validation remains required before closing.

Workstream: native Rust XR locomotion, shared render-session camera/player
movement, and XR scene tracking-origin policy. This is not a Gorilla
locomotion tuning slice, though hand-push has a follow-up ownership issue once
the normal room-scale fix is validated.

## Purpose

Fix subtle stationary XR "swimming" where the rendered world appears to drift
or float independently from the user's real headset pose. The current likely
cause is the room-scale body/head reconciliation path converting microscopic
OpenXR pose jitter into player body movement, collision work, tracking-origin
rebases, and possibly network-visible position deltas.

The fix should keep the useful property from tactical `109`: the player body
and collision box must not drift arbitrarily away from the headset. The target
is a discrete, one-way body follow policy:

> The HMD render pose is authoritative every frame. The body/capsule follows
> the HMD's horizontal room-scale residual only after a meaningful threshold is
> crossed. When body movement is consumed, rebase the tracking origin so the
> current headset world pose remains fixed.

## Pre-Fix Diagnosis

Relevant code paths before Slice 1:

- `mclone-xr-scene::XrMcloneTerrainState::apply_locomotion_input(...)` calls
  `reconcile_room_scale_body_to_headset(&views)` before UI, snap turn, blink,
  stick movement, and hand-push input are applied.
- `mclone-xr-scene::tracking::reconcile_room_scale_body_to_headset(...)`
  computes the headset world position, calls
  `EngineCameraController::reconcile_room_scale_headset(...)`, then calls
  `XrTrackingOrigin::consume_world_movement(...)` with the consumed body
  movement.
- `mclone-render-session::camera::reconcile_room_scale_headset(...)` used
  `ENGINE_ROOM_SCALE_RECONCILE_EPSILON = 1.0e-9`, so effectively any finite
  horizontal residual becomes requested body movement.
- `mclone-xr-scene` already has tests proving that consumed body movement
  rebases the stage origin without double-counting the head. The rebase idea is
  correct; the problem is that it runs for residuals far below any perceptual or
  gameplay threshold.

This explains the reported symptoms:

- Standing as still as possible can still produce tiny HMD tracking residuals.
- Those residuals can become tiny body/collision/tracking-origin updates.
- The updates are not actual user locomotion, so they feel like world-space
  drift or Brownian swimming.
- During deliberate head motion, the artifact is masked by larger real motion.

Normal server correction is not currently the primary suspect. Accepted local
movement is not continuously corrected by the integrated server, and the client
already has a larger position-sync threshold than the room-scale epsilon. Server
position corrections should remain explicit warp/rebase events, not a smoothed
stationary drift source.

## External References

These references are useful for the intended shape, not for literal copying:

- Unity XR Interaction Toolkit locomotion model:
  <https://docs.unity3d.com/Packages/com.unity.xr.interaction.toolkit@3.0/manual/locomotion.html>
- Unity `XRBodyTransformer` docs:
  <https://docs.unity3d.com/Packages/com.unity.xr.interaction.toolkit@3.0/manual/xr-body-transformer.html>
- Unity continuous movement source mirror:
  <https://raw.githubusercontent.com/needle-mirror/com.unity.xr.interaction.toolkit/master/Runtime/Locomotion/Movement/ContinuousMoveProvider.cs>
- Godot XR Tools `PlayerBody`, which keeps a body under the XRCamera but uses
  gameplay-scale thresholds and comfort behavior:
  <https://raw.githubusercontent.com/GodotVR/godot-xr-tools/master/addons/godot-xr-tools/player/player_body.gd>
- Godot XR Tools direct movement provider:
  <https://raw.githubusercontent.com/GodotVR/godot-xr-tools/master/addons/godot-xr-tools/functions/movement_direct.gd>
- Another Axiom GorillaLocomotion reference:
  <https://github.com/Another-Axiom/GorillaLocomotion>
- GorillaLocomotion `Player.cs`, useful mainly for meaningful movement
  thresholds, head validation, and hand-push follow-up context:
  <https://raw.githubusercontent.com/Another-Axiom/GorillaLocomotion/main/Player.cs>

Common pattern from the references:

- Locomotion systems apply explicit body/origin transformations.
- Tiny numerical residuals are not treated as player locomotion.
- HMD/camera tracking remains the immediate render source.
- Body/capsule correction uses meaningful thresholds, collision, and comfort
  behavior rather than a floating-point epsilon chase.

## Target Model

Use two separate concepts:

- **Render pose**: the headset pose from OpenXR transformed into world space.
  This updates every frame and should not be pulled toward the body by normal
  reconciliation.
- **Body follow rebase**: a discrete correction that moves the player
  body/capsule toward the headset's horizontal room-scale residual, then
  rebases the tracking origin to preserve the headset's world-space position.

Basic algorithm:

```text
head_world_before = current OpenXR headset pose transformed to world
body_eye = current player/body eye position
horizontal_residual = head_world_before.xz - body_eye.xz

if body_follow_hysteresis permits movement:
    requested_body_delta = horizontal_residual, horizontal only
    consumed_body_delta = move body through normal collision
    tracking_origin.consume_world_movement(consumed_body_delta)
    head_world_after should equal head_world_before, within tolerance
else:
    do not move body
    do not touch tracking_origin
```

Hysteresis should be perceptual/gameplay-scale, not numerical:

- Candidate enter threshold: `0.03m` to `0.05m`.
- Candidate exit threshold: `0.01m` to `0.02m`.
- The existing local-player position sync threshold is roughly `3cm`, so the
  body-follow threshold should not be substantially smaller unless headset
  testing proves it needs to be.
- Below the exit threshold, there should be no player body movement, no
  tracking-origin mutation, no collision probe, and no network-visible movement.

The body-follow state should reset on explicit discontinuities:

- teleport or blink commit
- snap turn / artificial rotation that rebases around the HMD
- server position correction or pending teleport acknowledgment
- world/session start, recenter, or tracking-origin reset
- movement mode changes

## First Fix Scope

Keep the first slice focused on normal room-scale body follow:

- [x] Add a room-scale follow deadband/hysteresis state in the shared engine/XR
  boundary where it can make one decision per XR locomotion frame.
- [x] Replace the `1.0e-9` effective movement threshold with named thresholds that
  can be tuned from headset testing.
- [x] Preserve the existing collision-backed body catch-up behavior once the enter
  threshold is crossed.
- [x] Preserve the existing tracking-origin consume/rebase invariant so body
  movement does not move the rendered HMD world pose.
- [x] Keep vertical HMD movement out of body movement.
- [x] Keep snap turn and teleport as explicit rebases, not part of stationary
  reconciliation.
- [x] Add tests for no-op micro jitter, enter/exit hysteresis, collision residual,
  and headset-world-position preservation across a consumed body move.

Do not disable body follow entirely. That reintroduces the confusing state
where the player collision box can become arbitrarily desynchronized from the
physical headset position.

Slice 1 landed details:

- `mclone-render-session::EngineCameraController` now tracks whether
  room-scale body follow is active.
- Enter threshold is `0.03m`; exit threshold is `0.01m`.
- Residuals below the current threshold return a reconciliation no-op, leaving
  player body, collision state, tracking origin, and network-visible movement
  untouched.
- Fully consumed clear-space follow movement exits active follow immediately.
- Collision-blocked residual can keep follow active so the body keeps trying to
  catch up while comfort diagnostics report the remaining head/body offset.
- Explicit discontinuities reset the follow state: eye/feet pose changes,
  movement/collision mode changes, yaw-turn rebases, and accepted server
  position updates.

## Slice 2 - Blocked Residual Stability

Headset validation after Slice 1 reported that standing still felt good in open
space, but pressing the HMD/body target slightly into a wall still produced a
one-dimensional swim along the wall edge. The likely mechanism is:

- A collision-blocked reconciliation leaves a large residual, so body follow
  remains active.
- Each tiny HMD jitter while the target remains inside the wall becomes another
  collision-backed movement attempt.
- The collision solver rejects the wall-normal component, but can still consume
  tiny wall-tangent components, so the body swims along the edge.

Landed policy:

- [x] After a collision-blocked reconciliation, remember the remaining
  horizontal residual as a blocked-residual anchor.
- [x] While the current residual stays within `0.05m` of that anchor, report a
  no-op reconciliation instead of retrying collision.
- [x] Clear the blocked-residual anchor when the residual drops below the normal
  body-follow threshold, an explicit discontinuity happens, or the residual
  changes enough to represent deliberate motion.
- [x] Preserve deliberate movement along/out of the wall by retrying once the
  blocked residual changes by more than the blocked retry threshold.
- [x] Add focused tests for wall-edge jitter suppression and meaningful
  blocked-residual retry.

## Gorilla / Hand-Push Follow-Up

The experimental hand-push mode likely has a related but distinct ownership
problem:

- The XR scene currently reconciles room-scale body-to-headset before it builds
  hand-push input.
- Hand-push then ticks in the same frame and mutates the same
  `LocalPlayerController`.
- The hand-push controller stores previous head, hand, and player world
  positions; it is not explicitly informed that an earlier room-scale body
  follow event happened in the same frame.

Do not include this in the first stationary-swim fix unless headset validation
shows it is unavoidable. After the normal body-follow hysteresis fix is proven,
audit hand-push ownership separately. Likely options:

- In `HandPush` mode, let hand-push own artificial body movement for the frame
  and run room-scale follow only as a post-solve capsule-near-head constraint.
- Or skip room-scale body follow while hands are actively braced, then re-enable
  it after hand release with hysteresis reset.
- Or teach hand-push about consumed room-scale body movement so its stored
  previous head/hand/player positions and velocity history remain coherent.

2026-07-07 audit result:

- `HandPush` mode now suppresses room-scale body-follow reconciliation inside
  `EngineCameraController::reconcile_room_scale_headset`.
- XR tracking can keep calling the reconciliation hook, but the shared camera
  controller returns zero consumed body movement, clears room-scale follow state,
  and leaves the hand-push collider for hand-push locomotion to own for that
  frame.
- This keeps stale blocked residuals out of comfort/debug consumers while
  avoiding the same-frame player-position mutation that could pollute
  hand-push velocity history.
- Future headset tuning may still add a post-solve capsule-near-head constraint
  for `HandPush`; this slice intentionally avoids pre-solving the body before
  hand collision input is applied.

## Validation

Required before closing:

- Focused Rust tests for threshold no-op, threshold enter/exit, blocked
  residual, and rebase-preserves-HMD-world-position.
- Headset validation standing still with continuous movement disabled and hands
  off controls.
- Headset validation that deliberate room-scale walking still keeps the body
  from drifting far away from the HMD.
- Headset validation that snap turn and teleport remain discrete, explicit
  comfort events.
- Confirm no new stream of local-player position sync updates appears while the
  user is physically stationary under the deadband.
