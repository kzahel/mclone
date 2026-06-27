# 102: Render Resource Rebuild Contract

Status: active. Shared `RenderConfig`, `FlatRenderResources`, desktop F8
no-op rebuild trigger, headless before/after no-op rebuild smoke, and
render-scale config-changing rebuild smoke landed on 2026-06-27; broader
platform adoption remains pending.

## Purpose

Create a shared renderer rebuild contract so runtime render-setting changes can
recreate GPU resources without restarting the app, tearing down gameplay state,
or pushing renderer policy into platform crates.

This follows from `100-render-color-profiles.md`: `Vanilla` and
`StylizedBright` can mostly remain dynamic frame settings, but future renderer
profiles such as `LinearExperimental`, HDR, render scale, MSAA, or alternate
intermediate targets need a safe way to rebuild pipelines and attachments while
the client/server/session/UI state survives.

The long-term goal is not just convenience. A working rebuild path enforces the
right architecture:

- CPU-side world, camera, UI, session, render-section cache, and asset state are
  durable.
- GPU-side textures, buffers, depth targets, pipelines, bind groups, and
  frame-target-sized attachments are disposable.
- Platform crates own presentation targets, not renderer policy.
- Device loss, Android surface recreation, web canvas resize, and XR swapchain
  changes have a common recovery shape.

## Setting Tiers

Render settings should be classified before implementation.

### Tier 1: Dynamic Frame Settings

No renderer rebuild. Values are passed through uniforms, draw options, or
per-frame render state.

Examples:

- `RenderColorProfile::Vanilla` vs `RenderColorProfile::StylizedBright`
- fullbright
- section occlusion
- fog enable/range/color
- sky darken

### Tier 2: Renderer-Resource Rebuild

Same `wgpu::Device`, `wgpu::Queue`, platform surface/session, and gameplay
runtime. Rebuild renderer-owned resources that depend on format, sample count,
target size, render scale, or shader permutations.

Examples:

- HDR or linear intermediate target
- MSAA sample count
- render scale
- post-processing chain enablement
- depth format change
- swapchain color format change when the platform surface itself can stay alive
- future PBR pipeline selection

### Tier 3: Device, Surface, Or Session Rebuild

Platform-owned recreation. The platform may need to recreate a `wgpu::Surface`,
Android `ANativeWindow` binding, browser canvas configuration, OpenXR
swapchains, or even the device.

Examples:

- device loss
- graphics backend/device change
- Android surface destruction/recreation
- OpenXR eye swapchain format or size change
- OS/display HDR mode changes that force presentation re-negotiation

Tier 3 should still feed back into the same shared renderer-resource rebuild
path after the platform has a valid device/queue/target config again.

## Ownership Boundary

Shared renderer/runtime crates own:

- `RenderConfig` or equivalent shared renderer configuration.
- Compatibility and validation rules for renderer-owned resources.
- Constructors/rebuilders for terrain, sky, actors, GUI, screen effects, world
  GUI, frame-sized targets, and future post-processing resources.
- Rebuild reports and diagnostics.
- Tests that prove CPU-side render/session state survives resource rebuilds.

Platform/app crates own:

- Window, canvas, Android surface, and OpenXR swapchain lifecycle.
- Presentation format and size negotiation.
- Device-loss and surface-loss event handling.
- Calling the shared rebuild entrypoint with a new target/config.
- UI/input glue that requests a config change.

Do not put gameplay, asset, lighting, mesh-cache, or renderer policy into an app
crate just because desktop is the first lane to exercise the rebuild path.

## Target Shape

Initial shared config should stay small and grow only when a setting actually
needs it:

```rust
pub struct RenderConfig {
    pub color_profile: RenderColorProfile,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub sample_count: u32,
    pub render_scale: f32,
    pub hdr: bool,
}
```

Likely companion state:

```rust
pub struct RenderTargetConfig {
    pub size: [u32; 2],
    pub surface_format: wgpu::TextureFormat,
    pub present_mode_label: &'static str,
}

pub struct RenderRebuildReport {
    pub previous: RenderConfig,
    pub next: RenderConfig,
    pub rebuilt_pipelines: usize,
    pub rebuilt_textures: usize,
    pub preserved_sections: usize,
}
```

The exact names can change during implementation. The important contract is:

1. Platform code produces a target/config from the current surface/session.
2. Shared renderer code rebuilds GPU resources from `Device`, `Queue`, assets,
   retained CPU-side render cache, and `RenderConfig`.
3. Gameplay/runtime/client/UI state is not recreated for a renderer-only
   rebuild.

## Resource Preservation Rules

Preserve across Tier 2 rebuilds:

- active session/coordinator state
- integrated or remote runtime state
- camera/controller/interpolation state
- UI screen/input state
- CPU-side asset catalogs and decoded texture sources
- client actor presentations
- render-section cache keys and latest CPU mesh data where available
- pending render compile/rebuild scheduling state where it is CPU-owned

