# Offscreen Flat Client Host

This document defines the target shape for a real flat client host that does
not own a window, swapchain, or presentation surface.

The goal is not "better screenshots." The goal is a real client lifetime that
can run in a headless environment, consume neutral or synthetic input, render
full flat-client frames into offscreen targets, and hand those frames to a sink:
PNG, video encoder, network stream, automated visual check, or an AI/model
integration.

## Target Contract

```text
InputSource
  -> Offscreen/desktop flat client host
  -> shared flat client state
  -> explicit render target
  -> FrameSink
```

The same shared flat client state should be driven by desktop `winit`, scripted
headless tests, a future remote UI input stream, or an AI/controller loop. The
host should differ only in how it receives events, acquires render targets, and
delivers frames.

Shared flat client code owns:

- session/runtime lifetime: local world, remote dedicated join, pending/failed
  state, teardown, and replacement
- client replica and server command/update exchange through shared runtime
  contracts
- camera controller, input semantics, interaction controller, selected hotbar,
  and player-pose synchronization
- UI state, HUD/debug/status draw construction, and menu action routing
- actor interpolation and other client presentation state
- render-section sync, dirty/cache update application, traversal-ready facts,
  and render-resource lifetime
- full-frame flat composition: sky, terrain, actors, screen effects, selection
  outline, HUD, menus, debug pane, and loading/progress overlays

Host adapters own:

- event source: `winit`, Android input, browser events, scripted input, network
  input, or model/controller decisions
- target source: desktop surface frame, Android surface frame, browser canvas,
  offscreen texture, or future shared-texture target
- resize, device/surface loss, output format, readback/encoding, frame
  delivery, and presentation pacing
- platform-only concerns such as cursor lock, focus, window-manager behavior,
  app lifecycle, package validation, or network framing

## Non-Goals

Do not make offscreen/headless validation depend on a hidden desktop window. A
hidden `winit` window mostly validates window-manager and swapchain behavior,
which is useful only as a narrow desktop smoke. It is the wrong foundation for a
headless Linux or CI environment, and it does not model future remote UI or AI
client use.

Do not duplicate a second "screenshot renderer." Screenshot, recording,
remote-frame streaming, and model framebuffer handoff should all be sinks over
the same offscreen flat client host.

Do not treat OpenXR runtime emulation as part of this target. A future headless
stereo smoke can drive `mclone-xr-scene` with synthetic views and offscreen eye
targets, but OpenXR loader/session/swapchain behavior should remain a separate
platform/runtime validation concern.

## Current Gap

The existing code already has good low-level pieces:

- `mclone-render::target::RenderFrameContext` and `RenderFrameTarget` describe
  renderer inputs without knowing about windows or canvases.
- `mclone-render::headless` can create native offscreen GPU targets, submit a
  frame, read pixels, and write PNGs.
- `mclone-app-runtime::frame_render` contains the shared full-frame renderer.
- `WindowSceneRuntime` already wraps the shared native
  `NativeSingleViewSceneRuntime`, so local/remote session, runtime polling,
  chunk interest, render-section sync, and sky/time facts are mostly shared.

The gap is above those pieces. Desktop flat now has a native-client
`FlatClientDriver` staging owner for runtime, camera, interaction, actor
interpolation, render options, render stats, and frame timing, but the desktop
`winit` app shell still drives much of the real client lifetime directly.
`run_headless_screenshot` now uses `FlatRenderResources` and the shared
full-frame renderer bundle, but it is still screenshot-shaped: it creates fresh
resources per capture and owns scenario debug/UI setup plus scripted interaction
state. That still lets desktop features miss headless until session/UI/render
resource lifetime moves behind shared driver methods.

## Desired Cleanup Shape

Introduce a host-neutral flat client driver with a real lifetime:

```text
FlatClientDriver
  owns: runtime/session, camera/input state, UI state, interaction state,
        actor interpolation, render resources, render stats
  methods: start/join/teardown, resize_target, apply_input, tick/poll,
           render_frame, rebuild_render_resources
```

Desktop flat should become mostly:

- translate `winit` keyboard/mouse/focus/resize events into neutral input and
  UI events
- acquire a surface frame
- call the shared driver
- present and apply frame pacing

Offscreen flat should become mostly:

- construct the same driver
- feed scripted, network, or model input
- render to an offscreen target
- read back or encode/deliver the frame

The offscreen host should be able to run one screenshot, a deterministic frame
loop, a long-lived remote dedicated client, or a future remote UI session
without changing render/gameplay setup.

## Validation Direction

Pixel-producing validation should prefer the offscreen flat client once it
covers the needed feature. Older chunk-only and UI-only headless helpers may
remain as narrow renderer tests, but they should not be the main client feature
gate.

Target examples:

```bash
# Full flat-client screenshot from the real offscreen host.
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-flat-client.png \
  --width 1280 --height 720 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 \
  --day-time 6000 --freeze-time

# Future shape: deterministic client loop that writes frames or streams them.
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --offscreen-client --frames 240 --frame-sink png-sequence:/tmp/mclone-frames
```

## Related Tactical

Implementation is tracked in
[`tactical/105-offscreen-flat-client-host.md`](tactical/105-offscreen-flat-client-host.md).
The older
[`tactical/028-headless-window-frame-unification.md`](tactical/028-headless-window-frame-unification.md)
landed the first full-frame screenshot direction and is now a historical
precursor to this broader offscreen-client target.
