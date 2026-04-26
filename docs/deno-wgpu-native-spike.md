# Deno / wgpu native spike

This is a concrete spike plan for a native runtime path that keeps the current TypeScript engine code while removing the browser as a hard dependency for renderer validation and, later, OpenXR.

It does not replace the browser target. The browser WebGPU build remains the default product target until this path proves useful.

## Motivation

There are two separate reasons to explore this:

- **Headless renderer validation:** render into a GPU texture, copy pixels into a CPU buffer, and compare or write PNGs without Playwright/Chrome/browser page state.
- **OpenXR:** own the native frame loop and XR session directly instead of waiting for browser WebXR/WebGPU support to become adequate.

The first reason is the start gate. If a browser-free WebGPU render-to-buffer smoke test is not reliable, the larger native-host idea should stop there.

## Proposed shape

Use Deno first as a browser-shaped, headless JavaScript runtime:

```text
Deno process
  TypeScript engine modules
  Web Worker-like runtime where available
  navigator.gpu with offscreen render targets
  test harness writes PNGs / compares pixels
```

If that works, use Rust later as the native host:

```text
Rust executable
  window / OpenXR / native event loop
  wgpu device, queues, surfaces, offscreen targets
  embedded Deno runtime or deno_core isolates
  Worker/postMessage/transfer host services
  filesystem, asset, input, and storage adapters

TypeScript engine
  simulation / host / client runtime
  workers as separate JS isolates
  meshing and renderer-neutral draw payloads
```

Rust owns native presentation and OpenXR timing. TypeScript continues to own parity-critical simulation and high-level runtime orchestration.

## Worker model

The durable mapping is:

```text
browser Worker = native thread + separate JS isolate + event loop + message queue
```

For the early Deno CLI spike, prefer Deno's built-in `Worker` behavior. For embedded Rust, model each worker as a Deno/deno_core runtime on its own thread.

Start with message copying for correctness. Add true transfer after the message surface is stable:

1. Clone small structured messages.
2. Transfer large `ArrayBuffer` payloads by moving backing storage and detaching the sender side.
3. Use `SharedArrayBuffer` only for a measured queue/ring-buffer need.

The repo is already close to this shape because chunk, light, and render-world payloads collect explicit transfer lists.

## Renderer target model

Split render targets by capability instead of assuming a browser canvas:

- `BrowserCanvasTarget`: `HTMLCanvasElement` + `GPUCanvasContext`.
- `OffscreenTextureTarget`: `GPUTexture` + readback buffer for tests.
- `NativeWindowTarget`: Rust/wgpu surface for flat desktop mode.
- `OpenXrTarget`: OpenXR swapchain-backed presentation path.

The first useful test target is `OffscreenTextureTarget`. It should be able to render one frame, copy the color target into a mapped buffer, and write `/tmp/mclone-deno-webgpu-smoke.png`.

## Validation sequence

1. **Deno WebGPU smoke**
   - Run the repo-owned `pnpm smoke:deno:webgpu` command.
   - Create `navigator.gpu` adapter/device.
   - Clear or draw into an offscreen `GPUTexture`.
   - Copy texture to a readback buffer.
   - Encode a PNG in `/tmp` and inspect it.

2. **Deno harness around existing renderer code**
   - Run the repo-owned `pnpm smoke:deno:pipeline` command.
   - Import the smallest renderer module set that can compile shaders and render known geometry.
   - Avoid DOM, page input, localStorage, and IndexedDB.

3. **Deno texture atlas smoke**
   - Run the repo-owned `pnpm smoke:deno:atlas` command.
   - Build a memory `TextureAtlasSource`, stitch/reload the atlas, and sample from the uploaded atlas texture.
   - Keep filesystem-backed asset loading and chunk/world rendering out of this step.

4. **Renderer target refactor**
   - Move canvas acquisition/configuration behind a target interface.
   - Keep `GPUDevice` and `GPUTexture` below the renderer boundary.
   - Keep world/runtime/client modules free of DOM and WebGPU handles.

5. **Asset/input/storage adapters**
   - Replace browser image/canvas decode with an asset decoder interface.
   - Keep browser `document`/pointer-lock/debug form code in browser-only entry points.
   - Add native/headless storage choices only behind existing persistence contracts.

6. **Rust native host spike**
   - Create a Rust `wgpu` offscreen clear/readback equivalent.
   - Embed Deno/deno_core only after the Deno CLI harness proves the engine-side seams are right.
   - Add windowed and OpenXR presenters after headless/native flat rendering works.

## Spike result: 2026-04-26

The first smoke passed on macOS and has been promoted to a repo command:

```bash
pnpm smoke:deno:webgpu
```

Observed runtime:

```text
deno 2.7.13
v8 14.7.173.20-rusty
typescript 5.9.2
```

