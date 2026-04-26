# RendererHost1: Render Target And Worker Host

Status: done.

## Goal

Move the next Deno headless integration point out of script-local wiring and into a shared renderer host boundary. Browser and headless hosts should now provide the same kind of platform object for WebGPU device acquisition, target creation, and render-world worker construction.

This is still intentionally narrow. It does not replace the smoke orchestration yet; it only makes the target and worker ownership seams explicit so the next slice can share more of the frame harness without inventing a Deno-only path.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/renderer/renderer-host.ts` | done - shared browser/headless host interfaces for WebGPU device context, canvas/offscreen target creation, and render-world worker endpoint factories |
| 2 | `src/renderer/scene-setup.ts` | done - browser scene setup accepts an injectable `BrowserRendererHost` and defaults to the standard browser host |
| 3 | `scripts/deno-vanilla-asset-world-smoke.ts` | done - Deno vanilla-asset smoke uses `createHeadlessRendererHost(...)` for device acquisition, offscreen target creation, and Deno module worker construction |
| 4 | `vite.config.ts` / `playwright.config.ts` | done - browser smoke can run on a selected `VITE_PORT` when the default dev-server port is occupied |
| 5 | tactical docs | done - index records this slice and the next shared frame-harness slice |

Do not add:

- shared input injection
- shared static/generated frame orchestration
- live generated-world Deno smoke
- native window or Rust/wgpu host code
- browser behavior changes beyond using the injected default host

## Design Notes

The reusable shape is:

```text
renderer host
  -> request WebGPU device context
  -> create canvas or offscreen render target
  -> create render-world worker endpoint
  -> renderer scene / smoke harness
```

`BrowserRendererHost` owns DOM canvas configuration and browser module-worker construction. `HeadlessRendererHost` owns offscreen `GPUTexture` target creation and accepts a host-supplied worker endpoint factory. The Deno worker remains a real Deno module worker; the difference is that construction now flows through the same renderer-host contract the browser path uses.

This keeps the host/platform split pointed at the larger end state: browser, Deno headless, and future native hosts share engine APIs while only the platform adapters vary.

## Validation

Completed:

- `pnpm smoke:deno:world-assets` - passed through the new `HeadlessRendererHost`
- inspected `/tmp/mclone-deno-vanilla-asset-world-smoke.png`: blue sky background with a centered, textured Minecraft stone wall
- `file /tmp/mclone-deno-vanilla-asset-world-smoke.png` - reported `PNG image data, 128 x 128, 8-bit/color RGBA, non-interlaced`
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts` - passed
- `git diff --check`

Also prepared local browser prerequisites:

- `pnpm assets:pack` - regenerated the local browser asset pack because this host had not had assets packed recently
- browser dev-server configuration now accepts `VITE_PORT`; `5173` and `3402` were already occupied by unrelated local servers on this host

`pnpm typecheck` remains blocked by the unrelated existing worktree error in `test/runtime/generated-world-host-factory.test.ts(43,43)` accessing `WorldHostMessage.snapshot`.

`CI=1 VITE_PORT=3501 pnpm test:browser` reached the app and loaded the packed assets, but Chrome failed during WebGPU frame submission on this headless Linux host with:

```text
OperationError: A valid external Instance reference no longer exists.
```

A one-off Playwright instrumentation run narrowed that rejection to `GPUQueue.onSubmittedWorkDone()` after submitting the smoke frame. The same Deno WebGPU frame path passed and wrote the expected PNG, so this is recorded as a Chrome/headless WebGPU validation limit for this host rather than a Deno host-boundary failure.

Observed Deno output:

```json
{"ok":true,"outputPath":"/tmp/mclone-deno-vanilla-asset-world-smoke.png","width":128,"height":128,"format":"rgba8unorm","atlasWidth":32,"atlasHeight":16,"mipLevel":0,"stoneTexture":"minecraft:block/stone","stoneSprite":{"u0":0,"u1":0.5,"v0":0,"v1":1},"dirtySectionCount":25,"renderedChunkCount":1,"solidDrawCount":1,"nonClearPixels":2916,"centerPixel":[93,93,93,255],"byteLength":65536,"workerCounters":{"ingestBatchCount":1,"meshBuildRequestCount":3,"meshNotReadyResponseCount":2,"meshCompletionCount":1},"adapter":{}}
```

## Done When

- Done: browser renderer setup gets WebGPU context, canvas target, and render-world worker endpoint through a host object.
- Done: Deno vanilla-asset smoke gets WebGPU context, offscreen target, and render-world worker endpoint through a host object.
- Done: Deno visual output remains unchanged.
- Done: browser smoke has an explicit port override for this multi-dev-server host.

## Follow-Up

Next tactical: `RendererHost2` should extract the duplicated static-world/offscreen-frame orchestration into a shared harness with injectable host, scene, and camera/input state. The first target should keep the current Deno vanilla-asset smoke behavior but route frame setup through shared source code that a browser probe can also call.
