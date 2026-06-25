# 079: Desktop OpenXR Mclone Frame

Status: proposed, blocked on `077-multiview-render-contract.md` and
`078-desktop-openxr-clear-smoke.md`.

## Purpose

Render a real mclone runtime frame through the desktop OpenXR path.

This is the first slice where XR should consume the client/server/render-session
stack. The goal is not polished VR gameplay. The goal is to prove that the
shared runtime, renderer resources, render-section streaming, and per-eye
view/target contract work together without forking the engine.

## Target Shape

The XR host owns:

- OpenXR frame wait/begin/end.
- Per-eye swapchain image acquisition and release.
- Per-eye depth target ownership.
- Runtime-provided view/projection conversion into `ChunkRenderView`.
- Optional mirror/diagnostic output if cheap.
- XR-specific startup and validation mode.

Shared code owns:

- `SingleViewRuntime` or its successor for client/server/render-session state.
- Render-section compile and upload policy.
- Asset loading through existing packed/loose source helpers.
- Full-frame world composition from explicit view/target facts.

This slice may still run from a fixed headset-relative pose and static
controller-free interaction. Movement, hand/controller input, in-world UI, and
comfort features should be separate follow-ups.

## Implementation Slices

### Slice 1 - Runtime Reuse In XR Host

- [ ] Build the same integrated-runtime scene used by desktop and flat Android
  smokes.
- [ ] Load the standard packed/loose Minecraft asset source through shared
  render asset helpers.
- [ ] Poll until the initial chunk set is ready.
- [ ] Compile and upload render sections before the first XR world frame.
- [ ] Keep the runtime/asset setup independent of OpenXR types.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
```

### Slice 2 - Per-Eye World Rendering

- [ ] Convert OpenXR view/projection facts into `ChunkRenderView` values.
- [ ] Render sky and terrain once per eye through the shared full-frame helper.
- [ ] Use one host-owned depth target per eye.
- [ ] Reuse one render-section cache and texture atlas for both eyes.
- [ ] Ensure frustum/culling behavior is view-specific but cache ownership is
  frame/runtime-shared.

Suggested smoke:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- \
  --xr-mclone-smoke --frames 240 \
  --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --day-time 6000 --freeze-time
```

The exact CLI spelling can change, but it should be bounded and scriptable.

### Slice 3 - Diagnostics And Capture

- [ ] Log runtime name, backend, swapchain format, view count, target size, and
  frame count.
- [ ] Log render-section counts and drawn indices for eye 0 at least once.
- [ ] Add a mirror or readback screenshot only if it is cheap and reliable on
  the local desktop runtime.
- [ ] Record inspected output or headset-visible validation notes in this doc.

## Out Of Scope

- Android XR / Quest packaging.
- Controller actions and locomotion.
- Hand tracking.
- In-world UI or XR panels.
- Passthrough and spatial-room support.
- Performance tuning beyond basic non-stutter diagnostics.
- Multi-player/remote-player-specific XR presentation.

## Review Rejection Criteria

- Duplicating the desktop or Android runtime/render-session stack inside an XR
  module.
- OpenXR types inside shared runtime, client, server, protocol, mesh, asset,
  light, worldgen, or render-session crates.
- A separate XR renderer for chunks or sky.
- Per-eye render-section rebuilds when the section cache could be shared.
- Any Android XR package or Quest manifest changes in this desktop slice.

## Completion Criteria

- Desktop OpenXR can render real mclone terrain through shared renderer/runtime
  code.
- The XR path uses explicit per-eye `ChunkRenderView` and host-owned
  `RenderFrameTarget`/depth facts.
- The render-section cache, texture atlas, and asset helpers are shared with
  desktop/Android rather than forked.
- The validation path is bounded, documented, and runnable by command.
- Android XR can be planned as a packaging/runtime-host slice instead of an
  engine-boundary refactor.
