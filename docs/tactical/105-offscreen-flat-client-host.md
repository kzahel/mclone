# 105: Offscreen Flat Client Host

Status: active; desktop and headless full-frame UI assembly now route through
the native flat driver.

## Purpose

Turn headless/offscreen from a screenshot helper into a real flat client host.

The target is a no-window client lifetime that can run in a headless
environment, consume neutral or synthetic input, render full flat-client frames
through the same path as desktop flat, and hand frames to PNG readback, video
encoding, network streaming, visual tests, or future AI/model consumers.

This tactical supersedes the remaining direction in
[`028-headless-window-frame-unification.md`](028-headless-window-frame-unification.md).
Tactical 028 landed useful low-level and full-frame pieces; this tactical owns
the broader platform target and the cleanup that removes screenshot-shaped
divergence.

Durable target contract:
[`../offscreen-flat-client.md`](../offscreen-flat-client.md).

## Current State

Good pieces already exist:

- `mclone-render::headless` creates a native offscreen `wgpu` device/texture,
  exposes `RenderFrameContext`, submits work, reads RGBA pixels, and writes
  PNGs.
- `mclone-app-runtime::frame_render` owns the shared full-frame flat renderer.
- `FlatRenderResources` already bundles the mature flat render resource set and
  is used by desktop flat and the renderer rebuild smoke.
- `WindowSceneRuntime` wraps `NativeSingleViewSceneRuntime<RemoteServerSession>`,
  so local/remote host mode, polling, render-section sync, cached sections, and
  sky/time facts are mostly shared.

The current gaps:

- `ChunkApp` still owns much of the real flat client lifetime: input semantics,
  camera updates, UI/session actions, interaction dispatch, actor interpolation,
  render-resource lifetime, section upload, debug/HUD construction, and frame
  composition setup.
- `run_headless_screenshot` now constructs a `FlatClientDriver`, gives it the
  screenshot runtime/camera, rebuilds `FlatRenderResources` through the driver,
  uploads sections through the driver, and renders through
  `FlatClientDriver::render_full_frame`.
- `mclone-native-client::flat_client_driver` now exists as a native staging
  owner for flat-client runtime, camera/spectator, interaction, actor
  interpolation, render options, render stats, frame timing, and full-frame UI
  draw-list assembly. `ChunkApp` still drives it directly from the desktop event
  loop and still owns the `GameUi` instance and session/startup policy.
- `run_headless_screenshot` is still screenshot-shaped: it constructs a fresh
  runtime/driver per capture and still owns UI scenario setup and
  scripted interaction instead of sharing a long-lived flat-client host loop.
- Older `--headless-chunk`, `--headless-chunk-scenarios`, and `--headless-ui`
  modes validate narrower renderer surfaces and can drift from real flat-client
  behavior.

## Target Shape

Add a host-neutral flat client driver:

```text
Desktop winit events      Scripted/network/model events
        |                           |
        v                           v
  desktop host adapter       offscreen host adapter
        |                           |
        +------ FlatClientDriver ---+
                    |
                    v
             explicit frame target
```

The driver should own real client state:

- session coordinator and runtime replacement
- camera controller and neutral flat input application
- UI state and host-neutral menu action handling
- client interaction controller, carried item sync, block target, selection
  outline source
- actor interpolation and local presentation state
- render resources, section upload, traversal-ready facts, render stats, and
  frame diagnostics
- full-frame composition inputs for `mclone-app-runtime::frame_render`

Desktop host keeps only true desktop concerns:

- `winit` event loop, window, focus, resize, cursor lock, raw mouse motion
- native surface acquire/present/reconfigure
- frame pacing and monitor/present-mode policy
- desktop CLI wiring and desktop-only diagnostics

Offscreen host keeps only true offscreen concerns:

- offscreen device/target creation, resize, and readback
- scripted/network/model input source
- deterministic or externally driven frame clock
- frame sink selection: PNG, RGBA buffer, future video encoder, future network
  stream

## Implementation Slices

### Slice 0 - Screenshot Render Resource Cleanup

- [x] Replace private screenshot construction of depth, chunk draw resources,
  sky, actors, screen effects, selection outline renderer, and GUI renderer
  with `FlatRenderResources`.
- [x] Upload cached screenshot sections with
  `FlatRenderResources::draw_mut().update_sections(...)`.
- [x] Compose screenshots through `FlatRenderResources::render_full_frame(...)`
  so outline, actors, underwater overlay, sky, GUI, and render-scale presentation
  follow the shared full-frame renderer bundle.
- [x] Preserve existing screenshot scenario inputs: seed, fixed time, camera
  pose, UI screen, debug pane flag, and scripted interaction.

