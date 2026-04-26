# RendererHost6: Generated World Transition Scenarios

Status: done.

## Goal

Extend the shared generated-world scenario contract from one static frame to a small sequence of host-neutral steps. The next useful parity boundary is not another still image; it is proving that browser and Deno can both inject input, move camera/chunk interest, drain runtime updates, render again, and report comparable results through the same runner API.

This is the bridge from "headless can render a generated frame" to "headless can drive the same engine harness shape as browser probes."

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | scenario step model | supports named steps for camera state, expected chunk center/count, injected player input, and per-step readback expectations |
| 2 | generated-world runner | runs one or more scenario steps, preserving the existing single-frame result shape for the default smoke |
| 3 | Deno generated-world smoke | executes the default single-frame scenario plus one transition scenario and writes `/tmp` PNG artifacts for the visible frames |
| 4 | browser smoke/probe path | can consume the same step model without browser-only control flow for input/chunk-interest transitions |
| 5 | result validation | reports per-step loaded chunk counts, render-world counters, queue settle state, player-input acknowledgement, and sampled pixels |

Do not add:

- new movement physics
- prediction/reconciliation changes
- persistent Deno storage
- native windowing
- broad probe-suite rewrites

## Design Notes

Keep the API shape small:

```text
GeneratedWorldScenario
  -> steps[]
      -> camera
      -> optional player input
      -> expected loaded chunk count / chunk center
      -> optional readback artifact
```

The runner should own sequencing and validation. Host adapters should remain limited to scene creation, render target/readback, artifact writing, and worker/asset/platform capabilities.

The first transition should be conservative: move the camera enough to change chunk interest and render a second settled frame, then inject a later input sequence and verify the authoritative player acknowledgement advances. Avoid adding gameplay semantics beyond what the current runtime already supports.

Landed shape:

- `GeneratedWorldSmokeScenario.steps[]` now carries the host-neutral step contract: name, camera, optional player input, expected loaded chunk count/center, and readback artifact expectations.
- `runGeneratedWorldSmokeScenario(...)` sequences every step: set chunk interest, wait for the loaded ring, verify the session chunk view, inject step input, render a settled frame, read back pixels, and report per-step counters/state.
- The top-level smoke result remains the final step result plus the existing single-frame fields, with a `steps` array for transition comparisons.
- Deno runs the original static smoke and the two-step transition scenario in one smoke command, writing step PNGs under `/tmp`.
- Browser smoke can select the same transition scenario through `generatedWorldScenario=transition`; the worker-integrated browser lane now exercises it.

## Validation

Completed:

- `pnpm smoke:deno:generated-world` - passed; wrote `/tmp/mclone-deno-generated-world-smoke.png`, `/tmp/mclone-deno-generated-world-transition-01-initial.png`, and `/tmp/mclone-deno-generated-world-transition-02-shifted.png`
- `pnpm smoke:deno:world-assets` - passed
- `pnpm test -- test/renderer/generated-world-smoke-scenario.test.ts` - passed
- `env VITE_PORT=35173 pnpm test:browser` - passed on Mac Chrome/WebGPU with remote, worker, and worker transition screenshots under `/tmp`
- `pnpm typecheck` - passed
- `git diff --check`

Run browser smoke only on a host with usable Chrome WebGPU. On this Mac, use an unused `VITE_PORT` if another local app is already listening on the inherited port; Playwright has `reuseExistingServer` enabled outside CI.

## Done When

- A transition scenario can be run by the same generated-world runner in Deno and by the browser path.
- Per-step results prove chunk-interest movement, queue settling, readback pixels, and authoritative player-input acknowledgement.
- The single-frame generated-world smoke remains a thin wrapper around the same runner.
- Deno writes transition artifacts under `/tmp` and the screenshot is visually inspected.

## Follow-Up

After transition scenarios are shared, decide whether the next host contract should cover real-time input sampling/prediction hooks or a native-window/wgpu spike adapter.
