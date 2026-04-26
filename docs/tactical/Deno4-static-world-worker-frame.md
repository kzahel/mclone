# Deno4: Static World Worker Frame

Status: done.

## Goal

Render the first browser-free chunk/world frame in Deno. The smoke must keep the render-world worker boundary in play: the main Deno thread sends packed authoritative chunk snapshots to a Deno module worker, the worker builds section meshes, the main thread uploads/draws those meshes through the existing chunk render pipelines, reads pixels back, and writes a PNG under `/tmp`.

This is not a live game loop yet. It intentionally avoids integrated-server startup, transport polling, asset-pack fetch, DOM, canvas, Playwright, Chrome, Rust/wgpu, and OpenXR.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `scripts/deno-static-world-smoke.ts` | done - main-thread Deno WebGPU smoke that prepares an in-memory atlas, spawns a Deno render-world worker, sends packed static chunk snapshots, renders one offscreen frame, validates pixels, and writes `/tmp/mclone-deno-static-world-smoke.png` |
| 2 | `scripts/deno-static-world-worker.ts` | done - Deno module worker that owns a `ClientChunkCache`, ingests packed chunk snapshots, and builds section meshes through `RenderChunkRegion` / `buildSectionMesh(...)` |
| 3 | `scripts/deno-static-world-smoke-shared.ts` | done - shared in-memory atlas/model helpers for the main thread and worker, including a tiny cube model and deterministic stone sprite UVs |
| 4 | `package.json` | done - pinned `pnpm smoke:deno:world` command with `--allow-read=.` for the Deno worker module graph and `--allow-write=/tmp` for the PNG artifact |
| 5 | `docs/tactical/README.md` / `docs/deno-wgpu-native-spike.md` | done - document the worker-backed static world smoke and next target |

Do not add:

- live host/client orchestration
- browser asset-pack loading
- real extracted texture/model loading
- DOM/canvas image decode
- Playwright/Chrome dependencies
- Rust/wgpu or OpenXR host code

## Command

```bash
pnpm smoke:deno:world
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-static-world-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-static-world-smoke.png
```

Expected readback:

```text
centerPixel: [141, 141, 141, 255]
nonClearPixels: >= 1000
```

## Implementation Notes

The smoke builds a tiny authoritative source level on the main thread:

- generated block registry palette
- 3x3 chunks around the camera target
- one 16x16 stone wall in chunk `(0, 0)`
- `buildChunkSnapshot(...)` + `packChunkSnapshot(...)` for each chunk

Those packed snapshots are transferred to a Deno `Worker` through `RenderWorldWorkerClient` / `RenderWorldWorkerUpdateSink`. The worker owns a `ClientChunkCache`, unpacks snapshots, and builds meshes through the same `RenderChunkRegion` and `buildSectionMesh(...)` path used by the browser render-world worker.

The main thread owns WebGPU resources:

- in-memory `TextureAtlasSource`
- `TextureAtlas.prepareToStitch(...)` / `TextureAtlas.reload(...)`
- `ChunkRenderDispatcher` with a render-world worker client
- `LevelRenderer` / `GameRenderer` frame assembly
- `encodeSceneFrame(...)` chunk draw submission
- readback to PNG

The command needs `--allow-read=.` because Deno requires explicit read permission for the module worker entrypoint and its imported module graph.

## Validation

Completed:

- `pnpm smoke:deno:world` - passed after granting network access for the pinned Deno package through `npx`
- inspected `/tmp/mclone-deno-static-world-smoke.png`: blue sky background with a centered gray stone wall
- `file /tmp/mclone-deno-static-world-smoke.png` - reported `PNG image data, 128 x 128, 8-bit/color RGBA, non-interlaced`

Also run:

- `pnpm test -- test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `git diff --check`

`pnpm typecheck` remains blocked by the unrelated existing worktree error in `test/runtime/generated-world-host-factory.test.ts(43,43)` accessing `WorldHostMessage.snapshot`.

Observed output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-static-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[141,141,141,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

## Done When

- Done: Deno can spawn a module worker for render-world mesh ownership.
- Done: packed authoritative chunk snapshots are transferred into the worker.
- Done: the worker builds an uploadable section mesh from `ClientChunkCache`.
- Done: the main thread uploads/draws that mesh through the existing chunk render pipelines.
- Done: the smoke validates readback pixels and writes a PNG under `/tmp`.

## Follow-Up

Next likely tactical: `Deno5` should replace the synthetic cube model/texture with a small filesystem-backed asset source for extracted vanilla assets, still without browser APIs. Load only the minimal stone blockstate/model/texture set first, then expand to a tiny real block palette once the Deno asset adapter is stable.
