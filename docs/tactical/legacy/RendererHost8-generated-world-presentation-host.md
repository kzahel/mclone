# RendererHost8: Generated World Presentation Host

Status: implemented for generated-world smoke presentation.

## Goal

Extract the smoke-only presentation details behind a host adapter so browser, Deno headless, and future native/wgpu hosts share the same scenario runner contract. The runner should continue to own generated-world step order; hosts should own target acquisition, frame cadence waits, readback presentation, artifact writing, and cleanup.

## Scope

| Area | Result |
|---|---|
| shared contract | `GeneratedWorldSmokePresentationHost` now wraps the render target plus optional presentation-delay, step-artifact, and close hooks |
| runner | `runGeneratedWorldSmokeScenario` consumes a presentation host, validates the host target format, waits cadence through the host hook, writes per-step artifacts through the host hook, and closes the host at run end |
| browser adapter | `createGeneratedWorldBrowserPresentationHost(...)` owns canvas presentation and readback texture rendering, including optional GUI probe overlay composition |
| headless adapter | `createGeneratedWorldHeadlessPresentationHost(...)` owns offscreen target readback and optional PNG artifact writing without importing Deno APIs |
| Deno smoke | the generated-world Deno script passes a headless presentation host instead of inline target construction plus a separate artifact loop |
| tests | focused runner tests cover delay resolution and presentation-host hook wiring |

## Acceptance

- Static, transition, and tick-cadence scenarios still run through the same shared runner.
- Deno still writes all generated-world PNG artifacts under `/tmp`.
- Browser still presents to the canvas and returns normalized readback data for smoke validation.
- Browser-specific canvas/GPU readback code is outside `main.ts`.
- Deno-specific filesystem writes stay in the Deno script; source modules receive host-neutral callbacks.

## Validation

- `pnpm test -- test/renderer/generated-world-smoke-runner.test.ts test/renderer/generated-world-smoke-scenario.test.ts`
- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- `pnpm test:browser`
- `pnpm typecheck`
- `git diff --check`

## Next

RendererHost9 should pull scene creation into the same host-neutral generated-world boot shape: browser, Deno, and native/wgpu should supply device context, asset pack, world transport, worker factories, and presentation host through one boot adapter instead of keeping separate browser and Deno setup functions.
