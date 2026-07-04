# 138: Flat Render Camera Pose

Status: proposed; Slice A renderer-side pose path landed 2026-07-04
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

- [ ] Replace desktop flat `EngineRenderCamera -> ChunkCamera` conversion with
      direct shared render-pose/view construction.
- [ ] Keep `SpectatorCamera` as a compatibility shell if needed, but do not let
      it own new render math.
- [ ] Verify first-person and third-person-back view modes.
- [ ] Capture and inspect a desktop/offscreen frame looking straight down.
- [ ] Capture and inspect a desktop/offscreen frame looking straight up.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- the current desktop/offscreen smoke from `docs/platforms.md`
- screenshots saved under `/tmp`, not in the repo

### Slice C: Route Web And Flat Android Through The Same Shared Path

- [ ] Update `native/apps/mclone-web-client/src/web_canvas.rs` to use the shared
      render-pose/view conversion.
- [ ] Update `native/apps/mclone-android-client/src/lib.rs` to use the shared
      render-pose/view conversion.
- [ ] Remove or shrink duplicated `chunk_camera_from_engine(...)` helpers where
      they only bridge the old target/up shape.
- [ ] Preserve existing web and flat Android input semantics.

Validation:

- `pnpm native:web:build`
- web smoke/canvas validation from `docs/native-web.md` or
  `docs/platforms.md`
- flat Android build/smoke only if this slice touches Android presentation
  enough to warrant device or emulator time

### Slice D: Sky And Secondary Render Paths

- [ ] Replace `ChunkRenderView::sky_view_projection()` internals so it does not
      call `look_at_rh` with `camera_forward` and `camera_up`.
- [ ] Audit entity, selection-outline, far-LOD, world-GUI, and screen-effect
      paths for assumptions that `camera_up` is world-up.
- [ ] Keep XR multiview behavior unchanged.
- [ ] Add tests that sky view-projection is finite for vertical flat views and
      remains translation-free.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- screenshot comparison at ordinary pitch and vertical pitch

### Slice E: Cleanup And Contract Hardening

- [ ] Decide whether `EngineRenderCamera` should be replaced outright or kept as
      a deprecated compatibility wrapper.
- [ ] Remove app-local render-camera conversion helpers that are no longer
      needed.
- [ ] Add debug assertions or fallible constructors for non-finite camera bases.
- [ ] Document the render-pose contract in `docs/native-engine-architecture.md`
      if the public shared boundary changed materially.

Validation:

- `cargo test --manifest-path native/Cargo.toml`
- `pnpm native:web:build`
- relevant offscreen/web screenshot smokes from `docs/platforms.md`

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
