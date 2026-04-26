# RendererHost4: Shared Generated World Scenarios

Status: next.

## Goal

Move the hardcoded generated-world smoke constants and result shape into shared source so browser probes and Deno headless smokes describe the same scenario. Browser and Deno should differ by host adapters and render target, not by separately maintained camera, chunk-interest, expected-count, and assertion logic.

This slice should make the Deno generated-world smoke a first-class harness peer of browser smoke without requiring Chrome on this headless Linux host.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | shared generated-world scenario module | defines seed, preset, view/render distance, camera, engine config, expected loaded chunk count, output naming, and pixel/readback assertions |
| 2 | `scripts/deno-generated-world-smoke.ts` | consumes the shared scenario instead of owning its own camera/config/result constants |
| 3 | browser smoke/probe helper | consumes the same scenario inputs where the browser path currently builds URL params and asserts boot results |
| 4 | result normalization | source-level helper produces comparable counters/pixels/result summaries for browser and Deno lanes |
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
- shared scenario owns camera, view radius, expected loaded chunks, engine config, and result assertions

## Validation

Required for completion:

- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- focused renderer/runtime tests touched by scenario extraction
- `pnpm typecheck`, with the known unrelated `WorldHostMessage.snapshot` failure documented if still present
- `git diff --check`

Run browser smoke only on a host with usable Chrome WebGPU. On this host, record the expected Chrome headless `GPUQueue.onSubmittedWorkDone()` failure if browser validation is attempted.

## Done When

- Deno generated-world smoke and browser smoke/probe inputs come from one shared scenario definition.
- Deno output and browser boot result are normalized enough to compare core counters and expected loaded-chunk behavior.
- Scenario-owned assertions are not duplicated across test scripts.
- Existing Deno generated-world and static asset lanes still pass.

## Follow-Up

After this, add a second generated-world scenario only if it exercises a genuinely different host/runtime boundary, such as injected player input/movement or a different camera/chunk-interest transition.
