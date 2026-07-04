# 138: Flat Render Camera Pose

Status: complete; flat pose route, sky route, and cleanup landed 2026-07-04
Workstream: native Rust, shared render/session boundary, desktop validation first

## Purpose

Replace the flat-client render camera's `eye + target + world_up` shape with a
stable renderer-facing pose/basis. The immediate bug is that looking straight up
or down can make terrain disappear because the flat path reconstructs a view
matrix through `Mat4::look_at_rh(eye, target, world_up)`. At exactly vertical
pitch, the view direction is collinear with `world_up`, the look-at right vector
is undefined, and the view-projection matrix can become invalid.

This tactical is not a gameplay pitch rewrite. Keep the local player/controller
state vanilla-shaped:

- player pose remains yaw/pitch/feet position,
- normal mouse turn remains clamped to Java's `[-90, 90]` `xRot` behavior,
- movement, protocol, block picking, actor rotation, and server sync keep using
  the existing yaw/pitch contracts.

The change belongs at the renderer-facing boundary: convert player yaw/pitch
into a stable render pose before building `ChunkRenderView`.

## Current Problem

Flat render lanes currently use this chain:

```text
EngineCameraController snapshot
  -> EngineRenderCamera { eye, target, up }
  -> app-local chunk_camera_from_engine(...)
  -> ChunkCamera::render_view(width, height)
  -> Mat4::look_at_rh(eye, target, up)
```

The important code paths are:

- `native/crates/mclone-render-session/src/lib.rs`
  - `EngineRenderCamera`
  - `render_camera_from_snapshot_with_view_mode(...)`
  - `view_forward(...)`
- `native/crates/mclone-render/src/chunk.rs`
  - `ChunkCamera`
  - `ChunkCamera::render_view(...)`
  - `ChunkRenderView::sky_view_projection(...)`
- flat adapters:
  - `native/apps/mclone-native-client/src/camera.rs`
  - `native/apps/mclone-native-client/src/flat_client_driver.rs`
  - `native/apps/mclone-web-client/src/web_canvas.rs`
  - `native/apps/mclone-android-client/src/lib.rs`
  - `native/crates/mclone-app-runtime/src/frame_render.rs`

The root issue is representational. `target + fixed world_up` is not a full
orientation. It works for most first-person views, but has a pole when the
view vector is parallel or anti-parallel to the chosen up vector.

XR does not have this failure mode because it already uses a full pose:

- `mclone-xr-host::view_pose(...)` reads the OpenXR quaternion.
- `mclone-xr-host::render_view_from_world_pose(...)` derives
  `camera_forward`, `camera_right`, and `camera_up` from that quaternion.
- the view matrix is built from `Mat4::from_rotation_translation(...).inverse()`.
- `mclone-xr-scene` passes the result into `ChunkRenderView` as an external
  projection/view.

Flat should converge on the same renderer-facing shape.

## Target Shape

Introduce a shared perspective render-pose contract that can build a
`ChunkRenderView` without `look_at_rh` and without a fixed world-up fallback.

The exact type names can change, but the data should be equivalent to:

```rust
pub struct PerspectiveRenderPose {
    pub eye: glam::Vec3,
    pub orientation: glam::Quat,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}
```

or:

```rust
pub struct PerspectiveRenderBasis {
    pub eye: glam::Vec3,
    pub forward: glam::Vec3,
    pub right: glam::Vec3,
    pub up: glam::Vec3,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}
```

Either representation is acceptable if it preserves these invariants:

- `forward`, `right`, and `up` are finite and non-degenerate.
- the basis is orthonormal or normalized close enough for rendering/culling.
- `view_projection` is finite at pitch `+90` and `-90`.
- sky rendering, terrain culling, translucent sorting, world GUI, entities,
  selection outlines, and screen effects all see the same camera basis.

Prefer a quaternion as the authoring/transport field if it reduces duplication,
but keep explicit `camera_forward/right/up` in `ChunkRenderView`; many render
paths already consume those directly.

