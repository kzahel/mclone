# Tactical 244: LOD-Native Vegetation Presentation

Status: active 2026-07-25.

Topic: `lod-native-vegetation`

Workstream: shared forest summaries, bounded stable tree-proxy admission,
Terrain Lab CPU/GPU presentation, current in-game Far LOD adoption,
exact/proxy arbitration, and rendered platform evidence.

## Objective

Carry Tactical 243's canonical Mclone tree identity through the two existing
LOD hosts:

```text
production forest intent
  -> scale-proportional summary samples
  -> bounded stable record admission
       -> Terrain Lab map/3D instances
       -> clipped in-game Far LOD proxy fragments
```

The completed slice must make coarse forests visible without enumerating
trees, preserve base/family/silhouette identity wherever an individual proxy
is admitted, and reuse the current Terrain Lab viewport and Far LOD resident
lifecycles. It must not add a second scheduler, GPU placement policy,
per-tree mesh allocation, or owner-chunk suppression shortcut.

## Binding Representation Decisions

### Summary product

`Cover` samples carry production forest coverage, density, family, canopy
height/variation, and grove/opening influence beside terrain samples.
Summary compilation is proportional to the existing sample lattice. It never
queries tree records when the sample spacing is coarser than the individual
admission checkpoint.

The CPU reference lane evaluates the shared Rust forest-intent owner. The GPU
kernel evaluates the portable macro form for diagnostics, but both displayed
CPU and GPU `Cover` panes consume the uploaded CPU vegetation product. GPU
base, hydrology, structured terrain, and surface compilation remain
independent; only the explicitly requested vegetation checkpoint waits for
the CPU product.

### Individual proxy admission

Individual records are requested only at sample spacings `1`, `2`, and `4`.
Admission is a pure global predicate of sample spacing and the record's
landmark rank:

- spacing `1`: all records;
- spacing `2`: rank `2` or `3`;
- spacing `4`: rank `3`; and
- coarser levels: no individual records.

The same tree therefore survives or aggregates in every intersecting tile.
There is no tile-local top-N decision that could cut a cross-boundary crown
in only one tile. Omitted trees remain represented by the forest summary.

Terrain Lab uses one procedural archetype draw with a compact per-tree
instance buffer. Broadleaf, conifer, and acacia instances preserve the
record's base, dimensions, orientation, and family color. No unique GPU mesh
is allocated for a tree.

### In-game handoff

Only `mclone-overworld-v1` Far LOD receives vegetation. Level-one tiles admit
all records, level two admits rank `2` or `3`, and level three uses summaries
only. Every proxy volume is clipped to the same 16-by-16 chunk tile that owns
its Far LOD terrain fragment.

The existing painted-capable real/synthetic coordinator suppresses that
whole tile fragment. When a real chunk becomes drawable, its exact tree
blocks replace the clipped proxy fragment for the same footprint. A tree
whose bounds cross chunks is queried and clipped independently in every
intersecting tile, so neither its base chunk nor mere residency controls the
handoff.

The existing Far LOD vertex path already implements mono, per-eye, and
full-frame multiview. Vegetation extends its tile payload rather than adding
a view-specific renderer.

## Shared Ownership

- `mclone-worldgen` owns summary facts, preview packing, record admission, and
  deterministic summary/record products.
- `mclone-terrain-view` owns the shared archetype pipeline, compact instance
  buffers, viewport residency, draw admission, and presentation metrics.
- `mclone-terrain-lab` owns narrow Wasm getters only; browser TypeScript owns
  labels and assertions, not vegetation policy.
- `mclone-app-runtime::far_lod` owns synthesis into the existing Far LOD tile
  compile job and clipped tile geometry.
- `mclone-render-session`, `mclone-render`, and `mclone-scene` retain their
  existing resident-tile, mono/stereo/multiview, and painted-coverage owners.

## Exclusions

- no Java `overworld`, Alpha, Beta, Small Island, or authored vegetation;
- no player-grown or edited-tree invalidation overlay;
- no server-streamed records or concealed-seed protocol;
- no persistent summary/record database;
- no wind, bush cards, falling trees, or new gameplay authority; and
- no universal natural-feature abstraction.

## Slice Plan

### Slice 0: contract and baseline

- [x] Lock summary, rank admission, clipping, and existing-lifecycle reuse.
- [x] Pin current Terrain Lab `Cover` and Mclone Far LOD surface-only
  diagnostics before presentation changes.

Gate: every new product has an owner, bounded work rule, and exact handoff.

Baseline record:

- commit `8a9430e3` packs 24 floats (six `vec4` values) per Terrain Lab sample;
  no forest summary or individual record bytes exist in a viewport tile;
- `Cover` derives one constant coverage value from biome recipe code in the
  render shader, so CPU and GPU panes have zero production vegetation facts
  and zero proxy instances;
- the existing local headed-Wayland Terrain Lab smoke passes with
  `gpu-preview-a5`, 33 resident cold-race tiles, 88,725 samples, zero cache
  hits, 25.53 MiB reported GPU residency, and the surface-only capture
  `/tmp/mclone-terrain-lab-desktop-canvas.png`;
- the Mclone Far LOD worker already receives the generation profile and
  retains bounded structured surface/stream facts, but its tile payload is
  exclusively terrain top/drop faces; it has zero vegetation-summary colors,
  record queries, or proxy vertices; and
- the existing Far LOD renderer admits one chunk-aligned tile atomically and
  already supports mono, per-eye, and multiview. The presentation slice
  extends that payload and does not replace this control.

### Slice 1: shared summaries and preview products

- [x] Add typed forest summary fields to the shared preview sample schema.
- [x] Evaluate production CPU forest intent and portable GPU macro intent.
- [x] Add bounded near record products and deterministic admission tests.
- [x] Integrate source revision, cache reports, and summary/record byte counts.
- [x] Prove coarse requests issue zero individual record queries.

