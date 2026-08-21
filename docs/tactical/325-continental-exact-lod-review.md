# Tactical 325: Continental Exact And LOD Review

Status: **implemented through Human Review C on 2026-08-21; awaiting human
review.**

Topic: `continental-ecoregion-planning`

## Instruction Synthesis

The continental candidate's macro organization is reasonable enough to
continue, but Human Review B did not accept the surface. World Explorer still
reads primarily as abstract colors and proxy cover, and its current reference
refill performance is poor. The next evidence must be ordinary exact voxel
terrain joined to the same procedural-horizon LOD, not another plan atlas.

Implement a bounded exact-to-LOD vertical slice at three contrasting journeys,
commit coherent milestones, and stop for Human Review C. Do not change the
product default or present the detached review source as a second long-lived
Overworld.

## Product Question

Does the accepted macro plan become memorable, plausible terrain when lowered
to real blocks, materials, water, vegetation, and walking-scale relief, while
the exact foreground and procedural horizon remain one recognizable geography
at usable streaming cost?

## Relationship To Mclone Overworld

The intended product outcome is a replacement of the macro backbone of
`mclone-overworld-v1`, not a replacement of every local generator subsystem
and not a permanent parallel planning world.

```text
future mclone-overworld-v1 descriptor
        -> continental/ecoregional plan
        -> shared surface, water, cover, and identity facts
             -> ordinary exact chunks near the player
             -> matching procedural-horizon LOD farther away
```

This tactical remains a promotion probe. It uses
`TerrainPreviewProfile::ContinentalEcoregionCandidate` to select one shared
worldgen source in the detached canonical exact compiler and World Explorer.
It does not add a persisted `WorldGenerationProfile`, a codec tag, a new-world
option, or a live server branch. If Human Review C accepts the result, a later
tactical may intentionally revise `mclone-overworld-v1` in place under the
compatibility safety ledger. If review rejects it, the detached source can be
removed without migrating worlds.

## Three Review Sites

Use the existing deterministic journey catalog and seed 12,345:

| Site | Required evidence |
|---|---|
| clearing-between-forest-cores | traversable open core, enclosing exact trees, quiet local relief, cover agreement into LOD |
| connected-water-country | exact owned water, bed/bank materials, flat lake or river surface, water agreement into LOD |
| upland-to-arid-basin | elevation transition, dry substrate strata, sparse exact cover, recognizable arid continuation in LOD |

The sites are review coordinates, not authored showcases or renderer recipes.
The same generator must work at arbitrary coordinates.

## Shared Exact Contract

`mclone-worldgen` owns candidate exact lowering. One generated column must use
the same `ContinentalSurfaceSample` that feeds LOD and publish:

- bounded solid terrain from bedrock through the quantized canonical surface;
- top and subsurface blocks derived from the typed substrate and water cause;
- explicit source water only where the shared sample owns water;
- an existing compatible biome ID for block tint and mesh presentation;
- candidate-owned deterministic vegetation records realized as ordinary tree
  blocks; and
- receipts that compare exact top, water, substrate, and cover facts with
  direct samples at the same coordinates.

Walking-scale refinement must be added to the shared surface source before
exact lowering if the current field is too smooth. Exact generation may
quantize continuous heights and realize bounded block strata; it may not add a
second unrelated heightfield, coastline, lake, route, or forest decision.

Surface and final-feature chunks use the ordinary `GeneratedChunk`, biome
payload, textured mesh, exact boundary profile, and exact/proxy ownership
formats. Candidate dependencies must be bounded and request-order independent.

## Exact-To-LOD Composition Contract

`mclone-terrain-view` continues to own composition. Extend the existing
canonical exact compiler and runtime exact renderer to preserve the selected
candidate source identity rather than hard-coding production. The existing
focus-connected exact coverage, discard mask, direct smooth frontier,
procedural water arbitration, and tree ownership must operate unchanged.

World Explorer may select journeys, exact radius, composition, and cameras. It
must not lower blocks, choose materials, synthesize trees, or special-case the
three sites. Journey selection should default to composed exact-plus-horizon
for this review, while explicit horizon-only comparison remains available.

