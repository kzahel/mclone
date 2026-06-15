# 022: Platform Target Contract And Render Boundary

Status: active near-term defensive slice. The renderer target and host frame boundary passes are landed; keep follow-up work small and validation-backed.

## Purpose

Lock in the platform posture before more renderer and app code grows around the desktop window path.

Native desktop remains the first-priority bring-up target. The goal of this slice is not to build Android or OpenXR now. The goal is to make the current desktop/headless renderer boundary explicit enough that future web, flat Android, and Android XR / Quest hosts can drive the same renderer without undoing desktop assumptions.

## Target Posture

- Desktop native is the active development loop and validation surface.
- Web/WASM stays compiling and smoke-tested at subsystem boundaries.
- Flat Android is a future single-view native host and packaging baseline.
- Android XR / Quest standalone is a future native XR host once desktop runtime/rendering is mature.
- The TypeScript/browser implementation is legacy/reference prior art only.

## Boundary Rules

- Shared engine crates must not depend on `winit`, Android activity glue, OpenXR sessions, DOM/canvas objects, or platform filesystem details.
- `mclone-render` may own `wgpu` resources, but renderer-facing APIs should accept explicit view/projection and render-target facts rather than reconstructing them from a desktop camera/window.
- App crates own platform lifecycle, input, surface/swapchain acquisition, and presentation pacing.
- Headless/offscreen rendering remains a first-class validation path.
- Client/server/protocol/mesh/asset contracts stay chunk/section and plain-data oriented; no GPU handles cross those boundaries.
- Android/OpenXR scaffolding should wait for a real validation lane instead of being added as inert structure.

## First Implementation Slice

Add small renderer data types and thread them through the existing chunk render path:

1. Introduce a render-view input type for the chunk renderer, carrying at minimum a `view_projection` matrix. It can start narrow and grow later if lighting, fog, water, or XR need camera position, projection center, or per-view metadata.
2. Introduce a render-target input type carrying the color view, size, clear color, and depth ownership policy needed by the current chunk passes.
3. Keep `ChunkCamera` as a desktop/headless camera helper, but make it produce render-view data instead of being the renderer's required input.
4. Update frustum culling to consume the explicit view-projection matrix, not a desktop camera plus target dimensions.
5. Update native window and headless callers to build the render-view/target values at the app boundary.
6. Preserve existing screenshots and stats; this should be a boundary refactor, not a visual change.

## Landed Boundary Shape

- `ChunkRenderView` is the renderer-facing camera input. It carries the view matrix, projection matrix, combined view-projection matrix, camera position, camera basis, aspect, FOV, and near/far planes. Desktop/headless `ChunkCamera` remains a helper that produces this data, not a required renderer dependency.
- `ChunkRenderTarget` is the renderer-facing attachment input. It carries explicit color and depth texture views, size, clear color, and clear depth.
- `ChunkDepthTarget` is owned by host paths, not by reusable draw resources. The native window path owns it beside `NativeSurfaceContext`; headless paths own it beside their offscreen color target.
- Chunk draw resources own reusable GPU draw state only: pipelines, uploaded meshes/sections, and texture atlas bindings. They do not allocate or resize window-shaped attachments.
- This is still a single-view renderer. XR should later feed one `ChunkRenderView`/`ChunkRenderTarget` pair per eye instead of asking the renderer to understand OpenXR sessions.

## Host Frame Contract

- `RenderFrameTarget` is the generic host-acquired target description. It carries a color view, optional depth view, and size. It does not know about `winit`, Android surfaces, OpenXR swapchains, browser canvases, or headless readback.
- `RenderFrameContext` carries the `wgpu` device, queue, command encoder, and `RenderFrameTarget` for a single frame encode.
- `NativeSurfaceContext::render_with` now passes a `RenderFrameContext` instead of a desktop-shaped tuple. Headless offscreen rendering builds the same context from `OffscreenTarget`.
- `ChunkRenderTarget::from_frame_target` is the chunk renderer's stricter adapter: it requires a depth attachment and adds chunk-specific clear state.
- A future flat Android host should acquire one color view, own/resize one depth target, build one `RenderFrameContext`, and drive the same chunk render call.
- A future XR host should acquire one color view per eye from its XR swapchain, pair each with the matching depth target, and feed one `ChunkRenderView` plus one `RenderFrameTarget` per eye. OpenXR session/swapchain ownership stays outside shared renderer code.

## Current Boundary Checks

Feature work should preserve these checks:

- A render call must be driveable from both a swapchain color view and an offscreen color view.
- Surface/offscreen acquisition should expose `RenderFrameContext`; feature code should not grow new callback tuples of raw target parameters.
- Depth attachment size/lifetime belongs to the host target path. Adding another host target must not require changing draw-resource constructors.
- Culling consumes `ChunkRenderView`, not a desktop window size or desktop camera.
- New camera-dependent render features should add facts to `ChunkRenderView` instead of reaching back into `ChunkCamera`.
- New attachment-dependent render features should add facts to `ChunkRenderTarget` or a sibling target description instead of allocating hidden per-pass targets inside draw resources.

## Out Of Scope

- Android Gradle projects or `cargo-ndk` scripts.
- Android `NativeActivity` or Quest manifests.
- OpenXR loader/session/swapchain code.
- Stereo rendering.
- WebXR.
- Renderer feature expansion beyond the minimum types needed to remove desktop-camera coupling.

## Playbox References

Use `~/code/playbox` as a pattern library only:

- `src/render/mod.rs`: `RenderViewInput` carries explicit view/projection facts.
- `src/render/targets.rs`: `SceneTarget` carries explicit render-target facts.
- `src/core.rs`: single-view desktop/headless/flat-Android runtime wrapper.
- `src/xr/mod.rs`: XR host drives the world/renderer directly with per-eye views and targets instead of going through a single desktop camera wrapper.

Do not copy Playbox's PhysX, VaM, egui, or monolithic runtime shape.

## Validation

Required for the first implementation slice:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-platform-host-contract-chunk.png --width 960 --height 640 --chunk-radius 1
git diff --check
```

Inspect `/tmp/mclone-platform-host-contract-chunk.png`. It should match the pre-refactor framing closely enough that any difference is explainable by intentional camera math cleanup, not by blank output, clipped terrain, or changed draw order.

## Follow-Up Trigger

Only after this boundary is in place should we consider a platform smoke tactical:

- flat Android clear/chunk smoke if the next risk is Android lifecycle and Vulkan-backed `wgpu`
- desktop OpenXR stereo clear/chunk smoke if the next risk is per-eye render targets and runtime swapchain wrapping
- Android XR / Quest loader/session smoke if there is an attached-device validation loop available
