# Deno3: Texture Atlas Smoke

Status: done.

## Goal

Prove the texture atlas path can run in Deno without browser asset-pack or image APIs. The smoke builds an in-memory `TextureAtlasSource`, runs `TextureAtlas.prepareToStitch(...)` and `TextureAtlas.reload(...)`, samples from the uploaded atlas through the repo `position_tex` pipeline, validates readback pixels, and writes a PNG under `/tmp`.

This remains a headless renderer validation lane: no DOM, no canvas, no Playwright, no Chrome, no asset-pack fetch, and no chunk/world rendering.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `scripts/deno-texture-atlas-smoke.ts` | done - Deno script that stitches two memory sprites plus `missingno`, reloads the atlas into a `GPUTexture`, draws two sampled atlas quads, validates known pixels, and writes `/tmp/mclone-deno-texture-atlas-smoke.png` |
| 2 | `src/renderer/texture/native-image.ts` | done - `NativeImage.upload(...)` pads source rows to WebGPU's 256-byte copy row alignment so 16x16 atlas sprites upload reliably in Deno/wgpu |
| 3 | `package.json` | done - pinned `pnpm smoke:deno:atlas` command |
| 4 | `docs/tactical/README.md` / `docs/deno-wgpu-native-spike.md` | done - document the new atlas smoke lane and next target |

Do not add:

- browser asset-pack loading in Deno
- filesystem-backed Minecraft asset loading
- DOM/canvas image decode
- chunk/world rendering
- Rust/wgpu or OpenXR host code

## Command

```bash
pnpm smoke:deno:atlas
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-texture-atlas-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-texture-atlas-smoke.png
```

Expected sampled pixels:

```text
left:  [230, 76, 13, 255]
right: [13, 188, 230, 255]
```

## Implementation Notes

The memory source implements the same `TextureAtlasSource` interface as the browser asset-pack loader:

- `getBasicSpriteInfos(...)` returns `TextureAtlasSpriteInfo` for two in-memory 16x16 `NativeImage` sprites.
- `loadSprite(...)` copies the source `NativeImage` and creates real `TextureAtlasSprite` instances.
- `TextureAtlas.prepareToStitch(...)` still injects the translated `missingno` sprite, so the smoke exercises normal atlas packing behavior.
- `TextureAtlas.reload(...)` uploads the atlas through the same `TextureAtlasUploadTarget` path used by the browser renderer.

`NativeImage.upload(...)` now pads rows before `GPUQueue.writeTexture(...)`. This preserves the existing byte layout while satisfying stricter Deno/wgpu validation for narrow sprite uploads.

## Validation

Completed:

- `pnpm smoke:deno:atlas` - passed after granting network access for the pinned Deno package through `npx`
- inspected `/tmp/mclone-deno-texture-atlas-smoke.png`: orange quad on the left, teal quad on the right, black background/gap; sampled pixels matched `[230, 76, 13, 255]` and `[13, 188, 230, 255]`

Also run:

- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts` - passed
- `pnpm smoke:deno:pipeline` - passed after granting network access for the pinned Deno package through `npx`
- `pnpm smoke:deno:texture` - passed after granting network access for the pinned Deno package through `npx`
- `pnpm smoke:deno:webgpu` - passed on Linux after allowing one-byte clear-color readback tolerance
- `git diff --check`

`pnpm typecheck` remains blocked by the unrelated existing worktree error in `test/runtime/generated-world-host-factory.test.ts`.

## Done When

- Done: Deno can build an in-memory `TextureAtlasSource` without browser asset/image APIs.
- Done: `TextureAtlas.prepareToStitch(...)` runs in Deno and produces real sprite UVs.
- Done: `TextureAtlas.reload(...)` uploads the stitched atlas into a sampled WebGPU texture.
- Done: the smoke samples from the uploaded atlas, validates known readback pixels, and writes a PNG under `/tmp`.

## Follow-Up

Completed by [`Deno4-static-world-worker-frame.md`](Deno4-static-world-worker-frame.md): render a minimal chunk/world frame from packed authoritative snapshots through a Deno render-world worker and offscreen WebGPU target.
