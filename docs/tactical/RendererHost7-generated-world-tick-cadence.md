# RendererHost7: Generated World Tick Cadence

Status: implemented for shared generated-world smoke scenarios.

## Goal

Extend the host-neutral generated-world smoke contract from camera/chunk-interest transitions to a small real-time cadence scenario. Browser and Deno should both prove that repeated player input commands, runtime update drains, settled frame presentation, and per-frame readbacks use the same renderer/runtime API shape.

## Scope

| Area | Result |
|---|---|
| scenario contract | `GeneratedWorldSmokeScenario.cadence` describes a fixed presentation interval and progression requirements; each step may carry a `frameIndex` and readback artifact |
| tick-cadence scenario | `GENERATED_WORLD_TICK_CADENCE_SCENARIO` runs four static-camera frames with input sequences `1..4` and `/tmp` PNG outputs |
| shared runner | the normal generated-world runner waits/drains the cadence interval before presenting each step and reports `frameIndex` / `presentationDelayMs` in the normalized result |
| validation | shared validation checks per-step input sequence, player state revision, player tick, and player position progression |
| Deno smoke | `scripts/deno-generated-world-smoke.ts` now runs static, transition, and tick-cadence scenarios through the same headless harness |
| browser smoke | Playwright consumes `generatedWorldScenario=tick-cadence` through the same browser boot path and writes `/tmp/mclone-browser-worker-tick-cadence-smoke.png` |

## Validation

- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- `pnpm test:browser`
- `pnpm test -- test/renderer/generated-world-smoke-scenario.test.ts`
- `pnpm typecheck`
- `git diff --check`

## Acceptance

- Static generated-world smoke remains a one-step scenario.
- Transition smoke still moves chunk interest between distinct centers.
- Tick-cadence smoke keeps chunk interest stable while advancing input sequence, player tick, player state revision, and player position across four frames.
- Deno writes all cadence checkpoint PNGs under `/tmp`.
- Browser writes the final cadence screenshot under `/tmp`.

## Next

RendererHost8 should turn the smoke-only cadence into a reusable host presentation loop contract: one adapter shape for frame timing, target acquisition, screenshot/readback artifacts, and cleanup that browser, Deno headless, and a future native/wgpu host can implement without duplicating lifecycle code.
