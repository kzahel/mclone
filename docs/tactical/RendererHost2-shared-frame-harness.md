# RendererHost2: Shared Frame Harness

Status: next.

## Goal

Replace the duplicated Deno smoke-frame orchestration with a source-level harness that can run against either a browser canvas host or a headless offscreen host. The harness should keep the same engine-facing API shapes for scene setup, camera/input injection, render-world worker lifecycle, frame encoding, readback, and artifact writing.

The purpose is parity of harness shape, not feature expansion. Deno should remain the first non-browser validation lane, but the code it exercises should be the same frame path a browser probe can call.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | shared renderer frame harness module | owns render-world worker client setup, static snapshot ingestion, chunk dispatcher/view-area setup, level/game renderer setup, frame encode, and readback hooks behind injectable host/scene inputs |
| 2 | `scripts/deno-vanilla-asset-world-smoke.ts` | becomes a thin Deno entry point that supplies host adapters, vanilla asset inputs, camera, static chunks, and `/tmp` output path |
| 3 | browser probe or smoke helper | proves the same harness can target a browser canvas path without depending on Deno-specific APIs |
| 4 | docs | record the shared harness contract and the next boundary after frame orchestration |

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

The harness should own lifecycle cleanup for render-world workers and GPU resources it creates. Browser-specific screenshot capture and Deno-specific PNG writing should stay in entry points; the shared harness should return pixels, counters, draw stats, and frame metadata in a host-neutral result.

## Validation

Required for completion:

- `pnpm smoke:deno:world-assets`
- inspect `/tmp/mclone-deno-vanilla-asset-world-smoke.png`
- focused renderer worker/chunk tests touched by the refactor
- `git diff --check`

Run `pnpm test:browser` when a working Chrome/WebGPU environment is available. On this current headless Linux host, record the known Chrome `GPUQueue.onSubmittedWorkDone()` failure if it still blocks browser validation after the shared harness refactor.

## Done When

- Deno vanilla-asset smoke is mostly host/input wiring around a shared harness.
- Browser code can call the same harness shape for a canvas-backed frame or probe.
- The shared result includes enough counters and pixel/readback metadata to preserve the existing Deno assertions.
- The Deno PNG remains visually unchanged.

## Follow-Up

After this, the next slice should move from static scene parity to generated-world scene parity: a headless harness input that uses the same `ClientRuntime` / integrated-server path as browser smoke, with injected input/camera state and no browser-only assumptions.
