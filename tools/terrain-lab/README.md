# Terrain Lab

Terrain Lab is the browser-hosted terrain workspace at `/terrain/`. Its
visible panes are independently configurable but coordinate-locked. One
global terrain profile selects the terrain family for every visible pane:

The ordinary URL opens one `Runtime composed` pane. Research and comparison
panes remain available through the pane controls and shareable URL state.
The header's **Play this seed** action opens `/play/` at the focused chunk,
requests immediate world entry, and starts in fly movement. Mclone terrain
uses composed presentation; vanilla-reference terrain falls back to exact-only
because the procedural horizon is not defined for that profile.

- `mclone-overworld-v1` (the default) provides exact Mclone terrain, a
  research landform plan, the production wildlife-population diagnostic, the
  streamed planner atlas, Mclone CPU LOD, and optional Mclone GPU LOD.
- `overworld` provides exact Minecraft Java 1.17.1 terrain and a direct
  Worker-backed vanilla CPU LOD. It has no GPU LOD.

The profiles cannot be mixed in one workspace. Within the selected profile:

- `Runtime composed` presents exact Mclone terrain and the procedural horizon
  on one shared camera and reversed-Z depth target. It uses the production
  asynchronous exact and vegetation services and is intentionally unavailable
  for the vanilla profile.
- `Real terrain` compiles exact first-party chunks through the production
  generator and renders their blocks, biomes, fluids, and final features with
  the production first-party texture atlas and cheap preview lighting.
- `Landform plan` is a research-only 2D summary over the fixed 6,144-block
  Tactical 267 plane domain. It shows independently selectable basin, quiet
  space, drainage, divide, confluence, and protected-sink facts. Production
  terrain does not consume this plan.
- `Wildlife` is a production diagnostic over fixed 64-by-64-block initial-
  population cells. A dedicated Worker calls the exact coordinate-pure Rust
  planner used by first entity-chunk realization. It shows quiet biome and
  landform substrate, desired density and occupancy rolls, all species
  weights, encounter anchors/group sizes, owner chunks, and aggregate visible
  counts. The browser draws receipts only; it does not simulate or spawn
  animals.
- `Planner atlas` is a research-only, freely pannable comparison of the
  coordinate-pure fallback, hierarchical shared facts, and feature-owned
  bounded graphs. Rust owns canonical region queries, plane/cylinder/torus
  lifts, bounded caches, and semantic checksums. Production terrain consumes
  none of the three candidates.
- `CPU LOD` samples the selected production preview. Mclone uses its preview
  fields and near-detail structured records; vanilla directly samples density
  columns, water, biome, and approximate surface material without generating
  chunks.
- `GPU LOD` evaluates Mclone's aligned natural fields in WebGPU and consumes
  the same sparse structured records.

At the Mclone `Cover` checkpoint, every preview point also carries production
forest coverage, density, family mix, canopy height/variation, and
grove/opening influence. Spacings `1`, `2`, and `4` use exact point intent
with center terrain plus four fixed-radius cardinal neighbors for slope.
Spacing `8` and above use four footprint quadrature taps at one quarter of the
sample spacing, combining coverage, density, family mixture, canopy
statistics, and grove influence. Both CPU modes therefore perform exactly five
terrain evaluations per output lattice point; neither walks the covered area.

The GPU evaluator applies the same coarse quadrature through its portable
macro fields. A GPU-only coarse `Cover` view consumes that GPU summary without
waiting for CPU samples. Near structured views still request CPU products
because stable individual records and planned stream records are shared Rust
products; WGSL does not invent a second tree planner.

Individual stable tree records are included only at sample spacings `1`, `2`,
and `4`. Spacing `1` admits every record, spacing `2` retains landmark ranks
`2/3`, spacing `4` retains rank `3`, and coarser views are summary-only with
zero tree-record queries. One shared procedural instancing pipeline draws
broadleaf, conifer, and acacia trunks/crowns in map and 3D; CPU and GPU panes
therefore preserve the same base, family, dimensions, and orientation while
their terrain compilers remain independent.

The headed-Wayland smoke runs `Cover` at 65.5 km with cold, warm, cache-off,
and GPU-only receipts. It directly checks CPU/GPU terrain, forest, and
footprint work counts; packing, upload, and readback bytes; zero record/cache
traffic at coarse scale; and independent GPU publication. The `Forest
summary` diagnostic layer exposes family-colored coverage and openings. The
same fixed 65.5 km anchor is captured in map view at `1:1024`, `1:512`, and
`1:256`, plus 3D at `1:256`.

The exact compiler runs in a replaceable Web Worker and publishes chunks
center-first as they finish. The LOD panes use 64 × 64 cell / 65 × 65 sample
tiles and may cover and progressively refine many tiles. Every visible pane
shares seed, center, footprint, and navigation; terrain panes also share the
camera. The landform plan stays two-dimensional.

The URL owns the review state:

- `seed`: signed 64-bit world seed
- `profile`: `mclone-overworld-v1` or `overworld`
- `x` and `z`: preview center in blocks
- `blocks`: continuous viewport width from 1 through 131,072 blocks
- `detail`: `auto` or a power-of-two sample spacing from 1 through 1,024
  blocks
