# RendererHost2: Shared Frame Harness

Status: done.

## Goal

Replace the duplicated Deno smoke-frame orchestration with a source-level harness that can run against either a browser canvas host or a headless offscreen host. The harness should keep the same engine-facing API shapes for scene setup, camera/input injection, render-world worker lifecycle, frame encoding, readback, and artifact writing.

The purpose is parity of harness shape, not feature expansion. Deno should remain the first non-browser validation lane, but the code it exercises should be the same frame path a browser probe can call.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/renderer/static-frame-harness.ts` | done - owns render-world worker client setup, static snapshot ingestion, chunk dispatcher/view-area setup, level/game renderer setup, frame encode, and readback hooks behind injectable host/scene inputs |
| 2 | `scripts/deno-vanilla-asset-world-smoke.ts` | done - thin Deno entry point that supplies host adapters, vanilla asset inputs, camera, static chunks, and `/tmp` output path |
| 3 | `scripts/deno-static-world-smoke.ts` | done - older synthetic static smoke now uses the same harness instead of carrying a second copy of frame orchestration |
| 4 | browser-callable helper shape | done - `encodeRendererHarnessFrame(...)` and the harness-owned `RendererScene` shape can target a browser canvas path when Chrome/WebGPU validation is available |
| 5 | docs | done - record the shared harness contract and the next boundary after frame orchestration |

Do not add:

- generated-world Deno parity yet
- generalized input device handling beyond injectable camera/input state
- native windowing
- full asset-pack/block-palette expansion
- worker protocol changes unless the shared harness exposes a real mismatch

## Design Notes

The desired shape is:

```text
host adapters
  -> assets / decode / worker / target
scene inputs
  -> chunks / camera / dimensions / clear-sky settings
shared frame harness
  -> render-world ingestion
  -> mesh build and upload
  -> encode frame
  -> readback or browser screenshot hook
```

The harness owns lifecycle cleanup for the render-world worker, view-area buffers, chunk dispatcher, light texture, offscreen depth target, and offscreen color target it creates. Browser-specific screenshot capture and Deno-specific PNG writing stay in entry points; the shared harness returns pixels, counters, draw stats, and frame metadata in host-neutral result objects.

## Validation

Completed:

- `pnpm smoke:deno:world-assets`
- inspected `/tmp/mclone-deno-vanilla-asset-world-smoke.png`: blue sky background with a centered, textured Minecraft stone wall
- `pnpm smoke:deno:world`
- inspected `/tmp/mclone-deno-static-world-smoke.png`: blue sky background with a centered, solid gray synthetic stone wall
- `file /tmp/mclone-deno-vanilla-asset-world-smoke.png` and `file /tmp/mclone-deno-static-world-smoke.png` - both reported `PNG image data, 128 x 128, 8-bit/color RGBA, non-interlaced`
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `pnpm typecheck` - still blocked only by the unrelated existing `test/runtime/generated-world-host-factory.test.ts(43,43)` `WorldHostMessage.snapshot` error
- `git diff --check`

Run `pnpm test:browser` when a working Chrome/WebGPU environment is available. On this current headless Linux host, Chrome headless WebGPU is expected to fail at `GPUQueue.onSubmittedWorkDone()`; Deno WebGPU is the active headless validation lane.

Observed vanilla-asset output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-vanilla-asset-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"mipLevel":0,"stoneTexture":"minecraft:block/stone","stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[93,93,93,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

Observed synthetic static output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-static-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[141,141,141,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

## Done When

- Done: Deno vanilla-asset smoke is mostly host/input wiring around a shared harness.
- Done: Deno static-world smoke uses the same shared frame harness.
- Done: browser code can call the same harness shape for a canvas-backed frame or probe.
- Done: the shared result includes enough counters and pixel/readback metadata to preserve the existing Deno assertions.
- Done: the Deno PNGs remain visually unchanged.

## Follow-Up

Next tactical: [`RendererHost3-generated-world-headless-runtime.md`](RendererHost3-generated-world-headless-runtime.md) should move from static scene parity to generated-world scene parity: a headless harness input that uses the same `ClientRuntime` / integrated-server path as browser smoke, with injected input/camera state and no browser-only assumptions.
