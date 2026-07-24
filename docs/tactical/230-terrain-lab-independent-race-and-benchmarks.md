# Terrain Lab Independent Race And Benchmarks

Status: active implementation 2026-07-24.

Topic: `gpu-procedural-terrain`

## Objective

Make Terrain Lab comparison visibly and measurably asynchronous. CPU and GPU
terrain must stop publishing as one coupled result, cache reuse must be
optional and explicit, and a repeatable cold stress run must exercise enough
work to reveal progressive readiness.

This tactical also fixes the map interaction defects found during hosted use:
trackpad/wheel zoom must not scroll the document, and vertical grab-to-pan must
follow the pointer like horizontal grab-to-pan.

## Originating Direction

The first viewport-driven slice made CPU/GPU comparison easy to inspect, but
its scheduler still compiled each CPU reference tile before encoding the
corresponding GPU tile. It then published one shared complete level. That
guaranteed aligned comparison but made both panels appear simultaneously and
prevented the visual result from communicating their different readiness.

Auto detail also intentionally keeps screen-space sample density roughly
constant. Zooming from kilometers to tens of kilometers therefore selects a
coarser spacing rather than creating a substantially larger workload.
Additional geographic zoom alone is not a useful throughput benchmark.

## Product Contract

### Input

- Install the terrain-stage `wheel` listener directly with
  `{ passive: false }`; React's passive delegated wheel listener cannot cancel
  Mac trackpad document scrolling.
- Cancel the native wheel event before applying cursor-anchored terrain zoom.
- Map drag uses grab semantics on both axes: the terrain under the pointer
  follows the pointer.
- Keep `touch-action: none` for direct touch and pointer gestures.

### Independent Readiness

- CPU reference compilation and GPU dispatch have independent queues.
- Creating or dispatching a GPU tile does not require its CPU reference grid.
- CPU and GPU each track and publish their own finest complete level.
- Compare may therefore show a coarse or target level on one panel while the
  other panel remains blank or at a coarser parent.
- The two panels retain identical seed, coordinates, requested detail, camera,
  and layer. Independent timing must not weaken comparison identity.
- Error comparison remains pending until both target-level sample sets are
  complete.

GPU readiness for this browser experiment means the compute output has
completed its existing validation readback. This is an end-to-end
dispatch-to-readable-result boundary, not a timestamp-query measurement of
GPU execution alone.

### Cache

The default remains one session-local bounded LRU. Stable identity is:

```text
field/evaluator revision + seed + aligned origin + sample spacing
```

The renderer retains CPU reference samples, uploaded reference buffers,
GPU-computed sample buffers, and completed validation readbacks. Camera,
presentation layer, and source selection are not generation identity.

The UI exposes:

- `Cache on`, retaining reusable tiles while panning and zooming;
- `Cache off`, discarding resident tiles whenever the generation viewport
  changes while retaining the active request until it is replaced; and
- `Clear and rerun`, invalidating current residency even when the viewport is
  unchanged.

Disabling cache also disables the speculative one-tile preload margin. It does
not repeatedly throw away the active request during camera-only orbit or layer
changes.

### Cold Benchmark

The Lab provides two repeatable actions:

1. cold-run the current viewport with its current detail; and
2. run a visible stress preset with cache disabled, Compare map view, a
   deterministic fixed footprint, and a larger diagnostic tile-axis budget.

The report distinguishes:

- CPU first coarse and target readiness;
- GPU first coarse and target readiness;
- CPU samples and total compile wall time;
- GPU samples and dispatch-to-readback target wall time;
- CPU and GPU target throughput derived only from those labeled boundaries;
- cache hits/resident tiles; and
- queue, stale, and eviction counts.

The benchmark must not label encode/submit or readback completion as GPU
execution time. A later timestamp-query path may add that separate fact.

## Ownership

- `mclone-terrain-view` owns independent queues, tile readiness, publication,
  residency policy, cache invalidation, and frame facts.
- `mclone-terrain-lab` owns browser monotonic timing and the narrow benchmark
  report facade.
- `tools/terrain-lab` owns native event capture, cache/benchmark controls,
  responsive status, and test interaction.

## Bounded Slice

This tactical includes:

- non-passive wheel cancellation and corrected map vertical drag;
- independent CPU/GPU scheduling and level publication;
- per-panel readiness labels and diagnostics;
- cache on/off and clear-and-rerun controls;
- current-view and deterministic stress benchmark actions;
- state, Rust, Wasm, headed desktop/mobile, and rendered-pixel validation; and
- hosted deployment and receipt.

It does not include:

- GPU timestamp queries where the adapter does not expose them;
- browser CPU Web Worker migration;
- mixed-level seams or individual child-tile publication within one panel;
- zoom beyond the current geographic limit;
- scale-aware band limiting or far-field summarization; or
- production in-game LOD integration.

## Stop Conditions

Stop for review only if:

- independent buffers cannot preserve exact comparison identity;
- the stress footprint exceeds the existing portable WebGPU resource limits;
- cancellation leaves stale tiles publishable in either panel; or
- native wheel cancellation conflicts with pinch/pointer behavior on the
  tested phone viewport.
