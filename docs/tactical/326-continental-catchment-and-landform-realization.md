# Tactical 326: Continental Catchment And Landform Realization

Status: **authorized 2026-08-21; Phase 1 graph foundation implemented. The
reported mountain-navigation crash has not reproduced in the old candidate's
static or full movement/teleport smokes, so the retained regression must be
repeated against the substantially higher realized range in Phase 2.**

Topic: `continental-hydrography`
Topic: `continental-ecoregion-planning`

## Instruction Synthesis

Human Review C retained the detached candidate's continent-scale direction but
rejected its present surface as final. Lake shorelines are abrupt and nearly
constant-height, mountain and snow destinations can crash during navigation,
the terrain lacks recognizable mountains and persistent features, and rivers
and streams are absent as a connected system. Existing water-system research,
accepted bounded-stream work, and rejected river attempts should inform the
correction rather than being rediscovered.

Build one bounded mountain-to-lake catchment in the detached continental
source. Co-author ranges, divides, valleys, tributaries, confluences, trunk
river, lake rim/spill/outlet, and varied shore morphology. Feed exact terrain,
the current procedural-horizon LOD, and downstream ecology semantics from the
same compact facts. Fix the navigation crash first, inspect pixels at every
drawable milestone, commit coherent phases, and stop at Human Review D before
changing `mclone-overworld-v1`.

## Product Question

Can one deterministic, bounded, directly queryable catchment create a
recognizable 16-32 km mountain-to-lake journey—with real ranges, converging
valleys, streams, a main river, floodplain, varied lake shore, and outlet—while
remaining cheap enough for arbitrary exact and LOD queries?

## Boundaries

This is a revision of the existing detached
`continental-ecoregion-candidate-v1`, not a new product profile. The ordinary
unbounded plane remains the main proof topology. One supported large cylinder
must repeat exactly at its seam, but this tactical does not select a product
cylinder period.

Shared ownership remains:

- `mclone-worldgen` owns catchment graph construction, direct point queries,
  terrain/water realization, exact lowering, semantics, witnesses, and review
  site selection;
- `mclone-terrain-view` owns procedural-horizon sampling, composition,
  frontier ownership, and scale-aware presentation;
- World Explorer selects sources, review sites, cameras, and diagnostics but
  owns no geography; and
- production `mclone-overworld-v1`, persistence tags, startup defaults, saved
  worlds, and animal simulation remain unchanged.

The plan overlay is diagnostic evidence. This tactical may not stop for human
review before real exact and composed terrain exists.

## Selected Mechanism

Use a bounded, feature-owned generative catchment graph rather than a complete
elevation raster or an independent contour field. A canonical owner and small
owner neighborhood reconstruct compact reach and basin records. Every record
has an explicit influence bound, so a point query does not walk upstream or
build the complete catchment.

The first typed facts are:

- catchment identity and bounded extent;
- range axis/branches, divides, peaks, saddles, passes, and basin rim anchors;
- directed headwater, tributary, trunk, lake-inlet, and outlet reaches;
- confluence identities, downstream reach, reach order, synthetic discharge,
  endpoint levels, width/depth envelopes, and current direction;
- lake boundary/rim intent, floor, flat water level, spill saddle, outlet, and
  open/closed classification; and
- analytic range, valley, channel, bank, floodplain, shoreline, and receiving
  outlet profiles.

Graph direction and levels are established before local realization. Static
water is flat within a lake or integer reach; different reaches meet through
explicit drops or compatible confluences. Ecology derives riparian, wetland,
crossing, and corridor facts from this graph instead of supplying a river
mask.

## Five Review Sites

Select deterministic sites from the actual catchment:

| Site | Required visible story |
|---|---|
| pass-and-headwaters | persistent range silhouette, saddle/pass, at least two small descending source valleys |
| tributary-confluence | two independently legible valleys and channels joining one wider downstream reach |
| trunk-and-floodplain | ordered main river, changing channel width/depth, asymmetric bank or terrace, lower quiet ground |
| lake-shore-and-outlet | flat lake water with varied banks/shelves/materials and an explicit visible spill/outlet |
| quiet-lowland-control | ordinary traversable lowland without a forced ridge, trench, or dense micro-noise |

