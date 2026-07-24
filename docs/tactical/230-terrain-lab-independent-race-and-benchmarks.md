# Terrain Lab Independent Race And Benchmarks

Status: complete 2026-07-24, including local and hosted desktop/mobile
headed-WebGPU validation.

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

## Local Validation Receipt

The implementation landed through commits `93ce5851` and `67c32a05`.

Validated locally:

- `cargo test -p mclone-worldgen terrain_preview --lib`;
- `cargo test -p mclone-terrain-view --lib`;
- `cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown`;
- scoped Wasm clippy with the pre-existing `manual_is_multiple_of` lint
  permitted;
- Terrain Lab state tests and TypeScript/Wasm typecheck;
- headed-Wayland desktop and phone Playwright interaction flows;
- dedicated headed-Wayland desktop and mobile BrowserWebGPU smokes; and
- inspected initial, map/error, continent-scale, and cold-race captures.

The interaction flow proves that wheel zoom changes the terrain URL while
document `scrollY` remains unchanged and that downward map grab decreases
world Z, matching horizontal grab semantics.

The fixed cold race uses seed `-98765`, center `(-304, 336)`, a 2 km Compare
map, requested `1:2`, cache off, zero cache hits, and a 12-tile-per-axis
diagnostic budget. The observed target-readiness sequence was:

```text
neither ready -> GPU ready -> both ready
```

The desktop run selected effective `1:8`, with 35 visible target tiles and 51
total nested-level tiles:

- GPU target plus validation readback: 287.9 ms;
- CPU target publication: 2,595.5 ms;
- GPU end-to-end target rate: 748.4 K samples/s; and
- CPU end-to-end target rate: 83.0 K samples/s.

The phone run selected effective `1:4`, with 81 visible target tiles and 119
total nested-level tiles:

- GPU target plus validation readback: 665.6 ms;
- CPU target publication: 1,881.3 ms;
- GPU end-to-end target rate: 755.4 K samples/s; and
- CPU end-to-end target rate: 267.2 K samples/s.

Both settled with empty queues, effectively zero base mean/P95 height error,
100% ocean agreement, and matching inspected CPU/GPU terrain. These are
end-to-end browser results for this fixed workload, not portable GPU execution
timings or a general promise of the same ratio on every adapter.

## Hosted Validation Receipt

A targeted Terrain Lab upload avoided publishing unrelated dirty applications.
The existing Worker version
`5575a252-3d41-48f9-b308-fa42f65220f7` was unchanged. The production route
serves:

- JavaScript `index-BcjjqS2k.js`;
- stylesheet `index-Jbtyl1cl.css`; and
- Wasm `mclone_terrain_lab_bg-BZKifX35.wasm`.

The HTML switched only after those immutable objects were available. Direct
requests confirmed the expected content types and immutable one-year asset
cache policy.

Dedicated hosted headed-Wayland BrowserWebGPU smokes then exercised the
desktop and Pixel 7-sized layouts at
`https://mclone.kzahel.com/terrain/`. Both observed:

```text
neither target ready -> GPU target ready -> both targets ready
```

The hosted desktop fixed race selected effective `1:8`, with 35 visible
target tiles and 51 total nested-level tiles:

- GPU target plus validation readback: 278.2 ms;
- CPU target publication: 2,187.3 ms;
- GPU end-to-end target rate: 774.5 K samples/s; and
- CPU end-to-end target rate: 98.5 K samples/s.

The hosted phone fixed race selected effective `1:4`, with 81 visible target
tiles and 119 total nested-level tiles:

- GPU target plus validation readback: 686.4 ms;
- CPU target publication: 1,918.9 ms;
- GPU end-to-end target rate: 732.5 K samples/s; and
- CPU end-to-end target rate: 262.0 K samples/s.

Both ran with cache disabled, recorded zero cache hits, drained both queues,
retained effectively zero base mean/P95 error, and had 100% ocean agreement.
The inspected final desktop side-by-side and phone stacked captures showed
the same coordinate-locked geography in both panels. As above, GPU target
time ends at validation readback; it is not a pure timestamp-query execution
duration.
