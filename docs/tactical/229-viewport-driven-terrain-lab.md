# Viewport-Driven Terrain Lab

Status: implementation and local headed-WebGPU validation complete 2026-07-24;
hosted deployment receipt pending.

Topic: `gpu-procedural-terrain`

## Objective

Turn the fixed-footprint Terrain Lab into a continuously navigable terrain
map: pan and zoom like a map, orbit into an oblique terrain view, let the
viewport request an appropriate sample level automatically, and permit an
explicit sample-spacing override for diagnosis.

This is a terrain-only interpretation of the Google Maps 3D interaction
model. It does not add buildings, imagery, labels, canonical chunks, or an
in-game minimap. The reusable result is the viewport, tile, scheduling, and
GPU-residency boundary needed by those later consumers.

## Originating Direction

The fixed Lab proves CPU/GPU field agreement, but its `Scale` control binds
two separate questions together:

1. how much world should the camera see; and
2. how densely should that world be sampled?

The requested product behavior is a continuously pannable and zoomable 3D
map. It should automatically load the detail justified by the viewport while
also allowing a manual `1:1`, `1:2`, `1:4`, and coarser diagnostic override.
Work should refine progressively, obsolete work should stop contributing, and
the CPU/GPU comparison should stay locked to identical coordinates.

## Product Contract

### Navigation

- Wheel or pinch changes a continuous world footprint, independently of
  sample spacing.
- Map drag pans. In 3D, left drag orbits while Shift+left or middle drag pans.
- Zoom is cursor-anchored in map view so the world point under the pointer is
  stable within integer world-coordinate precision.
- `Map` is the straight-down diagnostic view. `3D` is the terrain-height
  oblique view with the same center and zoom.
- The URL records seed, center, footprint, detail mode, source, view, layer,
  and camera angles. Legacy `spacing=` links retain their old visible
  footprint.

### Detail Selection

Terrain is divided into aligned `64 x 64`-cell sample tiles. Candidate sample
spacings are powers of two from 1 through 1,024 blocks.

`Auto` chooses the finest spacing that does not materially oversample the
visible panel. Its target is approximately two CSS pixels per sample cell.
Compare uses one panel's dimensions, not the combined canvas dimensions.

Manual detail requests an exact sample spacing. A bounded visible-tile budget
may raise the effective spacing for an excessively wide viewport. The UI must
show all three facts when they differ:

```text
requested detail -> effective detail -> visible tile count
```

This is an explicit safety limit, not silent quality degradation. Zooming in
eventually makes every manual level, including `1:1`, admissible.

### Progressive Refinement

Coverage precedes detail:

1. plan the coarsest level that covers the complete viewport with at most two
   tiles per axis;
2. admit missing tiles under a bounded per-frame work budget;
3. keep the previous complete parent level visible;
4. publish a finer level only when its required visible tiles are resident;
5. continue by halving spacing until the effective target is complete; and
6. retain a one-tile margin to make ordinary panning less likely to expose a
   gap.

The first implementation switches complete levels atomically. Geomorphing,
cross-fades, skirts, and mixed-level clipmap seams are follow-ups.

### Cancellation And Residency

Every viewport-affecting edit increments a request epoch. Queued tile work
from older epochs is discarded before compilation. A submitted GPU dispatch
cannot be preempted; its result may enter the bounded cache but cannot publish
an obsolete visible level. CPU reference compilation is likewise cancelable
between fixed-size tile jobs, not in the middle of one tile.

Resident identity includes:

```text
field revision + seed + tile origin + sample spacing
```

Camera, presentation layer, and CPU/GPU source are not generation identity.
Changing them must reuse resident samples. The cache is bounded and evicts
least-recently-used tiles that are not needed by the current parent or target
coverage.

## Ownership

- `mclone-worldgen` continues to own the production CPU sample and reference
  facts.
- `mclone-terrain-view` owns host-neutral viewport planning, tile identity,
  progressive-level state, bounded residency, WGPU compute, and drawing.
- `mclone-terrain-lab` owns the narrow browser/Wasm session facade.
- `tools/terrain-lab` owns DOM input, URL projection, responsive controls,
  status presentation, and the animation-frame pump.

