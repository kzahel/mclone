# 000: Native Render Bring-Up And Validation

Status: completed in `0f29a3b`.

Build the first native desktop render path and headless GPU validation path for the Rust engine. This slice starts from a clear-color frame, then becomes the foundation for rendering one generated chunk.

## Durable References

Primary native roadmap:

- [`../../native-rewrite-roadmap.md`](../../native-rewrite-roadmap.md)

Reference Rust engine:

- Local path: `~/code/playbox`
- Useful docs:
  - `~/code/playbox/docs/architecture/runtime-loop.md`
  - `~/code/playbox/docs/architecture/rendering.md`
  - `~/code/playbox/docs/architecture/platforms.md`
  - `~/code/playbox/docs/validation.md`
- Useful source references:
  - `~/code/playbox/src/gpu.rs`: `wgpu` instance, surface, adapter/device, surface format, resize, present mode
  - `~/code/playbox/src/app.rs`: `winit` `ApplicationHandler`, startup failure handling, redraw, surface error handling
  - `~/code/playbox/src/desktop/frame_pacing.rs`: frame pacing and present-mode policy
  - `~/code/playbox/src/render/targets.rs`: depth/offscreen target wrappers
  - `~/code/playbox/src/render/camera.rs`: camera math and tests
  - `~/code/playbox/src/headless.rs`: offscreen rendering, readback, PNG capture

Use Playbox as a pattern library, not as a dependency and not as architecture to copy wholesale. Do not import PhysX, VaM-specific systems, egui-first UI, or the monolithic world/runtime shape.

## Coordinate System

Mclone world coordinates follow Minecraft Java:

- `+X`: east
- `+Y`: up
- `+Z`: south
- `-Z`: north
- block cell `(x, y, z)` occupies `[x, x + 1]`, `[y, y + 1]`, `[z, z + 1]`
- chunk coordinates are `(chunk_x, chunk_z)` and chunk-local block coordinates are `0..15` on X/Z

Treat this as the engine world-space contract. Rendering may use `wgpu`/`glam` right-handed view/projection helpers, but it must adapt the camera/view transform to the Minecraft world instead of flipping world data or changing chunk coordinates.

With the standard vector cross product, this world basis is right-handed: `+X x +Y = +Z`. Minecraft yaw is the part that needs care: positive yaw turns from south toward west, which is the opposite sign of the usual positive mathematical yaw about `+Y` in many right-handed camera helpers.

When player yaw lands, preserve Minecraft's convention:

- yaw `0`: faces south (`+Z`)
- yaw `90`: faces west (`-X`)
- yaw `180`: faces north (`-Z`)
- yaw `-90` / `270`: faces east (`+X`)

## Scope

1. Add native `wgpu`/`winit` bring-up dependencies.
2. Implement a reusable `mclone_render` native surface context.
3. Implement a headless clear-color capture path that writes a PNG under `/tmp`.
4. Replace the native client scaffold with:
   - default windowed clear-color app
   - `--headless-clear <path>` validation mode
5. Validate by running the headless capture and inspecting the image.

## Out Of Scope

- chunk meshing
- worldgen-to-render chunk bridge
- textures, block models, atlas loading
- UI beyond temporary keyboard/window handling
- OpenXR implementation
- web/WASM render boot

## XR Pressure

Desktop native OpenXR is a near-term target. This means the first renderer shape should avoid assumptions that only fit a single desktop swapchain:

- keep view/projection matrices explicit
- keep render targets explicit
- avoid browser-only or `winit`-only types in renderer-facing data contracts
- keep headless/offscreen targets available for validation
- leave room for stereo views and XR swapchain wrapping later

Playbox's OpenXR path is useful future reference, but this slice should not pull XR code forward yet.

## Validation

Required for this slice:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-clear /tmp/mclone-native-clear.png --width 96 --height 64
git diff --check
```

Then inspect `/tmp/mclone-native-clear.png`. The image should be a nonblank solid clear color with the requested dimensions.

Windowed validation is useful when a display is available:

```bash
cargo run -p mclone-native-client
```

Do not require manual user verification for the first GPU path. Headless capture is the acceptance gate.

## Next Slice

Active follow-up:

- [`001-one-generated-chunk-render.md`](001-one-generated-chunk-render.md)