Sites are catalogued observations of arbitrary generator output, not showcase
recipes or coordinate-specific terrain branches.

## Phases And Commit Gates

### Phase 0: Review Decision, Crash, And Tactical

- Record Human Review C as **revise**, retaining the continental hierarchy but
  rejecting the current lake, mountain, and pseudo-river realization.
- Reproduce the high-relief/snow navigation crash with the narrowest existing
  command-driven Explorer or offscreen harness.
- Add a regression that covers the identified coordinate, height, allocation,
  or camera failure; correct it without weakening bounded admission.
- Commit the topic/tactical before implementation and the crash correction as
  its own milestone.

### Phase 1: Catchment Graph And Witness

- Add the shared graph types, stable identities, canonical owner selection,
  bounded influence index, and direct point-query result.
- Construct one directed source-to-lake-to-outlet family with tributaries and
  at least one confluence. Prove acyclicity, monotonic reach levels, unique
  downstream ownership, lake spill consistency, and bounded graph size.
- Prove point/window, reordered, partitioned, cache-reset, negative-coordinate,
  native/Wasm, and supported-cylinder seam equivalence.
- Measure cold point, bounded window, and review-site search separately. No
  exact chunks or LOD tiles may be generated by graph construction.
- Add a diagnostic graph receipt or atlas only as implementation evidence.
- Commit.

### Phase 2: Range, Valley, And Water Realization

- Replace scalar upland bumps inside catchment influence with analytic range
  axes/branches, peaks, saddles, fronts, foothills, and protected basin form.
- Carve coherent headwater and tributary valleys that broaden toward the
  trunk; realize confluence, trunk valley, floodplain/terrace, lake basin, and
  outlet in the same height source.
- Reuse Tactical 222's monotonic static-water/drop principles and Tactical
  225's morphology vocabulary without copying rejected local repair logic.
- Replace thresholded lake clipping with basin/rim/spill geometry and varied
  depositional, wetland, ordinary, gravel, rocky, inlet, and outlet shores.
- Keep subordinate noise subordinate and preserve a quiet lowland control.
- Capture and inspect the first exact-only frame at every review site.
- Commit.

### Phase 3: Exact, LOD, And Ecology Semantics

- Extend `ContinentalSurfaceSample` or a sibling shared query with stable
  catchment/reach/basin IDs, reach order, discharge, signed channel relation,
  water level, downstream vector, shore intent, and wetland/floodplain facts.
- Lower exact water, beds, banks, substrates, and existing candidate trees
  from those facts without a second hydrology decision.
- Make major reaches and valleys legible through direct coarse LOD queries;
  preserve narrow-water crossing summaries where sparse sampling would miss
  the channel. Small water may disappear only after its valley remains.
- Derive riparian and wetland ecology semantics from hydrology. Do not add
  current physics or animal migration in this tactical.
- Prove direct/exact agreement and complete exact/LOD frontier ownership at all
  five sites. Capture and inspect exact-only, coverage, composed, and broad
  horizon frames.
- Commit.

### Phase 4: Movement, Performance, And Human Review D

- Exercise cold teleports to all five sites and retained movement along the
  catchment. A query may not build work proportional to travel distance.
- Measure graph/sample, exact generation, meshing, vegetation, clipmap refill,
  and presented-frame costs separately on the established native and headed
  browser review lanes.
- Correct obvious repeated construction without making cache contents
  semantic. Include at least one cache-reset repeat.
- Package a sequential review index outside the repository, update living
  topics and this execution record, push and deploy the exact revision, verify
  public pixels, then stop at Human Review D.
- Commit.

## Automated Acceptance

- The catchment graph has finite declared bounds and no cycles.
- Every non-outlet reach has exactly one downstream target; every source
  reaches the selected lake and every open lake reaches its outlet.
- Reach bed levels never rise downstream except across no transition; flat
  source-water reaches and explicit drops remain statically closed under the
  existing wake checks where applicable.
- Direct point facts are invariant under query order, window partition,
  thread schedule, cache reset, negative coordinates, and topology lift.
- Exact and procedural samples agree on quantized height, water occupancy,
  water level, material, biome, stable tree bases, and new hydrology identities
  at common coordinates.
