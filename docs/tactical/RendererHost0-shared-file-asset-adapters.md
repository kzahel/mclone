# RendererHost0: Shared File Asset Adapters

Status: done.

## Goal

Pivot the Deno headless work from one-off smoke shims toward reusable host/platform adapters. The first slice extracts the file-backed asset and native texture-atlas loading path into engine source modules so browser, Deno headless, Node/headless, and future native hosts can share the same `AssetPack` and `TextureAtlasSource` shapes.

This intentionally does not expand the rendered block palette yet. The point is to remove script-local Deno asset logic and make the host boundary explicit.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/renderer/assets/file-asset-pack.ts` | done - reusable `FileAssetPack` implementing `AssetPack` with injected host filesystem operations |
| 2 | `src/renderer/texture/asset-pack-texture-atlas-source.ts` | done - reusable `AssetPackTextureAtlasSource` that decodes image bytes natively, parses `.mcmeta`, and implements `TextureAtlasSource` |
| 3 | `scripts/deno-file-asset-source.ts` | done - thin Deno host adapter that supplies `Deno.statSync`, `Deno.readTextFile`, `Deno.readFile`, and `Deno.errors.NotFound` to `FileAssetPack` |
| 4 | `scripts/deno-vanilla-asset-world-smoke-shared.ts` | done - Deno smoke now uses source-level adapters instead of script-local file and atlas source classes |
| 5 | `src/renderer/texture/browser-native-image-loader.ts` / `texture-atlas-source-utils.ts` | done - browser texture loader keeps its public helper exports while shared metadata parsing lives in a neutral texture module |
| 6 | tactical docs | done - index records the new renderer host integration direction and the next host slice |

Do not add:

- more block palette coverage
- generated-world smoke parity
- renderer target abstraction
- worker factory abstraction
- input abstraction
- native window or OpenXR host code

## Design Notes

The reusable shape is:

```text
host filesystem operations
  -> FileAssetPack
  -> AssetPackTextureAtlasSource
  -> TextureAtlas.prepareToStitch(...) / reload(...)
```

`FileAssetPack` is host-neutral. It has no Deno, Node, browser, or native assumptions; the host supplies file operations. The Deno script adapter is deliberately thin and can be replaced later by Node filesystem or native embedded-host implementations without changing atlas/model loading code.

`AssetPackTextureAtlasSource` is also host-neutral. It reads bytes through `AssetPack`, decodes PNGs through the native decoder path, parses animation metadata, and creates `TextureAtlasSprite` instances. That makes it suitable for Deno headless today and native/headless hosts later.

## Validation

Completed:

- `pnpm smoke:deno:world-assets` - passed after extracting the shared adapters
- inspected `/tmp/mclone-deno-vanilla-asset-world-smoke.png`: blue sky background with a centered, textured Minecraft stone wall
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/model/block-model-baking.test.ts test/renderer/model/block-model-blockstates.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `git diff --check`

`pnpm typecheck` remains blocked by the unrelated existing worktree error in `test/runtime/generated-world-host-factory.test.ts(43,43)` accessing `WorldHostMessage.snapshot`.

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-vanilla-asset-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"mipLevel":0,"stoneTexture":"minecraft:block/stone","stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[93,93,93,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

## Done When

- Done: file-backed asset reads are behind a reusable `AssetPack` implementation.
- Done: native atlas loading from `AssetPack` is in renderer source, not a Deno script shim.
- Done: the Deno vanilla-asset smoke uses those source modules through a thin Deno host adapter.
- Done: browser texture metadata helper imports remain compatible.

## Follow-Up

Next tactical: `RendererHost1` should introduce a shared renderer host/platform boundary for target creation and worker creation. The first implementation should make the Deno world-assets smoke and the browser renderer setup call a common shape for offscreen/canvas target ownership and render-world worker construction, without changing visual behavior.
