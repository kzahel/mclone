# RendererHost3: Generated World Headless Runtime

Status: done.

## Goal

Move the Deno headless renderer from static chunk snapshots to the same generated-world runtime path used by browser smoke. The host should inject camera/input state and render to an offscreen target, but chunk generation, client-world hydration, render-world ingestion, and mesh compilation should flow through the shared `ClientRuntime` / integrated-server shape instead of a Deno-only static snapshot shortcut.

This is the first slice that should prove the headless lane can exercise real engine harness behavior without depending on Chrome.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | `scripts/deno-generated-world-smoke.ts` / `package.json` | done - `pnpm smoke:deno:generated-world` boots generated-world runtime, sets chunk interest from an injected camera, waits for the loaded chunk ring, renders one settled frame, and writes `/tmp/mclone-deno-generated-world-smoke.png` |
| 2 | `src/renderer/generated-world-headless-harness.ts` | done - reusable headless generated-world harness over `ClientRuntime`, integrated-server transport, render-world worker ingestion, frame settling, and counter collection |
| 3 | `scripts/deno-generated-world-worker.ts` | done - Deno module worker runs the generated-world host behind the same world-worker transport/integrated-server shape as browser singleplayer |
| 4 | `scripts/deno-generated-render-world-worker.ts` / `src/renderer/chunk/render-world-worker-handler.ts` | done - Deno render-world worker reuses the shared render-world protocol handler with a Deno file-backed asset context |
| 5 | `src/renderer/chunk/asset-pack-mesh-context.ts` | done - source-level asset-pack model/atlas/biome-color context for Deno render-world worker and headless main-thread renderer setup |
| 6 | `src/renderer/texture/png-native-image-decoder.ts` | done - native PNG decoder now supports the vanilla grayscale-alpha and indexed-palette PNGs needed by the full generated block atlas |
| 7 | docs | done - record which browser assumptions remain and which host adapters are now shared |

Do not add:

- native windowing
- full gameplay input loop
- persistent Deno storage
- remote multiplayer transport
- browser-only fallback shims

## Design Notes

The expected shape is:

```text
Deno headless host
  -> WebGPU device and offscreen target
  -> generated-world runtime/client setup
  -> injected camera and chunk interest
  -> shared render-world worker and mesh path
  -> settled frame encode/readback
  -> /tmp PNG artifact
```

Prefer extracting browser smoke setup into shared runtime/renderer helpers only where the Deno path needs the same behavior. Do not copy the whole browser boot function into a Deno script. The end state is one harness API with browser and headless adapters, not two independently maintained smokes.

## Validation

Completed:

- `pnpm smoke:deno:generated-world` - passed
- inspected `/tmp/mclone-deno-generated-world-smoke.png`: blue sky with a generated textured terrain island/mountain visible in frame
- `file /tmp/mclone-deno-generated-world-smoke.png` - reported `PNG image data, 256 x 256, 8-bit/color RGBA, non-interlaced`
- `pnpm smoke:deno:world-assets` - passed, proving the earlier static vanilla-asset lane still works
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `pnpm typecheck` - still blocked only by the unrelated existing `test/runtime/generated-world-host-factory.test.ts(43,43)` `WorldHostMessage.snapshot` error
- `git diff --check`

Observed generated-world output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-generated-world-smoke.png","width":256,"height":256,"format":"rgba8unorm","saveId":"generated-world-v5-browser_smoke-12345-liquid-none","expectedLoadedChunkCount":25,"loadedChunkCount":25,"viewDistance":1,"renderDistance":112,"solidDrawCount":8,"nonClearPixels":9004,"centerPixel":[143,184,255,255],"terrainPixel":[79,105,79,255],"byteLength":262144,"renderWorldCounters":{"ingestBatchCount":7,"meshBuildRequestCount":106,"meshNotReadyResponseCount":83,"meshCompletionCount":23,"mainThreadGpuUploadCount":20},"renderQueueStats":{"renderedChunkCount":8,"pendingVisibleChunkCompileCount":0,"queuedChunkBuildCount":0,"activeChunkBuildCount":0},"sessionId":"local-session","playerId":"local-player","playerTick":0,"playerPosition":[968.5,168,-8119.5],"adapter":{}}
```

Browser smoke can remain documented as unavailable on this headless Linux host because Chrome headless WebGPU fails at `GPUQueue.onSubmittedWorkDone()`. Do not block this Deno slice on Chrome here.

## Done When

- Done: Deno renders a generated-world frame through shared runtime/client/render-world setup.
- Done: the camera/chunk-interest input is injected by the host instead of read from DOM events.
- Done: the output PNG and counters prove real generated chunks reached mesh upload and draw submission.
- Done: static Deno smokes continue to pass through `src/renderer/static-frame-harness.ts`.

## Follow-Up

Next tactical: [`RendererHost4-shared-generated-world-scenarios.md`](RendererHost4-shared-generated-world-scenarios.md) should move toward reusable harness parity for browser probes and headless Deno generated-world probes: one set of scenario inputs, two target adapters.