- Exact and composed rendering report complete frontier and tree ownership at
  every review site.
- LOD cold teleport and pan perform bounded point/window work with zero exact
  generation inside the procedural source.
- Native and Wasm deterministic witnesses match.

## Execution Record

### Phase 0: Review Decision And Crash Baseline

Started on 2026-08-21.

- Human Review C is recorded as revise in Tactical 325 and the living
  continental topic. The new `continental-hydrography` topic consolidates the
  accepted/rejected river, stream, shoreline, basin, planning, and
  representation evidence.
- The existing headed browser composition runner reached the upland/arid site
  with 81 exact chunks, ten committed LOD levels, complete tree ownership, and
  no panic or device loss.
- A native full smoke beginning at highland checkpoint `(12800, 19968)` then
  exercised retained pans, negative coordinates, and a cold teleport to
  roughly `(1000051, -1000022)`. Native-surface and offscreen lanes both
  completed with 81 exact chunks and all 160 procedural slots.
- These controls do not falsify the report. They show that height 123 in the
  old scalar candidate is not sufficient to reproduce it. Phase 2 must repeat
  the same movement and teleport path over the new higher range, and retain
  any resulting failure coordinate as the regression.

### Phase 1: Catchment Graph Foundation

Implemented on 2026-08-21.

- Added a detached `ContinentalHydrographyPlan` over 32,768-block canonical
  owner cells. Each point reconstructs exactly a 3-by-3 owner neighborhood,
  selects at most one bounded catchment, and evaluates eight compact reaches.
- One graph contains three headwaters, two tributaries, one trunk, one lake
  inlet, an explicit lake/spill, and one outlet. Stable typed IDs distinguish
  catchment, reach, lake, and spill ownership.
- Reach order, synthetic discharge, head/tail water levels, width envelopes,
  downstream identity/vector, lake level, shore intent, range/divide/saddle,
  valley, and floodplain weights are directly queryable. Point work records
  zero raster cells and zero exact chunks.
- Focused tests prove bounded topological order, monotonic levels, every
  pre-lake source reaching the lake, arbitrary query-order identity, negative
  owners, and exact repetition on the 196,608-block X-cylinder.
- The native/Wasm corpus covers three graphs and eleven semantic samples per
  graph. Its committed witness is
  `febe9aea251e4e6fd63127e4b6c0e4187ef76ee525ece2bcdeff25916d2880d0`.
- The graph is not yet connected to `ContinentalSurfacePlan`, exact chunks, or
  LOD. Its JSON receipt is diagnostic topology evidence, not a Human Review D
  artifact.

## Human Review D

Review the same five places in exact-only and composed exact-plus-LOD views.
The decision is:

1. **accept** the catchment and landform mechanism and authorize focused
   in-place production integration planning;
2. **revise** named graph, mountain, valley, stream, river, lake, shoreline,
   ecology-semantic, composition, crash, or performance behavior; or
3. **reject** the realization while retaining any independently useful
   bounded-query contracts.

Acceptance requires memorable causal terrain, not merely a valid graph or a
better diagnostic atlas. It does not by itself promote the product profile.

## Non-Goals

This tactical does not:

- change the product default, persistence, or internal saves;
- implement a scientifically exact unbounded watershed or erosion simulation;
- populate every continent with catchments;
- implement caves, arches, dams, deltas, braids, tides, groundwater, floods,
  sediment transport, freezing, swimming currents, boats, or fish;
- implement unloaded animal migration or alter existing animal AI;
- solve final vegetation variety or the exact/procedural water shader seam;
- select the default cylinder topology or size; or
- accept the surface based only on maps, metrics, or tests.

## Related

- [`325-continental-exact-lod-review.md`](325-continental-exact-lod-review.md)
- [`225-watercourse-morphology-and-coastal-outlets.md`](225-watercourse-morphology-and-coastal-outlets.md)
- [`../topics/continental-hydrography.md`](../topics/continental-hydrography.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/deterministic-streamed-landscape-planning.md`](../topics/deterministic-streamed-landscape-planning.md)
- [`../topics/streams-reflowing-reference.md`](../topics/streams-reflowing-reference.md)
- [`../topics/lod.md`](../topics/lod.md)
