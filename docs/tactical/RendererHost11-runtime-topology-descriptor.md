# RendererHost11: Runtime Topology Descriptor

Status: implemented for generated-world browser and Deno boot, including Deno worker-backed lighting declaration.

## Goal

Make the browser and Deno generated-world harnesses declare the runtime topology they actually provide before booting a scene. Shared smoke validation should compare that declaration against the scenario requirements so host drift is visible immediately instead of being inferred from scripts or docs.

This slice is about assurance, not adding another rendering path. Browser and Deno should still differ only at host adapters: canvas vs offscreen target, browser asset pack vs file asset pack, DOM/browser storage vs headless storage choices, and browser worker construction vs Deno module workers.

## Scope

| Area | Result |
|---|---|
| topology model | `GeneratedWorldRuntimeTopology` records host, world host carrier, render-world worker, mesh transport, lighting placement, liquid mode, storage, asset source, and render target |
| boot adapter contract | `GeneratedWorldBootAdapter.describeTopology(...)` is required before scene creation |
| boot validation | `runGeneratedWorldBoot(...)` rejects mismatched topology before GPU/world setup |
| smoke result | generated-world top-level and per-step results carry the declared topology |
| scenario validation | `validateGeneratedWorldSmokeResult(...)` requires topology and checks it against scenario engine requirements |
| browser adapter | declares browser canvas, browser asset pack, local worker or remote authority, storage, and lighting placement |
| Deno adapter | declares Deno, worker world host, offscreen target, file asset pack, no storage, and `none` or worker-backed lighting placement by scenario |
| Deno lighting | Deno generated-world host worker creates a worker-backed `LightingService` when `lightingMode: "vanilla17"` is requested |

## Design Notes

Topology is intentionally a small value object, not a second lifecycle framework:

```ts
interface GeneratedWorldRuntimeTopology {
  host: "browser" | "deno" | "native" | "node" | "test";
  worldHost: "worker" | "remote" | "in-process";
  renderWorld: "worker";
  meshTransport: "worker";
  lighting: "none" | "worker" | "remote";
  liquidSimulation: "vanilla17" | "none";
  storage: "indexeddb" | "file" | "memory" | "none" | "remote" | "default";
  assetSource: "browser-asset-pack" | "file-asset-pack" | "test";
  renderTarget: "canvas" | "offscreen-texture";
}
```

Scenario requirements are derived from the scenario engine config and requested world transport. Current generated-world smokes require:

- render-world ownership in a worker
- mesh transport through the worker path
- lighting `none` for the current scenario set
- the requested liquid simulation mode
- local worker or remote world host matching the requested transport

For a `lightingMode: "vanilla17"` worker scenario, the same validation requires `lighting: "worker"`. The Deno generated-world smoke now includes a small vanilla-lighting scenario that exercises this topology through a Deno module lighting worker.

## Non-Goals

- Change browser or Deno frame orchestration.
- Add a native window/wgpu host.
- Treat topology declarations as proof of remote server internals. A remote browser client can only declare that authority is remote; dedicated host internals need their own server-side topology checks later.

## Validation

- Focused unit tests cover browser topology declaration, headless worker-lighting declaration, missing-topology result rejection, and pre-scene boot rejection for a Deno-style adapter that cannot satisfy `vanilla17` lighting.
- Existing browser and Deno smokes now surface topology in their normalized result payloads.
- Deno generated-world smoke includes the default, transition, tick-cadence, and vanilla-lighting scenarios.

## Follow-Up

Run the same `vanilla-lighting` scenario through the browser worker smoke path on a Chrome/WebGPU host and compare its topology/result shape with Deno. After that, add server-side topology reporting for the dedicated remote host so `lighting: "remote"` carries inspectable authority details instead of only saying the browser client joined a remote authority.
