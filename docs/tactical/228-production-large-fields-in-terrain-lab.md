# Production Large Fields In Terrain Lab

Status: complete 2026-07-23.

Topic: `gpu-procedural-terrain`

## Objective

Turn the first hosted Terrain Lab proof into a trustworthy generator-iteration
tool by porting the production Mclone Overworld's large-scale field graph to
the shared GPU evaluator and making the three-dimensional interaction and
comparison defaults self-explanatory.

This slice should make a seed recognizable on both sides of a CPU/GPU
comparison across kilometer-scale footprints. It does not claim exact final
terrain parity: the CPU reference continues to include major rivers, wetlands,
and planned-stream carving, while this port stops at the production base
surface.

## Originating Direction

The first hosted proof exposed two product problems immediately:

- left mouse drag changed the sampled geographic center when a
  three-dimensional view conventionally suggests orbit; and
- the default split compared production terrain with an unrelated approximate
  field graph, so the two halves did not look like the same seed.

The requested next step is an end-to-end production-field port before another
review. The primary payoff is a fast, hosted macro-scale loop for evaluating
first-party terrain changes, including kilometer-scale and later
tens-of-kilometers views.

## Exact Port Boundary

The A2 evaluator ports the unbounded production graph in
`McloneOverworldSampler` through `base_surface_y`:

1. the production 64-bit signed-seed/domain/lattice hash, emulated from two
   `u32` words because portable WGSL has no 64-bit integer type;
2. cubic value noise and quintic 16-direction gradient noise;
3. continentalness, relief, ruggedness, ridges, and warped mountain detail;
4. warped temperature and moisture;
5. production land-height shaping; and
6. ocean basin selection, shelf/basin depth, and seabed relief.

The shader uses the production domains, scales, weights, warp constants, and
height formulas. It evaluates the same point samples at every spacing; coarse
band limiting and parent summaries remain the next progressive-refinement
experiment rather than silently changing the comparison target.

The A2 boundary deliberately excludes:

- major-river signed-distance derivatives and morphology;
- wetlands and bounded planned-stream records;
- final watercourse carving and water levels;
- surface recipes, vegetation, structures, lighting, chunks, and authority;
  and
- periodic-cylinder lattice wrapping.

Consequently, residual height/water disagreement along real watercourses is
expected and should remain visible. Broad continentalness, land/ocean
classification, base height, bathymetry, and climate disagreement is a defect.

## Shared Ownership

`mclone-worldgen` remains the source of truth. It exposes a compact
large-field specification whose domain and scale values are also used to build
the production sampler.

`mclone-terrain-view` generates the WGSL constant preamble from that
specification, owns the portable 64-bit arithmetic and field evaluator, and
retains bounded readback comparison.

`mclone-terrain-lab` remains a thin browser/WebGPU adapter.

`tools/terrain-lab` owns presentation-only orbit state and browser gestures:

- 3D left drag orbits;
- Shift+left or middle drag pans geography;
- map left drag pans geography;
- wheel and explicit controls change sample spacing;
- the initial source is production Reference, not Split; and
- separate controls reset the camera and load the fixed review site.

Camera orbit is local presentation state. Seed, center, spacing, source, view,
and layer remain URL-addressed terrain-request state.

## Validation

Required gates:

- production large-field fixed-point fixtures in Rust;
- exact shader-spec generation and WGSL parse/validation;
- native and `wasm32-unknown-unknown` checks;
- TypeScript state/gesture tests and Playwright desktop/mobile journeys;
- a real headed-Wayland BrowserWebGPU comparison readback;
- inspected Reference, GPU, Split, Error, and phone captures under `/tmp`;
- fixed-site receipts for mean/p95/max final-height error and water agreement;
- comparison with the CPU base-surface residual floor so river-only
  disagreement is not mistaken for a failed large-field port;
- aggregate bundle staging and hosted-route smoke; and
- a production deployment of the exact validated revision.

## Commit Slices

1. Record this bounded port and its honest residual boundary.
2. Share the production field specification and fixed-point fixtures.
3. Port the exact large-field graph into the A2 WGSL evaluator.
4. Add orbit-first interaction and truthful reset/default semantics.
5. Tighten browser comparison gates from measured A2 evidence.
6. Reconcile the topic, deploy, and record hosted evidence.

Every implementation commit uses:

```text
Topic: gpu-procedural-terrain
```

## Stop Conditions

Pause for review only if portable WGSL cannot reproduce the production 64-bit
hash reliably on admitted WebGPU adapters, the field port reveals a required
generator-contract change, or measured broad-field disagreement remains after
the exact graph is implemented. Ordinary numerical debugging and UX polish
remain inside this tactical.

## Result

The A2 evaluator now ports the complete intended boundary:

- the production signed-seed/full-domain 64-bit lattice hash runs in portable
  WGSL through a tested pair-of-`u32` implementation;
