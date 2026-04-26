# Deno2: Texture Decode Smoke

Status: done.

## Goal

Add a non-browser image decode seam and prove Deno can decode PNG bytes into `NativeImage`, upload that image as a sampled WebGPU texture, draw through a real textured renderer pipeline, read pixels back, and write a PNG under `/tmp`.

This keeps the Deno lane browser-free: no DOM, no `createImageBitmap`, no `HTMLCanvasElement`, no `OffscreenCanvas`, no Playwright, and no Chrome.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/renderer/texture/native-image-decoder.ts` | done - host-neutral `NativeImageDecoder` interface plus URL/blob helpers |
| 2 | `src/renderer/texture/browser-native-image-loader.ts` | done - browser loader now implements/injects `NativeImageDecoder` without changing default behavior |
| 3 | `src/renderer/texture/png-native-image-decoder.ts` | done - Deno-safe 8-bit RGBA PNG decoder into `NativeImage` |
| 4 | `src/renderer/texture/native-image.ts` | done - `NativeImage.fromRgbaPixels(...)` for decoded host-neutral RGBA bytes |
| 5 | `scripts/deno-texture-decode-smoke.ts` | done - Deno script that decodes a PNG, uploads it as a texture, draws with `position_tex`, validates a readback pixel, and writes `/tmp/mclone-deno-texture-decode-smoke.png` |
| 6 | `package.json` | done - pinned `pnpm smoke:deno:texture` command |

Do not add:

- atlas stitching
- browser asset pack loading in Deno
- chunk/world rendering
- DOM/canvas emulation
- Rust/wgpu or OpenXR host code

## Command

```bash
pnpm smoke:deno:texture
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-texture-decode-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-texture-decode-smoke.png
```

Expected center pixel:

```text
[230, 76, 13, 255]
```

## Implementation Notes

The PNG decoder supports the subset needed by the headless lane:

- PNG signature / `IHDR` / concatenated `IDAT` / `IEND`
- 8-bit RGBA (`colorType=6`)
- no interlace
- PNG filter types `0..4`
- zlib inflate through `DecompressionStream`, with a stored-block fallback for the repo-generated PNGs

The smoke uses a 64x64 source texture so `NativeImage.upload(...)` uses a 256-byte row. That keeps the Deno WebGPU upload path inside strict row-alignment requirements.

## Validation

Completed:

- `pnpm smoke:deno:webgpu` - passed after granting network access for the pinned Deno binary fetch through `npx`
- `pnpm smoke:deno:pipeline` - passed after granting network access for the pinned Deno binary fetch through `npx`
- `pnpm smoke:deno:texture` - passed after granting network access for the pinned Deno binary fetch through `npx`
- inspected `/tmp/mclone-deno-texture-decode-smoke.png`: orange square on black background; center pixel matched `[230, 76, 13, 255]`
- `pnpm test:browser` - passed after granting local-port access for the Vite web server
- `git diff --check`

Also run:

- `pnpm typecheck` - still blocked by unrelated worktree error in `test/runtime/generated-world-host-factory.test.ts`

## Done When

- Done: browser texture loading has an injectable image decoder seam.
- Done: Deno can decode PNG bytes into `NativeImage` without browser image/canvas APIs.
- Done: Deno can upload the decoded `NativeImage` as a sampled texture.
- Done: a repo-owned command draws a textured quad through `position_tex` and validates a known readback pixel.

## Follow-Up

Next likely tactical: `Deno3` should create an in-memory `TextureAtlasSource` for Deno and run `TextureAtlas.prepareToStitch(...)` / `TextureAtlas.reload(...)` without browser asset-pack or image APIs. That is the last small texture-system step before a minimal chunk/world frame.
