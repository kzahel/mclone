# RendererHost4: Shared Generated World Scenarios

Status: done.

## Goal

Move the hardcoded generated-world smoke constants and result shape into shared source so browser probes and Deno headless smokes describe the same scenario. Browser and Deno should differ by host adapters and render target, not by separately maintained camera, chunk-interest, expected-count, and assertion logic.

This slice should make the Deno generated-world smoke a first-class harness peer of browser smoke without requiring Chrome on this headless Linux host.

Landed in `src/renderer/generated-world-smoke-scenario.ts`.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | shared generated-world scenario module | defines seed, preset, view/render distance, camera, engine config, expected loaded chunk count, output naming, injected player input, and readback/player/counter assertions |
| 2 | `scripts/deno-generated-world-smoke.ts` | consumes the shared scenario instead of owning its own camera/config/result constants |
| 3 | browser smoke helper | consumes the same scenario inputs where the browser path builds URL params and asserts boot results |
| 4 | result normalization | source-level helper validates comparable counters, readback pixels, loaded chunks, and authoritative player-input acknowledgement for browser and Deno lanes |
| 5 | docs | record which fields are intentionally host-specific and which are scenario-owned |

Do not add:

- new rendering features
- native windowing
- persistent Deno storage
- Chrome workarounds for this headless host
- a second generated-world scenario until the first one is shared cleanly

## Design Notes

The intended shape is:

```text
shared generated-world scenario
  -> browser URL / canvas target adapter
  -> Deno headless host / offscreen target adapter
  -> comparable smoke result
```

Keep host-specific pieces explicit:

- browser owns DOM canvas, URL/query parsing, Playwright screenshot capture, and browser asset-pack fetch
- Deno owns file asset-pack reads, Deno module workers, offscreen texture target, and `/tmp` PNG writing
- shared scenario owns camera, view radius, expected loaded chunks, engine config, injected player input, and result assertions

The first scenario intentionally uses `viewDistance=1` for both browser smoke and Deno smoke so the shared lane is fast enough for this headless Linux host while still exercising chunk loading, render-world worker mesh builds, GPU uploads, draw submission, readback, and host-authoritative player input.

## Validation

Completed:

- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts`
- `pnpm typecheck` still fails only on the known unrelated `test/runtime/generated-world-host-factory.test.ts(43,43)` `WorldHostMessage.snapshot` access.
- `git diff --check`

Browser smoke was not rerun on this host because headless Chrome WebGPU is not usable here; that validation belongs on a host with a working Chrome GPU path.

## Done When

- Deno generated-world smoke and browser smoke inputs come from one shared scenario definition.
- Deno output and browser boot result are normalized enough to compare core counters, player input acknowledgement, readback pixels, and expected loaded-chunk behavior.
- Scenario-owned assertions are not duplicated across test scripts.
- Existing Deno generated-world and static asset lanes still pass.

## Follow-Up

Next: [`RendererHost5-host-neutral-generated-world-runner.md`](RendererHost5-host-neutral-generated-world-runner.md). The scenario is shared now, but browser and Deno still have parallel frame orchestration; the next slice should extract the generated-world scenario runner itself so host adapters provide only scene creation, targets, workers, assets, and screenshots/PNG writing.