- Rust generates every GPU domain and scale constant from
  `MCLONE_OVERWORLD_LARGE_FIELD_SPEC`, which the CPU sampler also consumes;
- cubic value noise and the production 16-direction quintic gradient noise
  drive continentalness, relief, ruggedness, ridges, warped mountain detail,
  warped climate, land height, and bathymetry;
- preview schema v2 carries both final production surface/water and
  base-surface/ocean facts, so comparisons isolate the omitted hydrology
  family;
- the compute and reference buffers remain resident and render with no CPU
  vertex or index arrays; and
- the evaluator remains presentation-only and does not mutate canonical world
  state.

The product behavior is now conventional and explicit:

- 3D left drag orbits through a shared renderer camera uniform;
- Shift+left and middle drag pan geography in 3D;
- left drag pans geography in map view;
- camera reset does not change the seed or sampled location;
- `Load review site` names the fixed request instead of looking like a generic
  reset;
- the initial source is production Reference;
- Compare labels the left side `CPU final reference` and the right side `GPU
  production base`; and
- the page explains that rivers, wetlands, and planned streams are the
  remaining reference-only residual.

## Evidence

The fixed review request is seed `-98765`, center `(-304, 336)`, 32-block
spacing, and a 2,048-block footprint. Headed-Wayland BrowserWebGPU measured:

- base-surface mean/P95/maximum error: `0.0 / 0.0 / 0.0` blocks;
- ocean-presence agreement: `100.0%`;
- mean continentalness error: `2.07e-8`;
- final-surface mean/P95 error: `0.5 / 3.0` blocks, attributable to the
  explicitly omitted watercourse family;
- resident preview allocation: about `396.2 KiB`; and
- representative production-reference compilation and encode/submit host
  clocks: about `2.0-4.7 ms` and `0.3-1.3 ms`.

The 1,024-block spacing uses the same 4,225 samples to span 65,536 blocks
(65.5 km) and measured:

- base-surface mean/P95 error: `0.0 / 0.0` blocks;
- ocean-presence agreement: `100.0%`; and
- mean continentalness error: `1.91e-8`.

This is exact point-sample evidence, not a claim that the 65.5 km image is
already a good coarse summary. Fine production bands visibly alias at that
spacing; scale-aware aggregation/band limiting is the next tactical.

The dedicated production bundle is:

- Terrain Lab WASM: `496.86 kB` raw / `142.66 kB` gzip; and
- Terrain Lab JavaScript: `238.49 kB` raw / `75.20 kB` gzip.

Validation completed:

```text
cargo test -p mclone-worldgen --lib
cargo test -p mclone-terrain-view
cargo test -p mclone-terrain-lab
cargo check -p mclone-terrain-view --target wasm32-unknown-unknown
cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown
pnpm --dir tools/terrain-lab test
pnpm terrain-lab:typecheck
pnpm terrain-lab:web:test
pnpm terrain-lab:web:smoke
pnpm terrain-lab:web:smoke -- --mobile
pnpm host:check -- --probe-browser-webgpu
pnpm native:web:bundle
```

Both Playwright projects passed. The smoke journey independently enforces the
2 km and 65.5 km large-field thresholds, proves orbit leaves the terrain URL
unchanged, resets the camera, then exercises map, error, zoom, and
continent-scale controls.

Inspected captures under `/tmp` include:

- `mclone-terrain-lab-desktop-canvas.png`;
- `mclone-terrain-lab-desktop-orbit.png`;
- `mclone-terrain-lab-desktop-map-error.png`;
- `mclone-terrain-lab-desktop-continent-scale.png`; and
- corresponding full-page and Pixel 7-sized captures.

## Deployment

The exact aggregate bundle was built at commit `a6e4e0f2` with asset version
`c2c6c9e0711c-20260723205619`. Because the standard deploy loop was spending
most of its time re-uploading hundreds of unchanged animal-catalog objects,
publication was safely narrowed after the complete bundle build to the four
new content-addressed Terrain Lab objects:

```text
terrain/index.html
terrain/assets/index-DoWPFh8W.js
terrain/assets/index-CR013qzw.css
terrain/assets/mclone_terrain_lab_bg-4694YNq0.wasm
```

All four R2 uploads completed before the Worker was published. Production is
live at
[mclone.kzahel.com/terrain/](https://mclone.kzahel.com/terrain/) under
Cloudflare Worker version `6e8ce34c-57f0-443d-8471-1930e6934c48`.

The hosted desktop and Pixel 7 smoke journeys both passed through 2 km
comparison, orbit, camera reset, error map, and the 65.5 km continent view with
the same local parity thresholds. Hosted response checks confirmed:

- `/terrain/`: `200`, `text/html`, no-cache;
- the A2 WASM: `200`, `application/wasm`, 496,862 bytes, immutable caching; and
- COOP `same-origin`, COEP `require-corp`, and CORP `same-origin` on both.