Validation run:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-render -p mclone-native-client`
- `cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-offscreen-flat-debug.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-debug-pane true`
- Visual inspection of `/tmp/mclone-offscreen-flat-debug.png`: nonblank terrain,
  debug pane, view swatch, and cow actor rendered.

### Slice 1 - Flat Client Driver Skeleton

- [x] Create a native-client staging `FlatClientDriver` module. This intentionally
  lands in `mclone-native-client` first because current dependencies still
  include native actor texture assets, `WindowSceneRuntime`, and desktop startup
  glue.
- [x] Move the first non-`winit` state cluster out of `ChunkApp`: runtime,
  camera/spectator mirror, interaction controller, actor interpolation, render
  options, render stats, and frame timing.
- [x] Move first behavior methods behind the driver: held/analog input
  application, look input, camera speed adjustment, pose sync, carried-item
  sync, block interaction, effective render options, underwater overlay, actor
  interpolation, and selection outline.
- [x] Move runtime polling, section sync, section upload bookkeeping, render-stat
  updates, traversal-ready lookup, and flat frame-input preparation behind
  driver APIs while leaving GPU device/resource ownership in the desktop app.
- [x] Move `FlatRenderResources` ownership, render-resource rebuild/resize,
  section GPU upload calls, and full-frame render dispatch into the driver.
  Desktop still owns the surface and passes frame targets/devices into the
  driver.
- [x] Move full-frame UI draw-list assembly into the driver: base `GameUi`
  rendering, debug pane, loading/progress overlays, HUD/status overlay, and
  readiness overlay now share `FlatClientDriver::render_full_frame_with_ui(...)`
  for desktop and headless screenshot.
- [ ] Move session coordinator/startup, UI state, input preferences, render
  resource asset loading, and menu action routing behind driver methods instead
  of direct `ChunkApp` field access.
- [ ] Keep platform transport/session construction injectable so desktop TCP,
  offscreen TCP, Android property TCP, and future network sources stay adapters.
- [ ] Preserve existing desktop behavior.

Validation run:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

Follow-up validation run after behavior delegation:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

Follow-up validation run after section/frame prep delegation:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

Follow-up validation run after render-resource ownership moved into driver:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

### Slice 2 - Shared Step And Render API

- [ ] Add a small API shape such as `apply_input(...)`, `tick(...)`,
  `upload_runtime_sections(...)`, and `render_frame(RenderFrameContext)`.
- [ ] Move desktop redraw logic into the driver except surface acquire/present,
  frame pacing, and `winit` event translation.
- [x] Move HUD/debug/loading/status draw-list construction into the shared
  driver path.
- [ ] Keep target size/render config explicit; do not hide swapchain or
  offscreen texture ownership in the driver.

### Slice 3 - Rewire Headless Screenshot To The Driver

- [x] Replace screenshot-owned render-resource lifetime, section upload,
  actor/underwater/selection setup, and full-frame render dispatch with the
  shared `FlatClientDriver` path.
- [x] Keep screenshot-specific setup as scenario input: initial seed, fixed
  time, camera pose, requested UI screen, debug pane flag, and optional scripted
  input.
- [x] Remove duplicate selection, underwater, actor, and full-frame renderer
  setup from the screenshot path.
- [x] Validate that screenshots exercise the same full-frame path as desktop
  flat.
- [x] Move debug/HUD draw-list assembly behind shared driver methods.
- [ ] Move scenario UI setup behind shared driver methods.
- [ ] Replace screenshot-owned runtime construction with a long-lived offscreen
  flat-client host.

Validation run:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-offscreen-driver-debug.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-debug-pane true`
- Visual inspection of `/tmp/mclone-offscreen-driver-debug.png`: nonblank
  terrain, debug pane, view swatch, and cow actor rendered.

Follow-up validation run after UI draw-list assembly moved into the driver:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-offscreen-driver-ui-debug.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-debug-pane true`
- Visual inspection of `/tmp/mclone-offscreen-driver-ui-debug.png`: nonblank
  terrain, debug pane, view swatch, and cow actor rendered.

### Slice 4 - Neutral Scripted Input

- [ ] Replace direct screenshot-only gameplay command shortcuts with neutral
  input/UI events where possible.
- [ ] Add deterministic movement, look, pointer, menu, hotbar, attack, and use
  scenarios over `mclone-input`/UI events.
- [ ] Keep direct protocol command helpers only for low-level protocol/runtime
  tests, not for flat-client validation.

### Slice 5 - Retire Older Client Validation Modes

- [ ] Move `native:desktop-chunk:smoke` to a full-frame offscreen flat-client
  smoke once coverage is equivalent.
- [ ] Replace `--headless-ui` with full-frame screenshot scenarios such as
  `--screenshot --screenshot-ui title`.
- [ ] Replace `--headless-chunk` and `--headless-chunk-scenarios` with
  full-frame screenshot scenarios unless a specific renderer-level test still
  needs them.
- [ ] Leave narrow renderer helpers in `mclone-render::headless` only when they
  test renderer units, not client behavior.

### Slice 6 - Long-Lived Offscreen Host

- [ ] Add a long-lived offscreen client mode that can render N frames or run
  until externally stopped.
- [ ] Expose frame sinks cleanly: PNG sequence first, RGBA buffer/report shape
  next, video/network sinks later.
- [ ] Add a remote-dedicated offscreen smoke that connects to a real server,
  applies synthetic input, renders frames, and asserts nonblank client output.

## Validation

Per slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-render -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
git diff --check
```

When a slice produces pixels, save captures under `/tmp` and inspect them.
Representative target command once Slice 3 lands:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-offscreen-flat-debug.png \
  --width 1280 --height 720 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 \
  --day-time 6000 --freeze-time \
  --screenshot-debug-pane true
```

The screenshot must be nonblank, show terrain, and include the same HUD/debug/UI
composition that desktop flat would produce for the same state.

## Review Rejection Criteria

- A hidden `winit` window as the basis for offscreen validation.
- Screenshot code constructing a private renderer/resource graph after the
  shared driver exists.
- New gameplay, input, UI, interaction, runtime, or render policy added only to
  desktop `ChunkApp`.
- A frame sink that owns client state instead of consuming frames from the
  shared driver.
- OpenXR runtime emulation mixed into this flat-client host. Synthetic stereo
  scene validation can be a later XR tactical.