Recreate during Tier 2 rebuilds:

- render pipelines
- bind group layouts and bind groups tied to rebuilt layouts
- depth targets and frame-sized intermediate textures
- GPU atlas textures and samplers when format/sampler policy changes
- GPU terrain/actor buffers when the owner cannot safely keep them
- screen-effect/world-GUI/overlay resources tied to color/depth formats

If a resource is expensive to recreate, keep a CPU-side source of truth first.
Do not make the only valid copy of a gameplay-visible asset live in GPU memory.

## Implementation Slices

### Slice 1: Ownership Inventory

- [ ] List renderer-owned GPU resources in desktop flat, web, flat Android,
  desktop OpenXR, and Android XR.
- [ ] Classify each resource as frame-sized, format-dependent,
  sample-count-dependent, asset-dependent, or dynamic.
- [ ] Identify any resource that currently lacks a CPU-side source of truth.
- [ ] Record any platform-only ownership that must remain outside the shared
  rebuild path.

Desktop flat inventory recorded on 2026-06-27:

- Platform-owned and preserved for Tier 2: `NativeSurfaceContext` owns
  `wgpu::Surface`, `wgpu::Device`, `wgpu::Queue`, `wgpu::SurfaceConfiguration`,
  present-mode support, and the current `RenderConfig`.
- Shared renderer-owned and disposable for Tier 2: `FlatRenderResources` owns
  the depth target, sky renderer, terrain draw resources, actor draw resources,
  screen effects renderer, and flat GUI renderer.
- Frame-sized: `ChunkDepthTarget`.
- Format-dependent: sky, terrain, actor, screen-effect, and GUI pipelines.
- Asset-dependent GPU uploads: terrain atlas, actor atlas, underwater overlay
  texture, and section vertex/index buffers.
- CPU sources of truth retained outside the bundle: decoded mesh/actor assets,
  asset source chain, runtime render-section cache, actor presentations, camera,
  session/coordinator, UI state, and input state.
- Platform-only ownership still outside the shared rebuild path:
  window/surface lifecycle, surface resize/configure, present mode, and audio.

### Slice 2: Shared Config Types

- [x] Add `RenderConfig` and small helper types in `mclone-render` or a shared
  render-session crate if cross-crate ownership requires it.
- [x] Convert existing color-profile target-format policy to use the shared
  config.
- [x] Add stable labels/diagnostics for config fields.
- [x] Add tests for defaults, equality/no-op detection, and invalid config
  rejection.

Slice 2 result:

- `native/crates/mclone-render/src/color_profile.rs` owns `RenderConfig`,
  default render target constants, `RenderConfigError`, validation, diagnostic
  labels, and profile-aware surface-format preference.
- Existing terrain/sky presentation transforms now derive from `RenderConfig`
  rather than loose profile/format pairs.
- Desktop surface setup records a `RenderConfig` beside the platform
  `SurfaceConfiguration`; web and flat Android use the same config-owned
  surface-format policy.
- No renderer resources are rebuilt at runtime yet.

Slice 2 validation on 2026-06-27:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-client`

### Slice 3: Desktop Flat Resource Bundle

- [x] Group desktop flat renderer resources behind a rebuildable bundle:
  depth target, sky, terrain draw resources, actors, GUI, screen effects, and
  any frame-sized attachments.
- [x] Add `rebuild_render_resources(...)` that preserves scene/runtime/camera/UI
  state and recreates only GPU resources.
- [x] Keep `NativeSurfaceContext` responsible for `wgpu::Surface` and
  `SurfaceConfiguration`; it should pass `RenderConfig` into the shared rebuild
  path instead of owning renderer policy.
- [x] Add a developer-only trigger and CLI/test path that performs a no-op
  rebuild while the world stays active.
- [x] Add a render-scale config-changing rebuild path while the world stays
  active in the headless desktop validation lane.

Slice 3 partial result:

- `native/crates/mclone-app-runtime/src/frame_render.rs` now owns shared
  `FlatRenderResources`, which groups depth, sky, terrain, actor,
  screen-effect, and GUI resources behind one config-validated constructor.
- The bundle currently accepts only the renderer shape we actually support:
  `Depth24Plus`, sample count 1, render scale `0.25..=2.0`, and non-HDR
  targets. Non-1.0 render scale renders into a renderer-owned color/depth target
  and presents it back to the platform target; MSAA, HDR, and alternate depth
  formats fail early instead of being silently ignored.
- `native/apps/mclone-native-client/src/app.rs` now stores one
  `Option<FlatRenderResources>` instead of separate depth/sky/draw/actor/effect
  GUI options.
- `ChunkApp::rebuild_render_resources(...)` recreates the bundle from the
  current `NativeSurfaceContext` and reuploads CPU-owned runtime sections when
  a world is active. It is currently used for initial construction and the
  developer no-op trigger; config-changing runtime rebuild remains pending.
- Desktop flat has a hidden developer F8 trigger that rebuilds the current
  renderer resources in place and schedules a redraw.
- `--renderer-rebuild-smoke <directory>` runs a deterministic offscreen no-op
  rebuild smoke: render frame 0, rebuild/reupload on frame 1, render again,
  save `before.png`/`after.png`, compare pixels exactly, and assert stable
  runtime/camera/UI state across the rebuild.
- `--renderer-rebuild-smoke <directory> --rebuild-render-scale <scale>` rebuilds
  the same desktop flat resource bundle with a different internal render size,
  keeps the gameplay/session/UI state stable, and allows the expected output
  pixel differences from resampling.

Slice 3 partial validation on 2026-06-27:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-client`

