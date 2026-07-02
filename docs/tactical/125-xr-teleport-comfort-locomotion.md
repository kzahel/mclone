# 125: XR Teleport Comfort Locomotion

Status: active; the input/snap-turn prerequisite in
[`124`](124-xr-input-snap-turn.md) is implemented and headset-validated.
Slice 1 code landed on July 1, 2026.

## Purpose

Add comfort locomotion options modeled after Half-Life: Alyx's Blink and Shift
movement. Continuous left-stick locomotion is useful but can feel nauseating
when headset frame pacing is imperfect. Blink/Shift should give the player a
room-scale-friendly way to relocate the authoritative body, including moving to
valid nearby higher surfaces, without desynchronizing the headset, collision
box, hands, or server pose.

## References

- Half-Life: Alyx movement modes: Blink is teleport with a brief fade, Shift is
  teleport with a fast linear movement, and Continuous remains available for
  players who prefer stick movement.
- Public summaries:
  - `https://www.uploadvr.com/valve-deep-dive-locomotion/`
  - `https://www.gamespot.com/articles/half-life-alyx-accessibility-options-full-guide/1100-6475045/`
  - `https://www.newsweek.com/half-life-alyx-vr-gameplay-pc-specs-vive-oculus-movement-1490121`
- Local pathfinding reference:
  `native/crates/mclone-server/src/entity/mob/navigation/`.

## Current Problem

The current XR locomotion stack has three ingredients but no comfort teleport
mode:

- continuous left-stick movement through shared player collision,
- room-scale head/body reconciliation,
- head-comfort fade when the head is obstructed or residual offset is blocked.

That is enough for walking and hand-push experiments, but it still relies on
continuous artificial movement. Quest testing showed that continuous movement
plus imperfect frame pacing can become uncomfortable.

We also have a pathfinding implementation, but it is nested under server mob
AI. That makes it awkward to reuse for player teleport preview/validation,
where the shared algorithm should be core and the mob-specific evaluator should
remain mob-specific.

## Target Shape

- Comfort teleport is a shared engine/client feature, with XR as the first
  product-facing input and presentation surface. The path query, target
  validation, and body relocation code must not be compiled only for XR.
- Desktop flat and offscreen/headless may expose Blink as a debug/test
  locomotion path so the shared evaluator, server pose sync, and render
  invalidation can be validated without a headset.
- `mclone-xr-scene` owns XR-specific controller mapping, stereo preview
  presentation, comfort fade, and tracking-origin rebasing. It should consume
  the shared teleport evaluator rather than own it.
- Teleport moves the authoritative body/player pose through shared
  engine/client contracts. It must not be app-local stage-origin glue or
  XR-only camera math.
- The current headset world position remains understandable across teleport:
  after relocation, the stage-to-world transform is rebuilt from the new body
  pose so render views, hands, rays, debug overlays, and server pose agree.
- Blink and Shift share target acquisition, path validation, and preview. Both
  modes require the same validated reachable path to the target; Blink must not
  allow destinations that Shift would reject. They differ only in execution
  comfort:
  - **Blink**: fade out/in and relocate immediately while faded.
  - **Shift/Warp**: move quickly along the same validated path with a comfort
    transition.
- Preview uses existing multiview-aware world overlay paths such as
  `WorldGuiLine` and world GUI panels where possible. Any new renderer must
  include both per-eye and full-frame multiview paths.
- Pathfinding core moves out of `mclone-server::entity::mob::navigation` into
  a shared module/crate before XR depends on it.

## Input Model

The first XR Blink control target is concrete:

- A movement-mode option selects Continuous, Blink, Shift, or Hand Push.
- In Blink, the left analog joystick is the arm/commit control.
- Pushing the left analog joystick more than `0.5` magnitude in any direction
  starts the Blink intent preview.
- While the stick remains past the threshold, the preview refreshes from the
  latest controller/camera aim.
- The preview shows where the player's feet/body would land.
- Returning the stick below the threshold commits the latest still-valid
  preview result.
- Blink has no cancel gesture. Once armed, release commits; if no valid
  completed result exists, commit is a no-op.
- Stick direction may influence landing yaw, so the player can choose the
  facing direction before committing, but any direction over threshold arms
  Blink.

The target preview should not conflict with the right-stick snap-turn work from
`124`. Snap turn remains right-stick based. Blink/Shift target selection should
not bind to the right stick.

## Desktop Flat Debug Input

