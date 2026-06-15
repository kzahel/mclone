# RendererHost5: Host-Neutral Generated World Runner

Status: done.

## Goal

Remove the remaining split between the browser generated-world smoke flow and the Deno generated-world smoke flow. `RendererHost4` made the scenario contract shared; this slice should make the scenario runner shared too, so host adapters supply capabilities while one source-level runner owns the order of operations.

The desired shape is:

```text
GeneratedWorldScenarioRunner
  -> host scene factory
  -> host render/readback target
  -> host artifact writer
  -> shared normalized result
```

This keeps Deno from becoming a one-off smoke shim and moves toward the long-term browser/headless/native host surface.

Landed in `src/renderer/generated-world-smoke-runner.ts`.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | shared generated-world runner | owns chunk-interest setup, loaded-ring wait, render settle, shader validation, player-input injection, readback normalization, and normalized result construction |
| 2 | Deno generated-world smoke | is now a thin Deno host adapter: WebGPU context, filesystem asset pack, module worker factories, offscreen target, `/tmp` PNG write |
| 3 | browser smoke entrypoint | delegates scenario execution to the same runner after browser scene creation and keeps browser-only URL/canvas/Playwright concerns at the edge |
| 4 | result contract | explicitly separates host-specific fields (`adapterInfo`, browser canvas format, output path) from scenario-owned fields (camera, counts, input, counters, pixels) |
| 5 | docs/tests | document the host adapter API shape and keep Deno generated-world smoke plus focused renderer/runtime tests green |

Do not add:

- a second scenario
- native windowing
- persistent Deno storage
- new movement or prediction behavior
- browser WebGPU workarounds for this headless Linux host

## Design Notes

The runner takes an already-created `RendererScene`, a `GeneratedWorldSmokeScenario`, and a host render target. Host-specific setup stays explicit:

- browser: DOM canvas, URL/query parsing, IndexedDB clearing, remote/worker transport selection, canvas screenshot
- Deno: file-backed asset pack, Deno workers, headless WebGPU device/target, PNG file write
- shared runner: chunk view request, render-world dirty-section draining, loaded-ring wait, settled frame render, player input command, shader/pipeline validation, submission validation, readback pixel sampling/counting, normalized result

The important boundary is API shape, not a new abstraction layer for its own sake. Prefer a small function and small option interfaces over a framework.

## Validation

Completed:

- `pnpm smoke:deno:generated-world`
- `pnpm smoke:deno:world-assets`
- `pnpm test -- test/renderer/texture/texture-atlas-plumbing.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts`
- `pnpm typecheck` still fails only on the known unrelated `test/runtime/generated-world-host-factory.test.ts(43,43)` `WorldHostMessage.snapshot` access.
- `git diff --check`

Browser smoke was not rerun on this host because headless Chrome WebGPU is not usable here; this slice is validated through the Deno WebGPU lane and source-level browser compile/type coverage.

## Done When

- Browser and Deno generated-world lanes call one shared runner for scenario execution order.
- Deno-specific and browser-specific code is limited to host capability adapters and artifact plumbing.
- The normalized result can be compared across hosts without duplicating assertions in scripts/tests.
- The Deno PNG still shows the expected generated terrain and authoritative player input is acknowledged in the result.

## Follow-Up

Next: [`RendererHost6-generated-world-transition-scenarios.md`](RendererHost6-generated-world-transition-scenarios.md). Add multi-step scenario support for camera/chunk-interest transitions and injected input sequences so headless/browser harnesses exercise more than a single static frame.