Slice 3 no-op rebuild validation on 2026-06-27:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-client`
- `cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --renderer-rebuild-smoke /tmp/mclone-render-rebuild-smoke --width 320 --height 180 --render-distance 2 --day-time 6000 --freeze-time --section-occlusion true`
  produced exact pixel match (`mismatch_pixels=0`), reuploaded 166 sections,
  preserved state, reported `config_changed=false`, and kept
  `before_render_size=[320, 180] after_render_size=[320, 180]`.

Slice 3 render-scale rebuild validation on 2026-06-27:

- `cargo test --manifest-path native/Cargo.toml`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-client`
- `cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --renderer-rebuild-smoke /tmp/mclone-render-rebuild-scale-smoke --width 320 --height 180 --render-distance 2 --day-time 6000 --freeze-time --section-occlusion true --rebuild-render-scale 0.5`
  produced `config_changed=true`, `before_render_size=[320, 180]`,
  `after_render_size=[160, 90]`, `mismatch_pixels=40922`, reuploaded 166
  sections, and preserved state. `/tmp/mclone-render-rebuild-scale-smoke/after.png`
  was visually inspected and showed the expected lower-resolution but correctly
  framed world render.

### Slice 4: Rebuild-Safe Dynamic Profile Toggle

- [ ] Make `Vanilla`/`StylizedBright` remain Tier 1 dynamic settings where
  practical.
- [ ] Add UI/action plumbing for session-local profile changes only after the
  shared config exists.
- [ ] Ensure changing the dynamic profile does not rebuild the renderer unless a
  future profile explicitly requires it.
- [ ] Keep `LinearExperimental` able to request Tier 2 rebuild when implemented.

### Slice 5: Validation Harness

- [x] Add a no-op rebuild test that renders before/after images and verifies no
  meaningful pixel drift.
- [x] Add a render-scale config-changing rebuild smoke that proves camera
  position, selected session, UI state, and resident sections survive.
- [x] Save validation captures to `/tmp`, not the repo.
- [ ] Expose rebuild counts/timing in debug or perf diagnostics.

### Slice 6: Web, Android, And XR Adoption

- [ ] Adapt web canvas resources to the shared bundle after desktop flat proves
  the shape.
- [ ] Adapt flat Android surface recreation to call the same shared rebuild path.
- [ ] Adapt desktop OpenXR and Android XR eye resources without pretending their
  swapchain ownership is identical to flat surfaces.
- [ ] Keep platform-specific target negotiation thin and explicit.

### Slice 7: Device/Surface Loss Path

- [ ] Route desktop surface loss/outdated handling through the same rebuild
  vocabulary.
- [ ] Add Android surface recreation coverage.
- [ ] Add web resize/configuration coverage.
- [ ] Add OpenXR swapchain recreation notes or smoke coverage where practical.

## Validation

Minimum first-lane validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- desktop headless or offscreen before/after no-op rebuild capture comparison
- live desktop smoke: rebuild while in-world and confirm camera/session/UI state
  survives

Cross-lane validation as adoption expands:

- `pnpm native:web:build`
- `pnpm native:web:smoke`
- flat Android build/smoke path
- desktop OpenXR mclone smoke where available
- Android XR build/smoke path where available

## Non-Goals

- Do not implement full PBR/HDR in this tactical. This creates the rebuild
  contract that later PBR/HDR work can use.
- Do not make every setting require renderer rebuild. Keep Tier 1 settings
  dynamic.
- Do not move platform surface or OpenXR swapchain ownership into shared
  renderer code.
- Do not reset the client/server/session just to apply renderer config.
- Do not build desktop-only renderer policy and then retrofit other platforms
  around it.

## Acceptance

- A shared `RenderConfig`-style contract exists and is used by desktop flat.
- Desktop flat can rebuild renderer-owned GPU resources at runtime without
  recreating gameplay/session/UI state.
- No-op rebuilds preserve rendered output within a documented tolerance.
- Config-changing rebuilds preserve camera/session/UI state and resident render
  data where CPU ownership exists.
- Platform code still owns surfaces/swapchains, but renderer policy and resource
  construction live behind shared contracts.
- The path is ready for future `LinearExperimental`, HDR, PBR, render-scale, or
  MSAA work without requiring an app restart as the only safe option.