Desktop flat should have an interactive Blink debug path for testing the shared
resolver without a headset. This is not a separate locomotion algorithm; it is
an input/presentation adapter over the same target query, preview result, and
body relocation contract used by XR.

First debug input shape:

- Holding a debug binding starts or refreshes the Blink intent preview.
- Moving the mouse/camera while held updates the aim direction.
- Releasing the binding commits the latest still-valid preview result.
- If there is no completed valid result on release, commit is a no-op.
- A cancel binding may clear the preview without committing.

The flat debug aim should use a synthetic left-hand pose rather than the exact
camera center ray. Start the intent arc from a camera/body-local offset that is
slightly left of player center and visible from the normal view. Aim should be
based on camera forward with a small upward pitch bias from the screen center or
crosshair so the arc begins just above the vertical centerline and is visible
while the user steers it with mouse look.

## Pathfinding Refactor Policy

The pathfinding algorithm itself does not belong under passive mob AI. The
refactor should separate:

- **Shared path core**: A* frontier/open-set logic, search limits, generic
  path result, nearest-reachable target behavior, and deterministic tests.
- **Mob navigation evaluator**: Java-shaped passive mob walkability,
  block-path types, malus, width/height rules, and follow-range behavior.
- **Player teleport evaluator**: player-body/feet target validation, headroom,
  step-up policy, unloaded-chunk rejection, and preview path facts usable by XR,
  desktop flat debug, and offscreen/headless tests.

Implementation shape:

- Prefer a small shared crate such as `mclone-path` or a clearly named core
  navigation module over putting reusable A* in `mclone-server`.
- Keep `mclone-server` mob AI depending on the shared path core.
- Let `mclone-client` or another shared client/engine crate own player teleport
  target validation against the client world snapshot and collision facts.
- Do not make `mclone-xr-scene` depend on server mob AI to compute a player
  teleport target.
- Do not hide Blink/Shift target validation or body relocation behind an XR-only
  feature flag. Platform-specific input, preview, and comfort presentation may
  be gated; the evaluator and commit contract should stay shared.

## Runtime And Worker Topology

Teleport preview must be responsive and must not run expensive path search on a
render/frame thread. The first implementation should use a client-side
latest-only worker mailbox for interactive preview surfaces:

```text
XR or flat frame thread
  read controllers/views or flat debug input
  submit or refresh the latest target/path request
  render the latest completed preview result
  commit only a still-valid completed result

teleport path worker
  receives compact client-world collision facts and request parameters
  runs bounded target/path search
  returns target feet pose, yaw, validity reason, and path facts

host/server thread
  remains out of the preview loop
  may later reuse the same shared path core for validation/budget policy
```

The worker should not borrow `ClientRuntime` live. Give it a compact immutable
query window or collision snapshot for the bounded search area, plus a client
world revision/generation and request sequence. Drop stale results whose
sequence or revision no longer matches.

Platform notes, without allowing a divergent runtime topology:

- Desktop OpenXR and Android XR use native OS worker threads or a native worker
  pool behind the shared `mclone-xr-scene` / `mclone-client` request path.
- Desktop flat debug/offscreen tests should use the same shared evaluator and
  may reuse the native mailbox when they need interactive preview. Deterministic
  tests can call the evaluator directly with bounded fixtures.
- Web/WASM has no supported XR target today, so this slice does not need a
  browser XR presentation surface. That is not permission for a reduced or
  synchronous web runtime topology: a future flat web debug surface should use
  the same latest-only client-side mailbox shape with a Web Worker backend for
  interactive preview.
- Keep `mclone-path` and the player teleport evaluator wasm-compatible where
  practical, so future web/non-XR callers can share the same algorithm,
  target-validity contract, and worker-backed request lifecycle.

The first mailbox should be latest-only and bounded: one in-flight request per
XR scene, pending requests replaced as aim/stick input changes, hard node/time
limits, and final cheap endpoint revalidation on commit.

## Teleport Intent Resolver Sketch

The player-facing behavior should be closer to Half-Life: Alyx Blink/Shift than
to an exact raycast validator. The user should not have to inspect a valid/invalid
state during combat. Aiming should almost always resolve to a usable reachable
landing, even if that landing is short because the intended direction is blocked.

Important preview contract:

- The feet marker is the exact committed landing pose.
- The dot is vertically aligned with the feet marker and communicates how far
  the teleport will go even when the user is not attending to the feet marker.
- Blink and Shift commit the same resolved `target_feet`; only the comfort
  transition differs.