## Ownership Boundary

`mclone-client` owns gameplay/player pose semantics. Do not move the player
controller to quaternions as part of this work.

`mclone-render-session` owns conversion from engine/player camera snapshots to
renderer-facing flat render poses:

- first-person eye pose,
- third-person-back render eye offset,
- fov/near/far policy,
- vanilla yaw/pitch to render orientation conversion.

`mclone-render` owns GPU-facing `ChunkRenderView` construction:

- perspective projection,
- view matrix from render pose/basis,
- finite-basis validation helpers,
- sky rotation-only view derived from the existing view/basis without a
  collinear look-at reconstruction.

App crates own only platform glue:

- desktop/web/flat Android call the shared conversion,
- no app crate should invent its own render-camera math,
- existing overview/headless/test cameras may stay as `ChunkCamera` temporarily
  if they are not player-controlled flat cameras.

XR remains the reference shape and should not be rewritten in this tactical
except for small helper reuse if it is clearly shared and low risk.

## Implementation Slices

### Slice A: Add Render-Pose Construction Beside The Old Path

- [x] Add a renderer-facing pose or basis type in `mclone-render-session` or
      `mclone-render`, whichever keeps dependencies cleanest.
- [x] Add a `ChunkRenderView` constructor that accepts the pose/basis directly.
- [x] Build the view matrix from quaternion/basis, not from `look_at_rh`.
- [x] Add unit tests for finite matrices at pitch `+90`, `-90`, and ordinary
      yaw/pitch combinations.
- [x] Add a third-person vertical-pitch test.
- [x] Leave existing `ChunkCamera` users working.

Result: `mclone-render` now has `PerspectiveRenderPose`, which builds a
`ChunkRenderView` from a finite quaternion orientation via
`Mat4::from_rotation_translation(...).inverse()`. The old `ChunkCamera` path is
unchanged for existing callers.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session`

### Slice B: Route Desktop Flat Through The New Pose

- [x] Replace desktop flat `EngineRenderCamera -> ChunkCamera` conversion with
      direct shared render-pose/view construction.
- [x] Keep `SpectatorCamera` as a compatibility shell if needed, but do not let
      it own new render math.
- [x] Verify first-person and third-person-back view modes.
- [x] Capture and inspect a desktop/offscreen frame looking straight down.
- [x] Capture and inspect a desktop/offscreen frame looking straight up.

Result: desktop/offscreen flat now carries `PerspectiveRenderPose` from
`mclone-render-session` into `mclone-app-runtime` full-frame rendering. The old
`ChunkCamera` method remains available for compatibility callers, while the
player-controlled flat path no longer reconstructs its render view through
`EngineRenderCamera -> ChunkCamera`.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-render-session -p mclone-app-runtime -p mclone-native-client`
- exact down capture inspected:
  `cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-flat-pose-down.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --startup-wait idle --screenshot-eye 8,80,8 --screenshot-target 8,79,8`
- exact up capture inspected:
  `cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-flat-pose-up.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --startup-wait idle --screenshot-eye 8,80,8 --screenshot-target 8,81,8`

### Slice C: Route Web And Flat Android Through The Same Shared Path

- [x] Update `native/apps/mclone-web-client/src/web_canvas.rs` to use the shared
      render-pose/view conversion.
- [x] Update `native/apps/mclone-android-client/src/lib.rs` to use the shared
      render-pose/view conversion.
- [x] Remove or shrink duplicated `chunk_camera_from_engine(...)` helpers where
      they only bridge the old target/up shape.
- [x] Preserve existing web and flat Android input semantics.