- `panes`: comma-separated `runtime`, `canonical`, `plan`, `wildlife`, `atlas`,
  `semantic`, `cpu`, `macro`, and/or `gpu`; `runtime`, `plan`, `wildlife`,
  `atlas`, and `semantic` are Mclone-only
- `canonical`: `surface` or `final`
- `radius`: one of `0`, `1`, `2`, `3`, `4`, `5`, `7`, `10`, or `15`,
  corresponding to centered footprints from `1x1` through `31x31 = 961`
  exact chunks
- `water` and `vegetation`: `1` to show or `0` to hide retained exact blocks
- `source`: legacy/procedural compatibility value (`reference`, `gpu`, or
  `split`); new links should use `panes`
- `view`: `3d` or `map`
- `projection`: `orthographic` (the default) or `perspective`; map view is
  always orthographic
- `reviewYaw` and `reviewPitch`: optional static initial/reload camera for
  fixed review links; ordinary orbit motion remains outside terrain URL state
- `stage`: `base`, `hydrology`, `structured`, `surface`, or `cover`
- `layer`: `terrain`, `height`, `error`, `continentalness`, `climate`,
  `rivers`, `wetlands`, `landforms`, `biomes`, `surface`, or `streams`
- `planBasins`, `planQuiet`, `planDrainage`, `planDivides`,
  `planConfluences`, and `planSinks`: `1` to show or `0` to hide each
  research-plan fact without rebuilding the plan
- `atlasTopology`: `plane`, `cylinder-x`, or `torus`
- `atlasRegions`, `atlasSamples`, `atlasFeatures`, `atlasHierarchy`,
  `atlasFacets`, `atlasGraphBounds`, `atlasGraphEdges`, `atlasIdentity`, and
  `atlasSeams`: `1` to show or `0` to hide each streamed-atlas overlay
  without rebuilding canonical plans

Legacy `spacing=` links still load with their original `spacing × 64`
footprint and fixed detail. New links keep coverage and resolution independent.

Wheel and two-finger gestures zoom the continuous viewport; centroid movement
pans during the same gesture. The stage captures wheel events
through a non-passive native listener, so a Mac trackpad or mouse wheel zooms
terrain without scrolling the document. Map zoom is anchored under the
pointer; map drag follows grab semantics on both axes; 3D left drag orbits;
and right, Shift+left, or middle drag pans in 3D. Arrow keys pan both views.
A shared projection selector applies the same orthographic or perspective
camera to exact, CPU LOD, and GPU LOD panes. Orthographic is the default so
aligned terrain keeps one scale from the near edge to the far edge.
A click or tap without a drag selects a Mclone production point receipt using
analytic map picking or the shared 3D projection and a bounded heightfield
ray. Vanilla point receipts are explicitly unavailable in the bounded first
pass. Tapping the landform plan instead requests a Rust-owned cell receipt
with basin/receiver identity, drainage accumulation/order, envelope strengths,
and structural flags.
Tapping the wildlife pane selects one production population-cell receipt with
habitat evidence, desired density, occupancy and species rolls, every species
weight, selected anchor, group size, and owning chunk.
`Auto` selects approximately two CSS pixels per sample cell. A manual detail
request remains visible even when the bounded eight-tile-per-axis interactive
budget must raise its effective spacing; zooming in eventually admits every
manual level, including `1:1`.

Canonical camera motion is independent from exact desired coverage. Moving
within one center chunk redraws the retained terrain without changing the
generation epoch. Crossing a chunk boundary preserves the old/new footprint
intersection, removes only departed chunks, and schedules only the entering
edge. Cached and Worker-produced results share a one-chunk-per-animation-frame
admission queue with bounded Worker backpressure, so a large cached footprint
cannot replay synchronously on the UI thread. Hard seed, checkpoint,
cache-mode, or cold-cache changes still replace the Worker and reject stale
results. Water and vegetation switches only remesh retained chunks; they do
not rerun or mutate generation.

The shared LOD scheduler has independent CPU compilation and GPU dispatch
queues. Mclone CPU tiles compile through the ordinary shared path. Vanilla CPU
tiles leave the render loop, compile in a retained Web Worker sampler, and
return through profile- and revision-checked asynchronous tile admission.
Each lane covers the viewport coarsely before refining through nested
power-of-two levels, and each publishes its finest complete level without
waiting for the other. The evidence panel reports canonical resident reuse,
admission frames, arrival/generation/upload timing, and the LOD queues,
readiness, throughput, and published detail.

CPU reference wall time and WGPU encode/submit wall time are not GPU execution
time. Portable timestamp queries are not enabled in this Lab slice, so the UI
says `GPU execution: unavailable` rather than inferring a GPU/CPU speed ratio.
The GPU evaluator remains a preview experiment, not authoritative world
generation. Content checkpoints are dependency-ordered preview compiler
stages, not gameplay generator flags. Natural rivers, banks, wetlands, pools,
and submerged outlets are pure CPU/GPU fields. Planned streams remain bounded
CPU route records reconstructed only through `1:4`, then merged into either
LOD lane. Coarser structured views explicitly report them unavailable.