## Performance Posture

The current 2.09-2.49 second complete 160-slot candidate refill is a measured
prototype limitation, not an accepted runtime budget. Separate:

1. direct surface sampling;
2. exact surface and feature generation;
3. exact meshing and upload;
4. candidate vegetation compilation;
5. cold clipmap completion;
6. retained movement/refill; and
7. browser Worker versus presentation time.

First make the exact/LOD result semantically and visually correct. Then remove
obvious redundant reconstruction and preserve retained work across movement.
Do not introduce a semantic cache whose contents affect geography, reduce
nearby quality based on clipmap level, or hide a full rebuild behind vague
"prototype" language.

## Phases And Commit Gates

### Phase 0: Tactical And Boundary

- Record Human Review B as conditional continuation rather than acceptance.
- Pin the detached exact compiler boundary and production non-change.
- Commit before implementation.

### Phase 1: Exact Surface Lowering

- Add candidate exact column/chunk generation in `mclone-worldgen`.
- Lower typed substrate and water facts into bounded block strata and biomes.
- Prove adjacent chunks, negative coordinates, traversal order, exact/direct
  agreement, flat owned water, and deterministic fingerprints.
- Connect Surface-stage canonical compilation and capture the first exact-only
  image. Inspect it before adding vegetation.
- Commit.

### Phase 2: Composed Exact And LOD

- Make native and browser exact executors source-qualified.
- Permit candidate exact, coverage, and composed modes without enabling them
  for incompatible sources.
- Exercise the ordinary frontier certificate at all three sites.
- Capture and inspect exact-only, coverage, and composed frames, including one
  frame where the exact boundary is plainly visible.
- Commit.

### Phase 3: Exact Candidate Cover

- Reuse the candidate's stable global vegetation records for exact blocks and
  LOD proxies.
- Preserve whole-tree exact/proxy ownership at the frontier.
- Add bounded local ground cover only if it is derived from shared candidate
  facts and materially improves walking-scale review.
- Inspect the clearing, water, and arid sites independently.
- Commit.

### Phase 4: Runtime Cost And Review Package

- Add sequential native and headed-browser three-site capture/smoke runners.
- Measure cold completion and retained movement with separate exact, clipmap,
  vegetation, mesh, upload, and frame receipts.
- Correct avoidable full-plan or full-coverage rebuilds exposed by the
  measurements without changing geography.
- Update living topics and this execution record, push, deploy the exact
  revision, verify public pixels, and stop at Human Review C.

## Acceptance Evidence

Human Review C receives, for each site:

- an exact-only walking/low-oblique frame;
- a coverage frame identifying exact ownership;
- a composed exact-plus-LOD low oblique;
- a broader composed horizon frame;
- a short retained movement step;
- direct-versus-exact height, water, material, and cover receipts; and
- cold and retained performance measurements.

Acceptance requires real terrain that reads beyond diagnostic colors,
recognizable continuity across the exact frontier, plausible water and cover,
materially different site stories, and a credible path to runtime performance.
Exact pixels alone do not authorize production integration.

## Execution Record

### Phase 1: Exact Surface Lowering

Completed on 2026-08-21.

- Added a detached `ContinentalCandidateExactGenerator` in
  `mclone-worldgen`. It retains one plane surface plan per compiler session and
  emits ordinary 0..255 `GeneratedChunk` payloads without adding a persisted
  world profile.
- Exact columns quantize the shared solid height and lower grass, coarse soil,
  sand/sandstone, gravel, exposed stone, bedrock, source-owned water, and
  compatible existing biome IDs.
- The first exact render exposed a tabletop result because the shared source
  stopped at a 512-block local field. Surface schema v4 therefore adds
  96-block walking form and 32-block micro form in the shared source, with
  clearing and wetland quieting. Both exact chunks and LOD consume those facts;
  exact lowering has no private detail heightfield.
