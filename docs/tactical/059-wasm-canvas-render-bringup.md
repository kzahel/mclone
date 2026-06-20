# 059: WASM Canvas Render Bring-Up

Status: completed first pass.

## Purpose

Move the native web/WASM target beyond runtime-only browser smoke by presenting
a Rust-owned `wgpu` frame into a real browser canvas.

The existing `mclone-web-client` path already proves:

- raw WASM builds and instantiates in a browser page
- the shared client/protocol/server loopback can load and unload chunks
- browser WebGPU capability probing is stable

This tactical crosses the first visual boundary without trying to port the
entire native desktop app loop at once.

## First Slice

Scope:

1. Keep the existing raw C ABI runtime smoke export intact.
2. Add a wasm-bindgen browser bundle for DOM/WebGPU entry points.
3. Create a `wgpu::Surface` from a `web_sys::HtmlCanvasElement`.
4. Request a WebGPU adapter/device for that surface.
5. Configure the surface and present a deterministic Rust-rendered clear frame.
6. Extend the Playwright smoke to:
   - build the raw WASM
   - generate the wasm-bindgen browser bundle
   - serve the bundle from localhost
   - load the runtime report
   - render the canvas
   - save full-page and canvas screenshots under `/tmp`
   - reject blank or non-rendered canvas pixels

## Boundary Direction

Keep browser platform ownership in `mclone-web-client` for now:

- DOM, canvas lookup, and wasm-bindgen exports belong to the web app crate.
- Shared render primitives remain in `mclone-render`.
- The existing `RenderFrameTarget` / explicit view-target style remains the
  shape to reuse when chunks move into the web canvas path.
- Do not introduce a JS WebGPU renderer or revive the legacy TypeScript engine.

The wasm-bindgen path is intentionally separate from the raw runtime smoke. The
raw export keeps the low-level compatibility signal, while the bindgen bundle
handles the browser APIs needed by `wgpu` and `web-sys`.

## Deferred

- Textured chunk rendering in the browser.
- Asset fetch/pack loading for web.
- Pointer lock, input, UI, and continuous animation.
- WebSocket/WebTransport remote sessions.
- Browser storage persistence.
- WASM workers, `SharedArrayBuffer`, and threaded mesh/light/status work.

## Validation

Required checks:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm --silent native:web:build
pnpm --silent native:web:smoke
pnpm --silent native:web:canvas-smoke
```

The smoke saves:

- `/tmp/mclone-native-web-smoke.png`
- `/tmp/mclone-native-web-canvas.png`

## Next

After the clear-frame canvas path is stable, the next slice should render one
generated chunk in the browser. Start with flat-color or a tiny generated atlas
fixture if asset packaging is still too large; do not block the browser render
boundary on full vanilla texture loading.