No product scheduler or terrain semantics may live only in React. Browser CPU
reference compilation may remain an inline validation fallback behind the
same tile-session lifecycle during this tactical. Production browser CPU work
still converges on the existing shared-memory Web Worker architecture.

## Measurement Contract

The Lab reports:

- request epoch and requested/effective spacing;
- visible, resident, queued, compiled, stale, and evicted tile counts;
- time to coarse coverage and time to target detail;
- CPU reference compilation wall time when CPU evidence is requested; and
- WGPU encode/submit wall time.

Encode/submit time is not labeled GPU execution time. If portable WebGPU
timestamp queries are not available, the Lab says `GPU execution: unavailable`
instead of inventing a GPU speed comparison. An apparently instantaneous CPU
and GPU render therefore remains evidence about interaction latency, not a
claim of equal generator throughput.

## Bounded First Version

This tactical includes:

- continuous viewport footprint and cursor-anchored map zoom;
- Auto and manual spacing;
- aligned fixed-size tile planning;
- coarse-to-fine whole-level publication;
- stale-epoch rejection and bounded residency reuse;
- synchronized CPU/GPU Compare;
- responsive controls adjacent to the preview;
- deterministic planner/session tests;
- real headed-WebGPU desktop and phone-sized pixel inspection; and
- hosted `/terrain/` deployment plus smoke validation.

It deliberately stops before:

- canonical block/chunk generation or CPU readback as world authority;
- a game minimap or Far LOD integration;
- structures, labels, roads, textures, or building extrusion;
- Web Worker migration of the Lab-only CPU comparison compiler;
- clipmap rings, mixed-level seams, geomorphing, or predictive camera fetch;
- lighting beyond the existing diagnostic terrain presentation; and
- claims about GPU execution time without timestamp-query evidence.

## Implementation Slices

1. Record this contract and the shared ownership boundary.
2. Add pure Rust viewport/tile planning and deterministic tests.
3. Refactor the WGPU terrain view into bounded cached tiles and progressive
   complete-level publication.
4. Expose one request-epoch Wasm session with honest scheduling diagnostics.
5. Decouple Lab zoom from detail, add Auto/manual controls, and preserve
   synchronized Compare navigation.
6. Validate state, planner, Rust, Wasm, headed-WebGPU pixels, interaction
   traces, and stale-work behavior.
7. Update the living topic, deploy `/terrain/`, and record hosted evidence.

## Stop Conditions

Stop for review only if one of these becomes true:

- browser WGPU resource or binding limits make the fixed-tile cache shape
  non-portable;
- production CPU reference compilation cannot fit a bounded incremental frame
  budget even at one tile per turn;
- the shared renderer cannot retain identical CPU/GPU coordinates without
  duplicating policy into TypeScript; or
- inspected coarse-to-fine transitions expose a correctness gap that requires
  a mixed-level seam design beyond this tactical.

## Local Validation Receipt

The implementation landed through commits `c6f80e24`, `9bf31e2c`, and
`2ef406af`.

Validated locally:

- `cargo test -p mclone-terrain-view --lib`;
- `cargo test -p mclone-worldgen terrain_preview --lib`;
- `cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown`;
- Terrain Lab state tests and TypeScript/Wasm typecheck;
- two-project headed-Wayland Playwright desktop/Pixel 7 flow;
- dedicated headed-Wayland desktop and mobile BrowserWebGPU smokes; and
- inspected desktop side-by-side, phone stacked, map-error, orbit, and
  continent-scale captures under `/tmp`.

The fixed seed `-98765` / center `(-304, 336)` Auto viewport selected `1:16`
at about 2 km on both tested layouts. The 65.5 km map selected `1:512` and
retained 12 complete visible tiles. Its BrowserWebGPU comparison measured:

- base mean/P95 error effectively zero;
- 100% ocean agreement;
- continentalness mean error around `1.8e-8`; and
- no invented GPU execution duration.

A manual `1:1` request at 4.1 km visibly reported an effective `1:16` under
the eight-tile-per-axis safety budget. This is the intended diagnostic
contract, not a failure to honor manual detail silently.

The first Wasm pixel attempt exposed `std::time::Instant` as unsupported on
this target. Timing now comes from an injected browser performance clock; the
shared scheduler no longer depends on a host time implementation.