- Commit may cheaply revalidate the same pose against the current world
  revision, but it must not silently choose a different landing than the preview.

First-pass algorithm shape:

1. Build a fixed-range intent arc from controller/camera pose. This should be a
   bounded parametric arc, not an unconstrained physics projectile; raising the
   hand should not extend range without limit.
2. Search reachable player-foot candidates in a bounded tube/cone around that
   arc and intent direction. Use `mclone-path` for coarse reachability, but keep
   the search intent-constrained rather than general NPC navigation.
3. Treat obstructions as stopping/bounce-like constraints. If the aim points at
   a wall or a body-too-small opening, prefer the best reachable body-valid
   landing before the obstruction over routing around to a surprising far target.
4. Score candidates by forward progress along the intent, proximity to the arc,
   reachable path cost, stable support, headroom/body clearance, and modest
   vertical change. Invalid should mean no reasonable reachable candidate exists
   inside the loaded/searchable window.
5. Refine the winning coarse foot cell into a continuous `Vec3d` feet pose.
   The player body is narrower than one block, so final placement must validate
   the actual standing AABB, headroom, and support footprint rather than snapping
   to block centers.
6. Derive the visual dot from the resolved landing pose. The invariant should be
   `marker_dot.xz == target_feet.xz`; the arc can visually fade/truncate around
   that marker.

The path nodes are diagnostics and a Shift/Warp execution guide. The committed
landing is the refined continuous feet pose, not a raw `BlockPos`.

## Slice 1 - Extract Shared Path Core

- [x] Add a shared pathfinding core with generic neighbor expansion and target
  acceptance callbacks.
- [x] Preserve current mob path behavior by adapting
  `mclone-server/src/entity/mob/navigation/path_finder.rs` to the shared core.
- [x] Keep `WalkNodeEvaluator`, `BlockPathType`, and mob malus policy in the
  server mob module.
- [x] Add behavior-equivalence tests around existing mob navigation fixtures.
- [x] Add standalone shared path tests for nearest-reachable target, search
  limit behavior, deterministic tie breaking, and empty/no-path cases.

Landed:

- Added `mclone-path`, a small shared block-grid pathfinding crate with bounded
  A* search, generic neighbor expansion, target acceptance callback,
  nearest-reachable fallback, deterministic open-set tie breaking, path
  reconstruction, and diagnostics.
- Rewired `mclone-server` mob navigation so
  `entity/mob/navigation/path_finder.rs` computes the Java-shaped mob start and
  walk neighbors through `WalkNodeEvaluator`, then delegates the A* search to
  `mclone-path`.
- Kept `WalkNodeEvaluator`, `BlockPathType`, mob malus policy, width/height,
  follow-range shaping, and `GroundPathNavigation` server-local.
- Added focused `mclone-path` tests for direct paths, obstacle routing,
  exhausted search budgets, empty frontier/no-path fallback, deterministic tie
  breaking, and target acceptance callbacks. Existing server mob navigation
  tests now exercise the shared path core through the server adapter.

## Slice 2 - Player Teleport Target Query And Worker

- [x] Add a shared target query that consumes:
  - current body/player pose,
  - controller/headset world pose, flat debug ray, or intended aim/stick
    direction,
  - loaded client world block/collision facts,
  - max distance and step-up/drop limits.
- [x] Return a target feet pose, target yaw, validity reason, and optional
  preview path.
- [x] Require a reachable path result for both Blink and Shift; no direct-only
  Blink exception.
- [x] Allow valid one-block-up landing when there is foot support and headroom.
- [x] Reject unloaded chunks, solid body overlap, insufficient headroom, and
  targets that would place the head/body into obstruction.
- [x] Keep vertical room-scale HMD motion out of target validity. Teleport can
  intentionally change body height; leaning forward into a block cannot.
- [ ] Add a native latest-only worker mailbox for desktop OpenXR and Android XR
  so path search stays off the XR render/frame thread.
- [x] Add a flat/offscreen test entry point that exercises the same query and
  target validation without requiring XR.
- [x] Add the desktop flat debug input adapter: hold-to-preview, mouse-look aim
  updates, release-to-commit, and synthetic left-hand origin/upward-biased aim
  feeding the shared query.
- [ ] Keep web/WASM XR-worker support documented as intentionally absent while
  web has no XR target, without making the shared evaluator non-wasm or XR-only.

Landed:

