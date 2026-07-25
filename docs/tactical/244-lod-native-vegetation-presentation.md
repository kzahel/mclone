# Tactical 244: LOD-Native Vegetation Presentation

Status: active 2026-07-25. Terrain Lab proxy presentation is landed; coarse
hierarchy, footprint summary, budget, and cache proof are the current gate.

Topic: `lod-native-vegetation`

Workstream: shared footprint-scale forest summaries, bounded stable tree-proxy
admission, Terrain Lab CPU/GPU cost and cache proof, multi-scale rendered
evidence, and a quarantined compatibility adapter for the current in-game Far
LOD system.

## Objective

Establish the vegetation representation worth adopting from Tactical 243's
canonical Mclone tree identity, using Terrain Lab as the proof host:

```text
production forest intent
  -> footprint-filtered, scale-aware summaries
  -> bounded stable near-record admission
       -> Terrain Lab CPU/GPU cost comparison
       -> Terrain Lab map/3D instances
       -> later in-game architecture decision
```

The completed tactical must demonstrate `Cover` at the existing 65.5 km
footprint, make coarse forests recognizable without enumerating trees, prove
CPU/GPU and cache costs remain proportional to the visible sample lattice,
and preserve base/family/silhouette identity only where a near record is
admitted. It must not treat the legacy chunk Far LOD system as the target
architecture before that representation proof exists.

## Binding Representation Decisions

### Summary product

`Cover` samples currently carry pointwise production forest coverage, density,
family, canopy height/variation, and grove/opening influence beside terrain
samples. This is a truthful local intent sample, but it is not yet accepted as
a footprint-scale summary. Each CPU sample reconstructs fixed-radius slope
from four additional terrain samples, so lattice-proportional output count
does not by itself prove acceptable continent-scale cost.

The CPU reference lane evaluates the shared Rust forest-intent owner. The GPU
kernel evaluates the portable macro form for diagnostics, but both displayed
CPU and GPU `Cover` panes consume the uploaded CPU vegetation product. GPU
base, hydrology, structured terrain, and surface compilation remain
independent. The next slice must measure this dependency explicitly and decide
which scale-aware summary facts can remain independently GPU-evaluable without
creating a second semantic tree planner.

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

### In-game compatibility adapter

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

Commit `b1db968c` implements this behavior, but it is a disposable
compatibility adapter rather than the destination architecture. Do not expand
or close it out while the Terrain Lab hierarchy/budget gate is open. A later
in-game LOD tactical may retain, replace, quarantine, or revert it after the
representation and terrain-LOD architecture are chosen deliberately.

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

### Slice 3: hierarchy, footprint summaries, budgets, and cache proof

- [ ] Run `Cover` at the existing 65.5 km/65,536-block Terrain Lab footprint.
- [ ] Record CPU compile, GPU dispatch/validation, packing, upload, sample,
  tile, and vegetation-cache costs for that exact request.
- [ ] Replace pointwise values with filtered footprint summaries where the
  inspected coarse result aliases forest edges, openings, or family mix.
- [ ] Prove summary work is proportional to the visible sample lattice and
  has no tree-record, chunk-generation, or hidden footprint-area walk.
- [ ] Preserve exact records only at spacings `1/2/4` and prove zero record
  planning/cache traffic at every coarser level.
- [ ] Inspect the same forest edges, openings, meadow, conifer, and steppe
  anchors across multiple power-of-two spacings in map and 3D.
- [ ] Decide whether the displayed GPU `Cover` lane can consume an independent
  portable summary product or must report its CPU dependency explicitly.

Gate: a 65.5 km `Cover` request is visibly useful, measured, and bounded by its
sample lattice rather than merely returning point samples without records.

Required evidence:

- cold and warm/cache-off receipts for at least one 65.5 km request;
- per-sample terrain/forest evaluation counts or an equivalent direct work
  counter, not wall time alone;
- CPU/GPU wall-time boundaries labeled honestly;
- a spacing matrix that keeps world anchors fixed while detail changes;
- direct assertions that spacing `8+` issues zero planning-cell requests and
  zero individual instances; and
- inspected `/tmp` captures at the first working 65.5 km `Cover` milestone and
  after any filtering change.

### Slice 4: current in-game compatibility adapter — landed, paused

- [x] Add local forest tint and clipped proxy volumes to Mclone Far LOD tiles.
- [x] Reuse bounded native/browser worker vegetation caches.
- [x] Prove cross-chunk clipping, level admission, source invalidation,
  Far-LOD-off, and non-Mclone byte stability.
- [ ] Do not expand, polish, or treat this adapter as final architecture while
  Slice 3 remains open.

Gate: none for the target architecture. Commit `b1db968c` is retained only as
a reversible compatibility experiment.

Recorded adapter facts:

- auto level one admits all intersecting records, level two admits ranks
  `2/3`, and level three makes zero record queries;
- each broadleaf, conifer, or acacia proxy is clipped independently to its
  16-by-16 tile;
- the representative level-one tile has 16 point-summary cells, one record
  query, two occurrences, 48 proxy vertices, and 216 proxy indices;
- the Java-overworld payload remains pinned at 100 vertices, 150 indices, and
  fingerprint `0x5c1c22da08c489e6`; and
- native pixels and settle coverage pass, but those results do not prove the
  coarse vegetation representation or justify the chunk-tile architecture.

### Slice 5: representation decision and closeout

- [ ] Reconcile the measured Terrain Lab hierarchy into the topic and product
  documentation.
- [ ] Choose whether a future in-game LOD system consumes these summaries and
  records directly, needs a different terrain hierarchy, or should omit
  individual vegetation beyond a nearer distance.
- [ ] Decide explicitly whether to retain, quarantine behind compatibility
  scope, replace, or revert `b1db968c`.
- [ ] Open a separate in-game terrain-LOD tactical if architecture changes are
  warranted; do not continue them inside this Terrain Lab proof.

Gate: Terrain Lab establishes the representation first; in-game architecture
follows as a separate decision.

## Commit Plan

1. Record this tactical and the surface-only baseline.
2. Add shared summary and bounded preview record products.
3. Add Terrain Lab instanced proxy presentation and diagnostics.
4. Record the redirection and quarantine the landed Far LOD adapter.
5. Prove footprint summaries, hierarchy, budgets, and cache behavior at
   65.5 km.
6. Close Terrain Lab evidence and reconcile living docs.
7. Reconsider in-game terrain LOD separately.

Every implementation commit uses:

```text
Topic: lod-native-vegetation
```