When both LOD panes are visible, desktop workspaces place them beside the
canonical pane as equal-height logical columns. Responsive phone workspaces
stack canonical, CPU LOD, and GPU LOD as equal-size logical rows. The CPU and
GPU panels publish independently at identical coordinates; their labels expose
the race while the final comparison waits for matching target samples. The
exact pane is not another height field: it is a bounded block-and-feature patch
nested at the same world center and scale. Generated fallback atlas tiles
intentionally remain visible for block materials that do not yet have curated
first-party textures.

The exact and LOD caches are independent. `Exact cache on` retains raw
generated chunks by profile, seed, checkpoint, and chunk coordinate in a
session-local 1,024-chunk LRU; exact GPU residency remains the separate current
desired set. Returning to a cached exact coordinate admits its raw result
through the same paced queue. `Exact cache off` compiles entering coordinates
cold while still preserving the current old/new resident overlap. The
evidence panel reports Wasm-resident raw bytes, browser-cache raw bytes, and
used vertex/index/grass mesh ranges. Their `Real tracked (lower bound)` sum
does not claim Wasm/JavaScript allocator overhead, spare GPU arena capacity,
the atlas, pipelines, render targets, dependency caches, or browser memory.

`Cache on` uses a session-local 192-tile LRU keyed by profile, seed, aligned
origin, sample spacing, and content stage. It retains CPU samples, uploaded
reference data, forest summaries, admitted tree records, GPU-computed buffers,
and completed validation readbacks while panning and zooming. Vegetation source
revision participates in product identity; camera, layer, map/3D mode, and
pane visibility do not. `Cache off` retains only the active request and
disables speculative preload. `Cold current view` invalidates both cache
domains explicitly. The evidence panel reports summary/record availability,
aggregated tiles, semantic instance/upload/proxy counts, planning-cell
requests/hits/misses, retained cells, and combined residency.

`Run stress race` selects the fixed review seed/site, turns cache off, and
uses a larger 12-tile-per-axis diagnostic budget over a 2 km requested-`1:2`
Compare map. This makes independent publication observable without changing
the default interactive budget. CPU end-to-end and GPU
dispatch-to-validation-readback throughput use labeled, different timing
boundaries; neither is presented as pure GPU execution time.

The LOD preview surface uses upward-facing counter-clockwise triangles,
rejects back faces, and projects the oblique camera from above the height
field. CPU and GPU LOD share the same per-tile sample lattices and viewport
plan. The sample payload retains natural river signed distance and width, so
the renderer preserves a bounded river contour at levels coarser than the
physical channel. Comparison reports both base and final height/water/material
agreement plus channel, bank, wetland, biome, and surface agreement. Full
provenance remains point-local in the inspector rather than bloating every
tile.

## Develop

From the repository root:

```sh
pnpm terrain-lab:install
pnpm terrain-lab:web
```

Vite serves the lab at <http://127.0.0.1:5180/terrain/>. Rust/WASM bindings are
regenerated before every development or production build.

For the accepted hybrid-plan review site, enable **Landform plan**, set the
center to `0, 0`, and zoom to `6.1 km`, or use:

<http://127.0.0.1:5180/terrain/?seed=-98765&x=0&z=0&blocks=6144&panes=plan&view=map>

For Tactical 270 Human Review R1, use **Planner atlas** and compare all three
panels while panning:

<http://127.0.0.1:5180/terrain/?seed=-98765&x=0&z=0&blocks=6144&panes=atlas&view=map&atlasTopology=plane>

Switch `atlasTopology` to `cylinder-x` or `torus` to inspect periodic seams.

For the deterministic initial-wildlife review, use **Wildlife** at the accepted
mixed meadow/woodland/water site:

<http://127.0.0.1:5180/terrain/?profile=mclone-overworld-v1&seed=-98765&x=0&z=-2048&blocks=1024&panes=wildlife&view=map>

The wildlife pane is not a second tuning implementation. The Wasm facade must
continue calling `McloneOverworldWildlifePlanner`, and the source-ownership
test rejects density or species arithmetic in the TypeScript canvas.

## Validate

```sh
pnpm terrain-lab:typecheck
pnpm --dir tools/terrain-lab test
pnpm terrain-lab:web:test
pnpm terrain-lab:web:smoke
pnpm terrain-lab:web:smoke -- --mobile
```

The smoke path follows the repository's headed-Wayland WebGPU policy on Linux
and writes screenshots and its diagnostic report under `/tmp`.

To exercise an already-hosted aggregate deployment without rebuilding or
starting Vite:

```sh
TERRAIN_LAB_SMOKE_BASE_URL=https://mclone.kzahel.com \
  pnpm terrain-lab:web:smoke
```

Add `-- --large-canonical` (and optionally `--mobile` after it) to the smoke
command when an explicit, multi-minute production-route proof of the `31x31`
exact footprint is wanted. That lane verifies 930-chunk overlap, a 31-frame
entering edge, a 31-hit cached return, bounded cache and tracked-memory facts,
and saves the completed pane under `/tmp`.