- Focused tests cover direct-column agreement, negative chunks, adjacent
  boundaries, reordered generation, repeated fingerprints, and a flat owned
  lake with gravel bed.
- Inspected native exact-only frames:
  `/tmp/mclone-continental-exact-surface-clearing-v2.png`,
  `/tmp/mclone-continental-exact-surface-water.png`, and
  `/tmp/mclone-continental-exact-surface-arid.png`. These prove real textured
  voxel geometry and contrasting relief, but are intentionally bare before
  candidate vegetation. The water journey center itself is dry; the exact
  water review camera must use a water-owning checkpoint in the next phase.
- The first three 17x17 exact-only captures completed in roughly 2.21-2.44
  seconds on Apple M4 Pro. Exact generation was roughly 196-200 ms and exact
  meshing 1.89-2.26 seconds. Those measurements are baselines, not accepted
  budgets.

Surface witness after the shared-detail revision:
`16173c8d5504d779a7e201eb3bb2ef068f49e971e328964a1aaca4ab0af53dac`.

### Phase 2: Composed Exact And LOD

Completed on 2026-08-21.

- Native and browser exact executors now carry the selected terrain profile
  through worker initialization, canonical meshing, exact source identity,
  runtime view height, coverage, and tree-ownership validation. Existing
  default constructors retain production behavior for other hosts.
- The first composed candidate frame exposed a production-source leak in the
  fine frontier support belt as a conspicuous snowy mountain ring. Frontier
  resources and tree ownership are now profile-qualified. Because the
  candidate is a CPU reference source, its fine support tiles upload the same
  candidate reference samples and height halo instead of dispatching the
  production-only GPU sampler.
- Journey URLs now default to composed exact-plus-horizon review. Explicit
  `composition=horizon`, `exact`, and `coverage` remain available.
- Inspected radius-4 native frames for all three sites at
  `/tmp/mclone-continental-composed-clearing-phase2-v2.png`,
  `/tmp/mclone-continental-composed-water-phase2.png`, and
  `/tmp/mclone-continental-composed-arid-phase2.png`, plus
  `/tmp/mclone-continental-coverage-clearing-phase2.png`. The water frame uses
  checkpoint `(7168, -21504)`, where the shared plan owns a lake.
- A source-qualified browser Worker completed the clearing site with 25 exact
  chunks, matching coverage generation 27, no missing ownership records, and
  inspected pixels at
  `/tmp/mclone-world-explorer-web-desktop-continental-clearing-between-forest-cores-composed-review.png`.
- Radius-4 composed captures completed in 2.95-3.24 seconds. A radius-8
  candidate composition exposed avoidable CPU frontier reconstruction on each
  incremental exact admission and missed the 15-second capture deadline after
  painting 227 of 289 chunks. That is recorded performance debt for Phase 4,
  not a smaller accepted exact radius.

The central exact ground is intentionally treeless at this gate. Phase 3 must
realize and transfer ownership of the same stable candidate tree records; the
empty square is not review-ready terrain.

### Phase 3: Exact Candidate Cover

Completed on 2026-08-21.

- The stable global candidate lattice now exposes bounded whole-tree queries.
  Large mesh-input regions partition into bounded queries, deduplicate by the
  existing record identity, and include neighboring bases whose crowns cross
  an exact boundary.
- Candidate final-feature chunks clone immutable cached 3x3 surface
  dependencies, realize the existing rounded broadleaf, layered conifer, and
  forked acacia voxel archetypes, and return only the target chunk. The
  256-chunk LRU affects work only; generation always starts from undecorated
  shared surface blocks and remains request-order independent.
- Canonical exact meshing queries those same records for separated natural
  tree meshes. Existing exact/proxy ownership suppresses complete proxy
  instances rather than clipping crowns at the frontier.
- Inspected native composed frames at
  `/tmp/mclone-continental-composed-clearing-cover.png`,
  `/tmp/mclone-continental-composed-water-cover.png`, and
  `/tmp/mclone-continental-composed-arid-cover.png`. The clearing admitted two
  records with one exact and one proxy owner; the lake admitted none; the arid
  site admitted nine with eight exact and one proxy owner. All three reported
  zero missing exact or proxy records.