- Added `mclone-client::teleport` with shared `TeleportIntent`,
  `TeleportConfig`, `TeleportPreview`, `TeleportValidityReason`, and
  `TeleportCollisionWorld`.
- Implemented a synchronous resolver that builds a bounded fixed-range intent
  arc, searches body-valid standing candidates near the intent, uses
  `mclone-path` for reachability and nearest reachable fallback, and refines
  the final target to a continuous feet pose rather than a block center.
- The resolved dot/marker contract is represented in the preview as
  `marker_dot`, with `marker_dot.xz == target_feet.xz`.
- Added deterministic flat/offscreen unit tests for wall stop, too-small
  opening rejection, one-block-up landing, unloaded/no-candidate invalid
  behavior, and off-block-center continuous placement.
- Added a desktop flat debug adapter on the `T` key: hold to arm/update Blink
  preview from mouse-look aim, release to commit the latest valid result or
  no-op, using a synthetic left-hand origin and slight upward pitch bias.
- Added provisional desktop debug world lines for the preview arc, feet marker,
  and vertically aligned dot. This validates the shared resolver interactively;
  the full XR/multiview preview overlay remains part of Slice 3.

## Slice 3 - Preview Overlay

- [ ] Render a world-space target feet marker.
- [ ] Render the dot directly above the resolved feet marker.
- [ ] Render a facing arrow or short body-forward line.
- [ ] Render the candidate path/arc/line, with invalid previews visibly
  different from valid previews.
- [ ] Keep the desktop flat debug arc visible from the normal camera by using
  the synthetic off-center origin and upward-biased aim.
- [ ] Route through existing multiview-aware world GUI/overlay paths where
  practical.
- [ ] Add headless or synthetic render coverage for the overlay renderer path.

## Slice 4 - Blink Execution

- [ ] Add Blink locomotion mode.
- [ ] Commit the path-validated target by moving the authoritative body/player
  pose, not only the XR stage transform.
- [ ] Use `ScreenEffectsRenderer` or the existing comfort fade primitive for a
  brief same-alpha stereo fade.
- [ ] Rebuild tracking origin after the relocation so the current headset pose,
  hands, rays, collision box, and debug overlays agree.
- [ ] Add tests for relocation, server pose sync command generation, and
  rejected target no-op behavior.

## Slice 5 - Shift/Warp Execution

- [ ] Add Shift/Warp locomotion mode using the same target query, path, and
  preview as Blink.
- [ ] Move the body quickly along the validated path with a short comfort
  transition.
- [ ] Keep collision authoritative during the transition. If the path becomes
  invalid due to world changes, cancel or finish safely rather than clipping.
- [ ] Decide whether Shift uses full fade, edge vignette, or the existing head
  comfort darkening; validate in headset.

## Slice 6 - Options, Diagnostics, And Quest Validation

- [ ] Add shared UI/options for Continuous, Blink, Shift, and Hand Push.
- [ ] Add debug status for target validity reason and selected locomotion mode.
- [ ] Validate on Quest standalone:
  - preview appears in both eyes,
  - body/collision box lands at the preview,
  - headset pose stays coherent after relocation,
  - one-block-up targets feel intentional,
  - invalid targets are obvious,
  - Blink is comfortable under imperfect frame pacing,
  - Shift is not more nauseating than continuous movement.

## Open Design Questions

- After the left analog joystick arms Blink, should the resolved arc aim use
  controller pose, headset/camera forward, or a blend?
- Should the landing yaw come from stick direction, current headset yaw, or a
  separate twist/turn input?
- What maximum distance is comfortable in Minecraft-scale terrain?
- Should one-block-up teleport be always allowed, or only when the preview path
  visibly shows the climb?
- Should the execution use an instantaneous server correction-like pose set, or
  a short sequence of normal movement commands? The first implementation should
  prefer correctness and pose coherence over multiplayer anti-cheat concerns.

## Validation Plan

Passed on July 1, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-path
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-path -p mclone-client -p mclone-server -p mclone-xr-scene
```

Required automated gates once implementation starts:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-path -p mclone-client -p mclone-server -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
git diff --check
```

If the shared path core is a module rather than a new crate, replace
`-p mclone-path` with the owning crate's focused test command.

## Out Of Scope

- Multiplayer anti-cheat or hostile-server validation for teleport.
- Full navmesh generation.
- Entity AI behavior changes beyond preserving existing mob path behavior
  during the path-core extraction.
- Replacing continuous movement or hand-push. Blink/Shift are comfort options,
  not a removal of the existing modes.
