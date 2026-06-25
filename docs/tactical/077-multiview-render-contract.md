# 077: Multiview Render Contract

Status: Slices 1-2 complete; shared render tests remain.

## Purpose

Remove the remaining single-camera assumption from shared full-frame rendering
before adding OpenXR runtime code.

The target is not stereo rendering as a product feature yet. The target is a
renderer contract that a future XR host can drive by supplying one explicit
view/target pair per eye, while desktop, headless, web, and flat Android keep
using the same single-view path.

## Current State

Useful pieces already exist:

- `ChunkRenderView` carries view, projection, view-projection, camera position,
  camera basis, aspect, FOV, and near/far planes.
- `RenderFrameContext` and `RenderFrameTarget` describe host-acquired GPU frame
  facts without knowing about windows, canvases, Android surfaces, or offscreen
  readback.
- `ChunkRenderTarget::from_frame_target` adapts generic frame targets to the
  chunk renderer's stricter color/depth requirements.
- `ChunkDepthTarget` is host-owned and already works for surface and offscreen
  paths.

The remaining shared helper shape is still single-view:

- `render_full_frame(...)` takes a `ChunkCamera`.
- It derives one `ChunkRenderView` from `frame.target.size`.
- It renders sky, chunks, actors, underwater overlay, and GUI to one target.

## Target Shape

Add a small render-view contract that lets callers supply already-built view
facts:

- Keep `ChunkCamera` as a host/debug helper, not as the full-frame renderer's
  required input.
- Let the shared full-frame helper accept a `ChunkRenderView` for world passes.
- Keep target acquisition host-owned through `RenderFrameContext`.
- Keep depth target ownership host-owned, including one depth target per XR eye
  later.
- Keep GUI overlay semantics single-target for now; XR in-world UI or mirror UI
  can be a later explicit slice.

The first implementation can stay conservative:

- Preserve a wrapper that accepts `ChunkCamera` for existing single-view callers
  if that keeps the patch small.
- Introduce a narrow `FullFrameView` or equivalent only if it removes repeated
  argument plumbing.
- Do not introduce a full frame graph.
- Do not add OpenXR dependencies in this slice.

## Implementation Slices

### Slice 1 - Explicit View Input

- [x] Add a full-frame render entry point that takes `ChunkRenderView` directly.
- [x] Convert existing desktop/headless/flat-Android callers to build the
  render view at the app/host boundary.
- [x] Keep any `ChunkCamera` wrapper as a compatibility helper, not the primary
  shared render path.
- [x] Preserve existing render stats and GUI draw-list plumbing.

Recorded Slice 1 result:

- Added `mclone_app_runtime::frame_render::render_full_frame_for_view(...)` as
  the explicit-view full-frame entry point.
- Kept `render_full_frame(...)` as a thin `ChunkCamera` compatibility wrapper.
- Updated desktop window, desktop headless screenshot, frame-budget perf, and
  flat Android callers to derive `ChunkRenderView` at the host/app boundary
  before calling the shared renderer.
- No visual behavior was intended to change.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-render -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
pnpm native:desktop-chunk:smoke
git diff --check
```

Screenshot inspected: `/tmp/mclone-desktop-runtime-chunk.png` (`2560x1600`),
showing the expected nonblank terrain cutaway. The smoke reported `520116`
vertices and `780174` indices.

### Slice 2 - Dual-View Headless Capture

- [x] Add a deterministic headless validation path that renders the same
  runtime section set from two slightly offset views.
- [x] Save either two PNGs or one side-by-side PNG under `/tmp`.
- [x] Assert both views are nonblank and carry expected terrain pixels.
- [x] Keep the capture independent of OpenXR.

Recorded Slice 2 result:

- Added `--headless-dual-view DIR`, writing `left.png` and `right.png`.
- The mode builds one integrated runtime, polls it to idle, syncs one runtime
  section set, and renders two offset full-frame world views through
  `render_full_frame_for_view(...)`.
- The capture fails if either rendered image has no RGB pixels differing from
  the top-left clear/background color, so an all-clear or all-sky frame does
  not pass as terrain output.
- The output remains OpenXR-free and uses ordinary headless `wgpu` readback.

Suggested command shape:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --headless-dual-view /tmp/mclone-dual-view --width 960 --height 640 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
```

Validation output:

- `/tmp/mclone-dual-view/left.png`: `960x640`, `35971` non-clear RGB pixels,
  `166` drawn sections, `780174` drawn indices.
- `/tmp/mclone-dual-view/right.png`: `960x640`, `36011` non-clear RGB pixels,
  `166` drawn sections, `780174` drawn indices.

Both screenshots were inspected and showed the expected terrain cutaway with a
small left/right view offset.

### Slice 3 - Shared Render Tests

- [ ] Add focused tests for view-derived sky matrix behavior if the helper is
  changed.
- [ ] Add a small smoke/test that proves culling consumes `ChunkRenderView`,
  not a desktop window or camera helper.
- [ ] Record the final validation output paths in this doc.

## Review Rejection Criteria

- Any OpenXR, Android XR, DOM, `winit`, or `android-activity` dependency added
  to `mclone-app-runtime` or `mclone-render` for this slice.
- A second renderer path for dual-view output.
- A renderer API that reconstructs projection from desktop FOV/window facts
  after the host already supplied matrices.
- A hidden global swapchain/depth target inside draw resources.
- Screenshot output written into the repo or `test-results/`.

## Completion Criteria

- Shared full-frame rendering can be driven from an explicit
  `ChunkRenderView`.
- Desktop, headless, flat Android, and native web builds remain green.
- A dual-view headless validation path produces inspected nonblank output under
  `/tmp`.
- The next desktop OpenXR smoke can feed per-eye views/targets without
  refactoring frame composition again.
