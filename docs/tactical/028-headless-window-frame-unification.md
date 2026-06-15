# 028: Headless Window Frame Unification

Status: active.

## Purpose

Make headless/offscreen rendering a first-class way to exercise the same native
client frame capabilities as the desktop window path.

The current native client has useful low-level target boundaries, but the
high-level modes still fork too early: windowed rendering composes world, UI, and
debug overlay in the `winit` redraw path, while headless helpers mostly render
world-only chunk captures or UI-only static screens. That makes every new HUD,
debug pane, menu, and capture feature risk drifting between modes.

## Target Shape

Keep platform differences in host adapters:

- desktop host owns `winit`, keyboard/mouse input, cursor lock, swapchain
  acquire/present, resize, focus, and real-time pacing
- headless host owns offscreen targets, deterministic stepping, scripted camera
  and UI state, readback, screenshot writing, and later MP4 encoding
- shared single-view client code owns runtime polling, chunk interest, camera
  facts, render-section uploads, UI state, debug visibility, frame stats, and
  full-frame composition

The shared frame path should be able to render:

- world only
- world plus debug pane
- world plus game UI/HUD
- UI screens that cover the world
- the same renderer options and diagnostics exposed in windowed mode

## Playbox Reference

Use Playbox as a pattern library, not as a dependency:

- `~/code/playbox/src/core.rs`: `SingleViewRuntime` owns one-camera world,
  renderer, camera helpers, and render submission for desktop, Android,
  headless screenshots, recording, and debug scenarios.
- `~/code/playbox/src/headless.rs`: screenshots construct the same single-view
  runtime with an offscreen target; `include_ui` renders the UI over the same
  target before readback.
- `~/code/playbox/src/app.rs`: desktop gathers real input/UI commands, advances
  simulation, renders the same single-view runtime to the swapchain, then renders
  UI.
- `~/code/playbox/docs/architecture/runtime-loop.md`: documents the shared
  single-view runtime and the host-specific desktop/headless responsibilities.

Do not copy Playbox's VaM, PhysX, egui, or monolithic runtime shape. Mclone's UI
path is first-party `mclone-ui`, and the renderer/client/server contracts must
stay native-web and future XR compatible.

## First Implementation Slice

1. Add a generic headless frame helper in `mclone-render` that creates a headless
   device/offscreen target, exposes a `RenderFrameContext`, submits the encoded
   frame, reads pixels, and writes a PNG.
2. Extract the native client's world/UI/debug composition into a shared
   full-frame render function that accepts any `RenderFrameContext`.
3. Add a full-frame headless screenshot mode that can select UI screen state and
   debug pane visibility while using the same runtime, camera, renderer options,
   GUI renderer, and debug-pane draw path as windowed mode.
4. Keep existing `--headless-chunk`, `--headless-chunk-scenarios`, and
   `--headless-ui` commands as compatibility validation lanes until the full-frame
   screenshot mode replaces them in later tacticals.

## Follow-Up Slices

- Rename remaining `Window*Runtime` symbols to single-view/client-runtime names
  once the shared frame path is stable.
- Move more native app state out of `main.rs` into host-neutral native client
  modules before adding broader UI/HUD behavior.
- Add deterministic scripted input for headless UI interaction and movement
  captures.
- Add `--record <path.mp4>` that loops deterministic ticks, renders the same
  full-frame path, reads RGBA, and pipes frames to ffmpeg.
- Update native validation docs so pixel-producing slices prefer full-frame
  headless screenshots when validating UI/debug behavior.

## Validation

Required for the first slice:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-full-frame-debug.png --width 960 --height 540 --screenshot-ui none --screenshot-debug-pane true
git diff --check
```

Inspect `/tmp/mclone-full-frame-debug.png`. It must be nonblank, show terrain,
and include the same debug pane styling/content shape as the native window path.