The command runs:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-webgpu-smoke.ts
```

The script:

- created `navigator.gpu` adapter/device
- rendered a clear pass into an offscreen `rgba8unorm` `GPUTexture`
- copied the texture to a mapped readback buffer
- encoded `/tmp/mclone-deno-webgpu-smoke.png`

Observed output:

```json
{"ok":true,"adapter":{},"outputPath":"/tmp/mclone-deno-webgpu-smoke.png","firstPixel":[26,102,204,255],"byteLength":16384}
```

`file /tmp/mclone-deno-webgpu-smoke.png` reports:

```text
PNG image data, 64 x 64, 8-bit/color RGBA, non-interlaced
```

The image is a solid blue clear color, matching the expected `[26, 102, 204, 255]` readback pixel. This validates the basic Deno WebGPU path for offscreen render, GPU readback, and PNG artifact generation.

## Pipeline smoke result: 2026-04-26

The first renderer-pipeline smoke also passed on macOS:

```bash
pnpm smoke:deno:pipeline
```

The command runs:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-render-pipeline-smoke.ts
```

The script:

- creates `navigator.gpu` adapter/device
- creates an offscreen `rgba8unorm` `GPUTexture`
- builds a custom `POSITION_COLOR` triangle `RenderType`
- compiles the repo `position_color` shader through `RenderPipelineCache`
- creates the shader uniform buffer and bind group through `ShaderProgramDefinition`
- draws a green triangle into the offscreen target
- copies the texture to a mapped readback buffer
- validates the center pixel
- encodes `/tmp/mclone-deno-render-pipeline-smoke.png`

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-render-pipeline-smoke.png","width":64,"height":64,"format":"rgba8unorm","renderType":"deno_position_color_triangle","shader":"position_color","centerPixel":[51,204,77,255],"byteLength":16384,"adapter":{}}
```

`file /tmp/mclone-deno-render-pipeline-smoke.png` reports:

```text
PNG image data, 64 x 64, 8-bit/color RGBA, non-interlaced
```

The image is a black background with a green triangle. This validates shader JSON import, WGSL generation, pipeline creation, uniform binding, vertex buffer layout, draw submission, GPU readback, and PNG artifact generation in Deno.

## Texture decode smoke result: 2026-04-26

The first non-browser image decode and sampled texture smoke passed on macOS:

```bash
pnpm smoke:deno:texture
```

The command runs:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-texture-decode-smoke.ts
```

The script:

- creates a small PNG in memory
- decodes that PNG through `PngNativeImageDecoder`, not browser image/canvas APIs
- builds a `NativeImage` from decoded RGBA bytes
- uploads the `NativeImage` into a sampled `GPUTexture`
- verifies the uploaded source texture through readback
- draws a textured quad through the repo `position_tex` shader
- validates the center pixel
- encodes `/tmp/mclone-deno-texture-decode-smoke.png`

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-texture-decode-smoke.png","width":64,"height":64,"format":"rgba8unorm","sourceWidth":64,"sourceHeight":64,"renderType":"deno_position_tex_quad","shader":"position_tex","centerPixel":[230,76,13,255],"byteLength":16384,"adapter":{}}
```

`file /tmp/mclone-deno-texture-decode-smoke.png` reports:

```text
PNG image data, 64 x 64, 8-bit/color RGBA, non-interlaced
```

The image is an orange square on black. This validates the image-decoder seam, Deno PNG decode, `NativeImage` construction, texture upload, sampled texture binding, textured draw submission, readback, and PNG artifact generation without Chrome.

## Texture atlas smoke result: 2026-04-26

The first browser-free texture atlas smoke passed on Linux:

```bash
pnpm smoke:deno:atlas
```

The command runs:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-texture-atlas-smoke.ts
```

The script:

- creates two 16x16 `NativeImage` sprites in memory
- implements `TextureAtlasSource` without browser asset-pack or image APIs
- runs `TextureAtlas.prepareToStitch(...)`, including the normal `missingno` insertion
- runs `TextureAtlas.reload(...)` to upload the stitched atlas into a WebGPU texture
- samples the uploaded atlas through the repo `position_tex` shader
- validates known left/right pixels
- encodes `/tmp/mclone-deno-texture-atlas-smoke.png`

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-texture-atlas-smoke.png","width":64,"height":64,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":32,"mipLevel":0,"renderType":"deno_texture_atlas_quads","shader":"position_tex","leftPixel":[230,76,13,255],"rightPixel":[13,188,230,255],"byteLength":16384,"adapter":{}}
```

`file /tmp/mclone-deno-texture-atlas-smoke.png` reports:

```text
PNG image data, 64 x 64, 8-bit/color RGBA, non-interlaced
```

The image is an orange quad on the left and a teal quad on the right with a black background/gap. This validates atlas preparation, stitching, atlas texture upload, sampled atlas binding, readback, and PNG artifact generation without Chrome.

## Refactor seams to preserve

- Simulation and protocol messages stay serializable and renderer-neutral.
- Workers are used through endpoint interfaces, not concrete browser globals.
- Texture/image loading is an adapter.
- Canvas/window/input/UI are entry-point concerns.
- Browser debug overlays remain browser-only; native HUD/debug UI should be renderer-owned or native-host owned.

## Stop conditions

Pause this path if:

- Deno WebGPU cannot reliably render and read back pixels on target machines.
- The first useful harness requires broad DOM emulation.
- Worker transfer semantics require invasive changes above existing transport/message boundaries.
- OpenXR swapchain integration forces renderer assumptions that would contaminate simulation/runtime code.
