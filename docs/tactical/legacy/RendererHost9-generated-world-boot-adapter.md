# RendererHost9: Generated World Boot Adapter

Status: implemented for browser and Deno generated-world smoke boot.

## Goal

Move generated-world smoke boot orchestration behind a host-neutral adapter shape. `RendererHost8` shared presentation and artifact handling; this slice shares the higher-level sequence: resolve scenario inputs, create a renderer scene through a host adapter, create a presentation host, run the shared generated-world scenario runner, and return the scene/run/camera lifecycle to the caller.

## Scope

| Area | Result |
|---|---|
| shared boot | `runGeneratedWorldBoot(...)` owns scene creation, presentation-host creation, scenario execution, failure cleanup, and runtime camera resolution |
| browser adapter | `createGeneratedWorldBrowserBootAdapter(...)` supplies browser scene creation, render config, world transport, optional GUI probe overlay, and canvas readback presentation |
| headless adapter | `createGeneratedWorldHeadlessBootAdapter(...)` supplies asset pack, world transport, render-worker factory, offscreen presentation, and PNG artifact writing |
| browser entrypoint | `main.ts` keeps URL/storage/title-screen policy and delegates generated-world smoke boot to the shared boot function |
| Deno smoke | `scripts/deno-generated-world-smoke.ts` now supplies only Deno workers, asset pack, WebGPU context, and artifact callback |
| tests | focused boot tests cover camera resolution and one-step browser camera override behavior |

## Acceptance

- Browser and Deno generated-world smokes call the same shared boot orchestrator before reaching the shared scenario runner.
- Browser-only IndexedDB clearing and URL parsing remain in `main.ts`.
- Deno-only Worker construction, asset-pack source, and PNG encoding remain in the Deno script.
- The boot result keeps the scene alive for the browser live-world path and exposes a close hook for headless callers.

## Validation

- `pnpm test -- test/renderer/generated-world-boot.test.ts test/renderer/generated-world-smoke-runner.test.ts test/renderer/generated-world-smoke-scenario.test.ts`
- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- `pnpm test:browser`
- `pnpm typecheck`
- `git diff --check`

## Next

RendererHost10 should finish lifecycle parity by introducing explicit scene/runtime disposal for browser smoke and live-world paths. Deno already closes the boot result; browser should expose the same close/dispose capability for tests, page transitions, and future native/wgpu hosts.