Result: native web and flat Android now build live player-camera
`ChunkRenderView` values from `EngineCameraController::render_pose(...)`. Web
keeps `ChunkCamera::overview_for_chunk_area(...)` only for deterministic
overview smokes, and native/headless compatibility cameras remain unchanged.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session -p mclone-web-client -p mclone-android-client`
- `pnpm native:web:build`
- `pnpm native:android:apk:avd`
- `pnpm native:web:chunk-smoke`
- direct live web app-camera screenshot inspected:
  `/tmp/mclone-native-web-live-camera-pose.png`

Note: `pnpm native:web:app-smoke` rendered many app frames but failed in its
later block-placement interaction probe with `place: miss`; the render-camera
path was separately validated with the direct live app-camera screenshot.

### Slice D: Sky And Secondary Render Paths

- [x] Replace `ChunkRenderView::sky_view_projection()` internals so it does not
      call `look_at_rh` with `camera_forward` and `camera_up`.
- [x] Audit entity, selection-outline, far-LOD, world-GUI, and screen-effect
      paths for assumptions that `camera_up` is world-up.
- [x] Keep XR multiview behavior unchanged.
- [x] Add tests that sky view-projection is finite for vertical flat views and
      remains translation-free.

Result: `ChunkRenderView::sky_view_projection()` now reuses the existing view
matrix basis with translation cleared instead of reconstructing a second
look-at view. The audit found no production world-up assumptions in entity,
selection-outline, far-LOD, world-GUI, or screen-effect render paths beyond
their existing `ChunkRenderView` basis use.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- ordinary offscreen capture inspected:
  `/tmp/mclone-sky-pose-ordinary.png`
- exact-up offscreen capture inspected:
  `/tmp/mclone-sky-pose-up.png`

### Slice E: Cleanup And Contract Hardening

- [x] Decide whether `EngineRenderCamera` should be replaced outright or kept as
      a deprecated compatibility wrapper.
- [x] Remove app-local render-camera conversion helpers that are no longer
      needed.
- [x] Add debug assertions or fallible constructors for non-finite camera bases.
- [x] Document the render-pose contract in `docs/native-engine-architecture.md`
      if the public shared boundary changed materially.

Result: `EngineRenderCamera` was removed outright. Remaining diagnostic callers
that still need fixed `ChunkCamera` values use
`legacy_chunk_camera_from_snapshot(...)` in `mclone-render-session`, so no app
crate owns an `EngineRenderCamera -> ChunkCamera` bridge. `ChunkCamera` and sky
view-projection construction now debug-assert finite render-view output, while
`PerspectiveRenderPose::render_view(...)` remains the fallible path for
player-controlled flat rendering.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-render-session -p mclone-native-client`
- `pnpm native:web:build`
- `pnpm native:desktop-offscreen:smoke`

## Acceptance Criteria

- Looking exactly straight down in desktop flat does not blank terrain.
- Looking exactly straight up in desktop flat does not blank terrain or sky.
- Third-person-back at vertical pitch keeps a finite render matrix and a sane
  camera offset.
- Normal gameplay/player pitch semantics remain vanilla-compatible.
- Web and flat Android use the same shared render-pose conversion as desktop.
- XR rendering remains unchanged except for any explicitly shared helper.
- `ChunkRenderView` has one clear invariant: its matrix and camera basis are
  finite before any render pass consumes it.

## Non-Goals

- Do not allow normal player gameplay pitch past vanilla `[-90, 90]` as part of
  this tactical.
- Do not convert server/player protocol rotation to quaternions.
- Do not introduce roll into normal Minecraft player controls unless a separate
  debug/free-camera feature explicitly asks for it.
- Do not refactor terrain culling, render compile scheduling, or XR multiview
  while doing the flat render-pose swap.

## Follow-Up Option: Free-Camera Over-Rotation

After the render-pose boundary is stable, a separate free-camera/debug mode can
support pitch beyond 90 degrees or full roll. That should be a camera-mode
feature with explicit input semantics:

- screen-up behavior across poles,
- yaw behavior while inverted,
- no-clip movement behavior,
- whether roll is exposed or implicitly stabilized.

That is intentionally separate from the vanilla player camera.
