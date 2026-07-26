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
stereo smoke can drive `mclone-scene` with synthetic views and offscreen eye
targets, but OpenXR loader/session/swapchain behavior should remain a separate
platform/runtime validation concern.

## Current Shape

Tactical 168 Slice 7d completed the native convergence:

- `mclone-scene` owns session/startup/replacement, camera and interaction,
  neutral input application, UI/HUD/debug state, actors, render admission,
  section sync/upload, traversal readiness, and full Mono frame composition.
- `WinitFrameDriver` owns the desktop surface, resize/render-scale targets,
  raw winit input delegation, and presentation cadence.
- `OffscreenDriver` owns native offscreen target sizing, deterministic cadence,
  readiness loops, frame-accounting feedback, and calls into the same Mono host.
- `offscreen_flat_client.rs` owns only screenshot scenarios, scripted neutral
  input/UI actions, camera overrides, report projection, and the PNG sink.
- timedemo, startup-streaming, frame-budget, movement-frame, and dual-view
  captures all use `OffscreenDriver`; no harness polls, synchronizes, uploads,
  or assembles render inputs independently.

The old app-local `FlatClientDriver` and deferred offscreen session-start
staging are deleted. Desktop and offscreen construction share one native
local/remote runtime factory. Screenshot readiness remains scenario-selectable
through `--startup-wait`. `playable` admits the first useful drawable view;
`view-settled`, the screenshot default, requires the server's accepted startup
view to match and complete the current center/radius request, every requested
chunk to be client-resident, and target render admission, compilation, upload,
and asset replacement to drain. Detached screenshot cameras affect capture
framing and camera-dependent render settlement without changing authoritative
startup interest. Performance probes use the same view-settled barrier while
measured frames retain the requested target cadence.

The remaining product gap is the long-lived exposed offscreen mode: today the
host is reusable internally, but CLI output is still screenshot/report oriented.
A future remote UI, video, or model client needs a public input-source/frame-sink
loop rather than a new renderer or client lifetime.

## Next Cleanup Shape

Build future offscreen products as sources and sinks around the existing driver:

```text
InputSource
  -> OffscreenDriver
  -> mclone-scene Mono host
  -> explicit RenderFrameTarget
  -> FrameSink
```

Likely additions are:

- a bounded or continuous frame-loop command with explicit cadence;
- pluggable frame sinks (PNG sequence, encoder, shared texture, network stream);
- an input-source trait or channel for scripted/network/model actions;
- lifecycle and backpressure policy for sinks that cannot consume every frame.

Those additions must not move runtime polling, render-section work, UI assembly,
or session policy back into `mclone-native-client`. Older chunk-only and
renderer-rebuild helpers remain narrow renderer tests, not alternate clients.

## Validation Direction

Pixel-producing validation should prefer the offscreen flat client once it
covers the needed feature. Older chunk-only and UI-only headless helpers may
remain as narrow renderer tests, but they should not be the main client feature
gate.

Target examples:

```bash
# Full flat-client screenshot from the real offscreen host.
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --screenshot /tmp/mclone-flat-client.png \
  --width 1280 --height 720 \
  --startup-wait view-settled \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 \
  --day-time 6000 --freeze-time

# Use the same host startup policy with playable readiness plus warmup frames.
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --screenshot /tmp/mclone-flat-client-warm.png \
  --width 1280 --height 720 \
  --startup-wait frames:2 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2

# Future shape: deterministic client loop that writes frames or streams them.
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --offscreen-client --frames 240 --frame-sink png-sequence:/tmp/mclone-frames
```

## Related Tactical

The shared-host implementation landed in
[`tactical/168-unified-native-scene-host.md`](tactical/168-unified-native-scene-host.md)
Slice 7d. The original target was developed in
[`tactical/105-offscreen-flat-client-host.md`](tactical/105-offscreen-flat-client-host.md),
and the older
[`tactical/028-headless-window-frame-unification.md`](tactical/028-headless-window-frame-unification.md)
landed the first full-frame screenshot direction and is now a historical
precursor to this broader offscreen-client target.
