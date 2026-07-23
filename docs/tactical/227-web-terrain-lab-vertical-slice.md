# Web Terrain Lab Vertical Slice

Status: active implementation as of 2026-07-23.

Topic: `gpu-procedural-terrain`

## Objective

Ship the first useful Terrain Lab as a standalone browser product under
`/terrain/`. A visitor on a desktop or phone should be able to enter a seed,
choose a location and scale, and see first-party Mclone terrain generated live
without starting a game session or materializing canonical chunks.

The vertical slice proves:

1. a shared CPU reference-grid contract derived from the production
   `McloneOverworldSampler`;
2. one presentation-only GPU compute evaluator and resident surface tile;
3. map, three-dimensional, reference, GPU, and split comparison views;
4. URL-addressed seed, location, spacing, layer, and view state;
5. visible CPU/GPU difference and timing diagnostics;
6. a dedicated Rust/WASM payload rather than the complete game client; and
7. aggregate deployment, desktop browser, and phone-sized browser acceptance.

This tactical combines Experiments 0 and 1 from
[`../topics/gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md).
Progressive multi-tile refinement, canonical readback, in-game Far LOD
adoption, native profiling, and volumetric terrain remain later slices.

## Originating Direction

The requested product is a much faster world-generation iteration loop: change
the first-party generator, rebuild, and inspect far more geography than a
normal render distance exposes. It should be reachable from a phone like the
existing Asset and Structure Labs, while remaining flexible enough for the
game to reuse its evaluator and tile machinery.

The first proof therefore starts with the hosted web lab. It does not build a
throwaway desktop binary first, and it does not put the entire lab UI inside
the game.

## Ownership

```text
mclone-worldgen
  production sampler + reference request/grid + semantic sample
          |
          v
mclone-terrain-view
  approximate WGSL evaluator + tile buffers + draw/comparison renderer
          |
          v
mclone-terrain-lab
  WebGPU surface/device + Rust/WASM facade
          |
          v
tools/terrain-lab
  responsive DOM controls + URL state + diagnostics presentation
```

`mclone-worldgen` remains renderer-independent. It owns:

- validated fixed-grid requests;
- source and field revision identity;
- production CPU sampling;
- water, surface, climate, and comparison facts; and
- tests for coordinate, bounds, negative-origin, and request stability.

`mclone-terrain-view` is a new small shared crate because the compute evaluator,
resident tile, reference upload, comparison readback, and render pipeline are
needed outside any one app. It owns WGPU resources but not a window, canvas,
surface, DOM, URL, or game session.

`mclone-terrain-lab` is a dedicated Rust/WASM app adapter. It owns:

- browser WebGPU instance, adapter, device, queue, canvas surface, and resize;
- conversion from browser strings/numbers into shared requests;
- reference compilation timing;
- surface acquisition, command submission, and presentation; and
- a narrow JSON diagnostic facade for the TypeScript shell.

`tools/terrain-lab` owns:

- the Vite/React product shell;
- form controls, responsive layout, touch/pointer pan and zoom;
- URL parsing, validation, history, and share links;
- capability, loading, error, and diagnostic presentation; and
- Playwright product tests.

TypeScript must not evaluate terrain, choose field bands, synthesize mesh
vertices, or claim canonical world authority.

## Reference Grid Contract

The first request is a centered square:

```rust
struct TerrainPreviewRequest {
    seed: i64,
    center_x: i32,
    center_z: i32,
    sample_spacing: u32,
    cells_per_axis: u32,
    topology: McloneOverworldSamplingTopology,
}
```

The vertical slice fixes the UI to 64 cells per axis while keeping the shared
contract validated and explicit. The grid contains 65 by 65 shared-corner
samples. Powers-of-two spacing from 2 through 1,024 blocks provide footprints
from 128 through 65,536 blocks per side.

The reference sample contains the production surface height, displayed water
height, continentalness, relief, temperature, moisture, and water state. It
does not materialize blocks, features, lighting, chunks, collision, or
persistence.

Every request must reject:

- zero or non-power-of-two spacing;
- unsupported cell counts;
- sample-count overflow;
- footprint or coordinate overflow; and
- unsupported topology.

The field revision appears in every report and URL-visible product state.

## GPU Approximation Contract

The first compute evaluator is explicitly presentation-only and revisioned
independently. Portable WGSL lacks the production sampler's `i64` and `f64`
arithmetic, so the initial kernel uses stable `u32` seed words, `f32` fields,
and a GPU-friendly hash/noise evaluator.

It must:

- dispatch one invocation per 65-by-65 sample;
- derive absolute sample coordinates from a compact uniform;
- write surface height, continentalness, climate, and water facts to a storage
  buffer;
- avoid CPU-built vertex/index buffers;
- draw the grid from `vertex_index` and the resident sample buffers;
- copy the compact result to a bounded readback buffer only for lab comparison;
- label itself approximate and never mutate canonical world data; and
- declare a stable GPU-preview revision in diagnostics.

The first kernel should preserve the broad first-party language: continents,
ocean depth, rolling relief, rugged inland mountains, climate color, water,
and highland material transitions. Exact river morphology, planned streams,
surface recipes, and structures are deliberately absent. Their disagreement
with the production reference is evidence for the next porting work, not a
reason to mislabel the evaluator exact.

## Presentation

The lab offers:

- **3D terrain:** an oblique heightfield view with slope lighting and water;
- **map:** a topographic plan view over the same GPU/reference samples;
- **GPU:** only the approximate compute result;
- **reference:** only the production CPU result;
- **split:** production reference on the left and GPU approximation on the
  right;
- **error:** height disagreement visualized directly;
- **height:** normalized elevation;
- **continentalness:** the large land/ocean field; and
- **climate:** temperature/moisture presentation.

The page must always identify which half or layer is being shown. A reference
view must not be reported as GPU generation.

Desktop controls can occupy a side rail. Phone controls collapse above or
below a full-width canvas with touch-sized inputs. Dragging pans by a fraction
of the current footprint; wheel, trackpad, or explicit buttons change the
power-of-two spacing. Interactions update the shareable URL and schedule a new
revision.

## Diagnostics

The first synchronous report includes:

- request and evaluator revisions;
- seed, center, spacing, cell/sample counts, and footprint;
- CPU reference compilation milliseconds;
- command encoding/submission milliseconds;
- CPU reference byte count, GPU resident byte count, and readback byte count;
- selected view, source, and layer; and
- adapter/backend name when available.

The asynchronous comparison report includes:

- maximum, mean, and p95 absolute surface-height error;
- water-presence agreement;
- completed request revision; and
- stale-result rejection count.

Browser timestamps around the complete render request may be shown separately,
but must not be labeled GPU execution time. Precise GPU timestamp attribution
remains a native/profile follow-up unless the admitted browser adapter exposes
it.

## Development And Deployment

The root workflow should provide:

```sh
pnpm terrain-lab:web
pnpm terrain-lab:web:build
pnpm terrain-lab:web:test
pnpm terrain-lab:wasm
```

The WASM build:

1. builds only `mclone-terrain-lab` for `wasm32-unknown-unknown`;
2. runs the pinned workspace `wasm-bindgen` version;
3. writes generated bindings into an ignored Terrain Lab directory; and
4. lets Vite content-hash the dedicated JS/WASM payload.

The Vite app uses base `/terrain/`. `scripts/deploy-native-web.sh` builds it
and stages the exact output into `dist-native-web/terrain/` beside
`/animals/` and `/structures/`.

The local development command must watch the web shell. A later optimization
may add shader-only or automatic Cargo/WASM rebuilds; this slice records
cold/incremental build and first-redraw evidence before promising a particular
latency.

## Validation

Shared Rust:

- request validation and negative-coordinate footprint tests;
- exact reference-grid equality with direct production samples;
- stable source/evaluator revision tests;
- CPU comparison-statistic tests;
- WGSL parse and entry-point tests; and
- native host compilation of the renderer crate.

WASM and web:

- dedicated crate check for `wasm32-unknown-unknown`;
- TypeScript typecheck;
- production Vite build;
- aggregate `native:web:bundle` staging check;
- URL round-trip and invalid-input fallback;
- seed, pan, zoom, layer, view, and source controls;
- WebGPU capability and visible compute-result diagnostics;
- asynchronous comparison completion; and
- no complete game-client bootstrap or asset-pack fetch.

Rendered acceptance:

- run `pnpm host:check -- --probe-browser-webgpu` first;
- capture and inspect a desktop 3D GPU view;
- capture and inspect reference/GPU split and error views;
- capture and inspect a phone-sized map view;
- confirm non-black, non-transparent, materially changed pixels after seed,
  pan, and zoom; and
- keep every capture under `/tmp`.

## Commit Slices

1. Record the accepted hierarchy and this implementation tactical.
2. Add the shared reference-grid contract and tests.
3. Add the GPU evaluator/tile renderer and shader validation.
4. Add the dedicated Rust/WASM Terrain Lab host.
5. Add the responsive web product, controls, URL state, and tests.
6. Integrate the aggregate deployment route.
7. Reconcile validation evidence and close the tactical.

Each implementation commit uses:

```text
Topic: gpu-procedural-terrain
```

## Explicit Deferrals

- multi-tile clipmaps and coverage-first parent/child refinement;
- band-limited extreme-distance summary synthesis;
- exact CPU/GPU arithmetic parity;
- ordinary chunk handoff and edit overlays;
- canonical GPU generation or readback;
- native profiling UI;
- Far LOD, minimap, world-map, and XR integration;
- exact rivers, planned streams, structures, vegetation, and lighting; and
- volumetric caves, bricks, meshing, or ray casting.
