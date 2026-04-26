# Deno5: Vanilla Asset World Smoke

Status: done.

## Goal

Replace the Deno4 synthetic cube model and synthetic stone texture with a small filesystem-backed vanilla asset source. The smoke still renders one browser-free static world frame in Deno through a Deno module render-world worker, but now the atlas, model baking, and chunk mesh UVs come from extracted Minecraft assets on disk.

This is not a live game loop or full asset-pack port yet. It intentionally limits the asset set to the vanilla stone blockstate/model/texture path.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `scripts/deno-file-asset-source.ts` | done - file-backed `AssetPack` plus Deno `TextureAtlasSource` that reads extracted assets, decodes PNG bytes through `PngNativeImageDecoder`, and parses `.mcmeta` when present |
| 2 | `scripts/deno-vanilla-asset-world-smoke-shared.ts` | done - shared stone asset constants, required-asset checks, vanilla atlas preparation, sprite lookup, and focused stone model baking |
| 3 | `scripts/deno-vanilla-asset-world-worker.ts` | done - Deno module worker that prepares matching sprite UVs from extracted assets and builds section meshes with the real baked stone model |
| 4 | `scripts/deno-vanilla-asset-world-smoke.ts` | done - main-thread smoke that reloads the vanilla stone atlas into WebGPU, sends packed static chunk snapshots to the worker, draws the worker-built mesh, validates pixels, and writes `/tmp/mclone-deno-vanilla-asset-world-smoke.png` |
| 5 | `package.json` | done - pinned `pnpm smoke:deno:world-assets` command |
| 6 | `docs/tactical/README.md` / `docs/deno-wgpu-native-spike.md` | done - document the vanilla-asset Deno smoke and next target |

Do not add:

- full generated block palette asset loading
- biome color table loading
- live host/client orchestration
- browser asset-pack fetch
- DOM/canvas image decode
- Playwright/Chrome dependencies
- Rust/wgpu or OpenXR host code

## Command

```bash
pnpm smoke:deno:world-assets
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-vanilla-asset-world-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-vanilla-asset-world-smoke.png
```

Expected readback:

```text
centerPixel: gray, opaque, stone-like
nonClearPixels: >= 1000
```

## Implementation Notes

The smoke reads assets from:

```text
reference/minecraft-1.17.1/extracted/assets
```

Required initial asset set:

- `assets/minecraft/blockstates/stone.json`
- `assets/minecraft/models/block/stone.json`
- `assets/minecraft/models/block/stone_mirrored.json`
- `assets/minecraft/models/block/cube.json`
- `assets/minecraft/models/block/cube_all.json`
- `assets/minecraft/models/block/cube_mirrored.json`
- `assets/minecraft/models/block/cube_mirrored_all.json`
- `assets/minecraft/models/block/block.json`
- `assets/minecraft/textures/block/stone.png`

The main thread and worker both prepare the same atlas inputs from extracted files. The main thread then reloads those preparations into a real WebGPU `TextureAtlas`; the worker keeps the prepared `TextureAtlasSprite` metadata so baked quads use UVs matching the main atlas upload.

The renderer path remains the Deno4 path:

- static 3x3 authoritative chunk set
- one 16x16 stone wall in chunk `(0, 0)`
- packed snapshots sent through `RenderWorldWorkerClient`
- worker-owned `ClientChunkCache`
- mesh build through `RenderChunkRegion` / `buildSectionMesh(...)`
- main-thread upload/draw through `ChunkRenderDispatcher`, `LevelRenderer`, `GameRenderer`, and `encodeSceneFrame(...)`

## Validation

Completed:

- `pnpm smoke:deno:world-assets` - passed after granting network access for the pinned Deno package through `npx`
- inspected `/tmp/mclone-deno-vanilla-asset-world-smoke.png`: blue sky background with a centered, textured Minecraft stone wall
- `file /tmp/mclone-deno-vanilla-asset-world-smoke.png` - reported `PNG image data, 128 x 128, 8-bit/color RGBA, non-interlaced`

Also run:

- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/model/block-model-baking.test.ts test/renderer/model/block-model-blockstates.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `git diff --check`

`pnpm typecheck` remains blocked by the unrelated existing worktree error in `test/runtime/generated-world-host-factory.test.ts(43,43)` accessing `WorldHostMessage.snapshot`.

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-vanilla-asset-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"mipLevel":0,"stoneTexture":"minecraft:block/stone","stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[93,93,93,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

## Done When

- Done: Deno reads model JSON and PNG texture bytes from extracted vanilla assets.
- Done: PNG decode uses the native decoder path instead of browser image/canvas APIs.
- Done: `TextureAtlas.prepareToStitch(...)` and `TextureAtlas.reload(...)` run with the real stone texture.
- Done: `ModelBakery` bakes the vanilla stone blockstate/model path.
- Done: a Deno render-world worker builds a section mesh with the real baked model and matching atlas UVs.
- Done: the main thread draws the mesh and writes a inspected PNG under `/tmp`.

## Follow-Up

Next likely tactical: `Deno6` should expand this from one stone block to a tiny real block palette loaded from extracted assets, still with Deno workers and no browser APIs. Start with opaque terrain blocks that avoid biome tint and liquids, then add color maps/special renderers as a separate step.