- Exact generation at radius 4 increased from roughly 54-57 ms to 226-249 ms
  after real feature dependencies and tree records. Meshing remained roughly
  461-571 ms. Total capture readiness remained 3.01-3.33 seconds, still
  dominated by incremental admission and candidate frontier churn rather than
  tree geometry.

No separate close-range ground-cover scatter was added. It is not needed to
establish the terrain/tree ownership contract and would distract from the
larger surface judgment.

### Phase 4: Runtime Cost And Review Package

Completed on 2026-08-21.

- Candidate admission now compiles at most four exact chunks per frame and
  rebuilds coverage, frontier, and vegetation ownership once for the batch.
  Radius 8 consequently reaches all 289 chunks in about 3.8-4.0 seconds on
  the review host instead of missing the 15-second deadline at 227 chunks.
  Production retains its existing one-chunk admission cadence.
- Rapid review movement exposed a real topology error: after the camera
  outran the retained overlap, a newly ready center could be admitted as a
  disconnected exact island. Admission now requires existing or cardinally
  adjacent ownership. A lost-overlap rebase clears only painted ownership;
  reusable compiler and resident work remain cached.
- The direct/exact harness compares a 5-by-5 exact-chunk neighborhood at each
  site. Its 19,200 columns, 76,800 biome samples, top materials, water facts,
  and tree bases have zero mismatches. The suite semantic witness is
  `6c01abd836840bd9d1df665a9cd3c33d38b9f339c5dd55635491f538cf0d42fb`.
- The water review location is selected deterministically at
  `(6816, -21184)`, near a bank rather than at the center of an open lake. Its
  neighborhood contains 840 land and 5,560 water columns, so one low-oblique
  frame shows exact water, floor, bank, land, and a crossing tree crown.
- A sequential review runner now packages direct agreement, exact-only,
  coverage, composed, and broad-horizon native frames; retained window and
  offscreen movement receipts; and three headed-browser composed receipts.
  The package uses radius 4, or 81 painted exact chunks, at every site.
- Cold radius-4 composition completes in roughly 3.6-4.0 seconds on Apple M4
  Pro. Retained movement avoids a full clipmap rebuild: three movements add
  only 36-48 refills beyond the 320-slot cold fill. The current 24 movement
  frames average roughly 27-33 ms with 57-83 ms p95. These are review-host
  reference measurements, not final live-game budgets.
- Native and browser captures report complete exact ownership, all ten LOD
  levels, and zero missing exact/proxy tree records. The final reproducible
  package is written outside the repository at
  `/tmp/mclone-continental-exact-review/review-index.json`.

The result deliberately retains two visible review issues. Procedural proxy
forests are still more regular than the exact block trees, and exact water's
translucent active-pack presentation differs conspicuously from the coarser
procedural-water presentation at the frontier. The direct/exact receipts show
that these are representation problems rather than a second geography. Human
Review C should judge whether to revise those presentations before or during
production promotion.

## Non-Goals

This tactical does not:

- modify `mclone-overworld-v1`, the new-world default, persistence, or saves;
- add a new persisted generation profile or retain the candidate as a product
  mode;
- implement caves, ores, structures, global erosion, or complete decoration;
- implement animal simulation or migration;
- solve every terrain-character family or exact site;
- select a large cylinder period; or
- declare the macro plan or surface accepted without Human Review C.

## Human Review C

The decision is one of:

1. **accept** the exact/LOD vertical slice and authorize promotion planning for
   a substantial `mclone-overworld-v1` revision;
2. **revise** named macro, local terrain, water, material, vegetation,
   frontier, or performance behavior; or
3. **reject** the surface realization while retaining or separately revising
   the plan.

## Related

- [`324-continental-surface-world-explorer.md`](324-continental-surface-world-explorer.md)
- [`313-direct-exact-to-smooth-horizon-transition.md`](313-direct-exact-to-smooth-horizon-transition.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/lod.md`](../topics/lod.md)
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