Gate: `Cover` is a truthful vegetation semantic product, not a biome tint.

Milestone record:

- reference schema `mclone-terrain-preview-reference-grid-v7` carries eight
  new production forest fields and reconstructs the exact fixed-radius slope
  at `Cover`;
- GPU evaluator `mclone-overworld-v1-gpu-preview-a6` evaluates the same
  dedicated grove domain and macro family/coverage equations, while displayed
  cover tint reads the uploaded CPU forest product in both panes;
- `TerrainPreviewVegetationProduct` requests base-owned stable occurrences
  only at spacing `1/2/4`, applies the binding global landmark-rank predicate,
  and returns per-request planning-cell cache deltas; and
- a spacing-eight `Cover` fixture proves summary availability with zero tree
  occurrences and zero vegetation planning-cell requests.

### Slice 2: Terrain Lab proxy presentation

- [x] Upload compact records with each resident preview tile.
- [x] Draw one shared archetype family through instancing in map and 3D.
- [x] Present the same CPU-produced vegetation overlay in CPU and GPU panes.
- [x] Expose summary/record readiness and performance diagnostics.
- [x] Prove pan/zoom/cache reuse and stable exact/LOD IDs.

Gate: zooming aggregates trees without moving or changing admitted records.

Milestone record:

- each resident tile uploads one 48-byte semantic instance per admitted tree
  for each of the two possible comparison panels; the one procedural shader
  emits trunk and family-specific broadleaf, conifer, or acacia crown boxes
  without per-tree mesh allocation;
- CPU and GPU panes draw the same CPU-produced occurrence buffer over their
  independently compiled terrain, including side-by-side desktop and stacked
  phone layouts;
- frame reports expose the vegetation source revision, summary/record/
  aggregated tile counts, semantic instances, upload bytes, proxy vertices,
  planning-cell requests/hits/misses, retained cells, and total residency;
- repeated shared-product tests preserve occurrence identity and hit the
  vegetation cache, while browser assertions prove spacing `4` has stable
  rank-3 records and spacing `8` has summary-only aggregation with zero
  record queries; and
- all 21 `mclone-terrain-view` tests and Terrain Lab Wasm type checking pass.
  Headed Wayland WebGPU desktop and Pixel 7 projects both pass the targeted
  browser contract. Inspected 3D and map captures are
  `/tmp/mclone-terrain-lab-{desktop-chrome,phone-chrome}-vegetation-cover.png`
  and
  `/tmp/mclone-terrain-lab-{desktop-chrome,phone-chrome}-vegetation-map.png`.

### Slice 3: in-game Far LOD adoption

- [x] Add forest summary tint/mass to Mclone Far LOD lattice cells.
- [x] Reuse the worker-owned vegetation cache for admitted exact records.
- [x] Append family-specific proxy volumes clipped to each tile.
- [x] Prove cross-chunk fragments, level admission, and source invalidation.
- [x] Prove Far-LOD-off and non-Mclone payloads remain unchanged.

Gate: current Far LOD renders Mclone vegetation through its existing resident
and painted-coverage lifecycle.

Milestone record:

- every Mclone Far LOD lattice sample evaluates production forest intent and
  blends bounded dominant-family canopy color into terrain top/side color;
  other profiles skip the evaluation;
- native compile workers and each persistent browser render-compiler session
  own one bounded `McloneOverworldVegetationPlanCache`, reset it with the
  surface/stream cache on source changes, and return the same packed tile
  contract;
- auto level one admits all intersecting records, level two admits rank `2`
  and `3`, and level three makes zero record queries; broadleaf, conifer, and
  acacia each emit a trunk plus two silhouette volumes;
- each volume is clipped independently to the owning 16-by-16 Far LOD tile.
  A cross-chunk fixture proves both adjacent fragments exist and no vertex
  escapes its tile, without owner-chunk suppression;
- the representative level-one tile has 16 summary cells, one record query,
  two admitted occurrences, six boxes, 48 proxy vertices, and 216 proxy
  indices. Level two retains the two ranked occurrences and six boxes from
  four summary cells; level three has one summary cell and zero queries or
  proxy geometry;
- a repeated two-tile source fixture records 32 planning-cell requests,
  16 hits, 16 misses, 16 retained cells, and 91 fixed candidates; and
- the pre-change Java-overworld payload remains exactly 100 vertices,
  150 indices, and fingerprint `0x5c1c22da08c489e6`. A Far-LOD-off native
  capture reports zero Far LOD draws/uploads and retains only exact terrain.

### Slice 4: rendered and platform closeout

- [x] Inspect Terrain Lab desktop and phone map/3D pixels through summary and
  individual levels, including meadow, woodland, conifer, steppe, streams,
  and forest edges.
- [ ] Inspect in-game stationary and moving exact/proxy handoff pixels.
- [ ] Run native, browser Worker/WebGPU, mono, synthetic stereo, and available
  multiview contract gates.
- [ ] Record planning, packing, upload, resident bytes, vertices, indices,
  draw counts, and Far-LOD-off comparison.
- [ ] Update the topic, Terrain Lab product documentation, and platform
  evidence.

Gate: the topic's first exact-to-coarse end-to-end vegetation path is
documented, measured, and visibly accepted.

## Commit Plan

1. Record this tactical and the surface-only baseline.
2. Add shared summary and bounded preview record products.
3. Add Terrain Lab instanced proxy presentation and diagnostics.
4. Adopt the same records and summaries in in-game Far LOD.
5. Close rendered/platform evidence and reconcile living docs.

Every implementation commit uses:

```text
Topic: lod-native-vegetation
```
