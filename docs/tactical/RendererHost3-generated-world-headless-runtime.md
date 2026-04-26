# RendererHost3: Generated World Headless Runtime

Status: next.

## Goal

Move the Deno headless renderer from static chunk snapshots to the same generated-world runtime path used by browser smoke. The host should inject camera/input state and render to an offscreen target, but chunk generation, client-world hydration, render-world ingestion, and mesh compilation should flow through the shared `ClientRuntime` / integrated-server shape instead of a Deno-only static snapshot shortcut.

This is the first slice that should prove the headless lane can exercise real engine harness behavior without depending on Chrome.

## Scope

Add or update:

| # | Module | Expected result |
|---|---|---|
| 1 | generated-world headless harness entry | Deno can boot the generated-world worker/integrated-server path, set chunk interest from an injected camera, wait for the loaded chunk ring, render one settled frame, and write `/tmp` PNG output |
| 2 | shared runtime/renderer helper | reuse browser `ClientRuntime` and renderer setup concepts without requiring DOM canvas, browser storage, or Chrome-only APIs |
| 3 | input/camera injection | represent the smoke camera and minimal command input as host-provided data instead of browser event state |
| 4 | validation result | return loaded chunk count, render-world counters, draw counts, queue stats, center/terrain pixels, and output path in the same spirit as browser smoke |
| 5 | docs | record which browser assumptions remain and which host adapters are now shared |

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

Required for completion:

- generated-world Deno smoke command, added to `package.json`
- inspect generated-world PNG under `/tmp`
- focused runtime/renderer tests touched by helper extraction
- `pnpm smoke:deno:world-assets` to prove the static harness still works
- `pnpm typecheck`, with the known unrelated `WorldHostMessage.snapshot` failure documented if still present
- `git diff --check`

Browser smoke can remain documented as unavailable on this headless Linux host because Chrome headless WebGPU fails at `GPUQueue.onSubmittedWorkDone()`. Do not block this Deno slice on Chrome here.

## Done When

- Deno renders a generated-world frame through shared runtime/client/render-world setup.
- The camera/chunk-interest input is injected by the host instead of read from DOM events.
- The output PNG and counters prove real generated chunks reached mesh upload and draw submission.
- Static Deno smokes continue to pass through `src/renderer/static-frame-harness.ts`.

## Follow-Up

After this, move toward reusable harness parity for browser probes and headless Deno generated-world probes: one set of scenario inputs, two target adapters.
