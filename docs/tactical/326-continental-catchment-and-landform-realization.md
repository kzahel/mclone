# Tactical 326: Continental Catchment And Landform Realization

Status: **authorized 2026-08-21; Phases 0-3 implemented. One selected bounded
catchment now realizes ranges, valleys, snow/rock summits, ordered channels,
one flat lake, varied shore intent, spill, and outlet in exact terrain and the
shared direct source. The reported crash still has not reproduced, including
new high-range captures. Direct LOD queries now preserve eligible narrow-water
crossings, exact agreement covers all five review sites, and an explicit
confluence apron removes the hard three-way mound. Phase 4 owns movement,
performance, browser evidence, deployment, and Human Review D packaging.**

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

### Phase 2: Range, Valley, And Water Realization

Implemented on 2026-08-21.

- `ContinentalSurfacePlan` now consumes the bounded graph directly. Compact
  range peaks and branch ridges rise above a broader divide, the selected pass
  remains lower, and reach-order profiles establish source valleys,
  tributaries, a trunk terrace/floodplain, lake basin, spill, and outlet.
- Reaches use locally warped centerlines, ordered widths/depths, and
  side-dependent banks and terraces. Lakes own one flat level, a warped
  boundary, varied wetland/depositional/gravel/rocky/inlet/outlet slopes, and
  narrow material transitions. The lake explicitly suppresses lower reach
  trenches in its interior.
- Elevation and range ownership now expose stone and snow summit substrates;
  candidate vegetation rejects water, stone, snow, sand, and gravel bases.
  Hydrology publishes stable IDs, reach order, discharge, signed channel
  relation, water level, downstream vector, floodplain, riparian, and shore
  intent through the ordinary surface query.
- A deterministic five-site catalog selects one land-surviving catchment and
  records review centers, camera scales, direct water/identity facts, and
  sparse local relief. Native `--catchment-site` and browser
  `catchmentSite=...` selection resolve those shared records without
  coordinate-specific generation branches.
- Exact-only captures were inspected at all five sites. The trunk is a real
  10-20-block channel with an asymmetric terrace; the lake/outlet frame shows
  flat water meeting land without the former constant-height cliff; the quiet
  control remains ordinary rolling ground. The broad confluence frame shows
  two independent valleys and channels meeting the trunk.
- A composed confluence capture exposed a rectangular height mismatch at the
  exact/procedural frontier where sparse LOD interpolation crosses narrow
  valleys. This is retained as the first Phase 3 correction rather than
  accepted as Phase 2 presentation evidence.
- Static and composed captures over the new 200-plus-block range completed
  without panic, device loss, or buried-camera failure. Orbit target selection
  now accounts for both viewer and focus surfaces; movement/teleport stress is
  still required in Phase 4.
- The revised hydrography native/Wasm witness is
  `8010fe5dbb1b6bb8ea344d1a003d4ff822ef7773c34121ca4f903c25ffce2ca6`;
  the integrated surface witness is
  `283e63b08aec8fcdbb32f3aea8cc89fabc7722afddec1b2e67818e84d6c8e55f`.

### Phase 3: Exact, LOD, And Ecology Semantics

Implemented on 2026-08-21.

- The surface query now publishes the final locally warped signed channel
  distance rather than the graph skeleton's pre-realization distance. Exact
  beds, banks, water, riparian suitability, and LOD presentation therefore
  use one centerline fact.
- Coarse direct compilation retains ordinary point samples at spacing one.
  For footprint spacings above one it may issue at most two perpendicular
  direct probes when an eligible reach crosses between lattice vertices.
  Headwaters retain water only through spacing 16, tributaries through 64,
  and trunk, inlet, and outlet reaches through 256. Broader valley geometry
  remains directly sampled after narrow water disappears.
- The preview carrier now publishes actual signed distance, channel width,
  channel influence, bank influence, and hydrology-derived riparian intent
  instead of reusing the older ecological route scalar as pseudo-river data.
- Matched horizon-only, exact-only, coverage, and composed confluence captures
  showed the reported box in both terrain representations while frontier
  ownership was complete and connector segments were zero. The cause was a
  hard angular three-way valley join, not an exact/LOD seam. Stable confluence
  identity, distance, floor, and ecology weight now drive one subtly warped
  low apron shared by exact and LOD terrain.
- A new five-site exact receipt compares 32,000 columns, 128,000 biome cells,
  water, material, stable tree bases, and direct catchment, confluence, reach,
  and lake identities. It passes with semantic witness
  `08746e12b1b756e7a1efef3463a18805767a0fa135a0bff2982e34ac97031e15`.
- The hydrography native/Wasm witness is now
  `ba43bdf5a7d43292eddaa52aec9fd64d895433b2576ac1e3b23d1ff359e64cc1`;
  the surface witness is
  `0760b0f2bda0c798d21bee7be873f1255c30b28dbd863967046cd56f15f68c84`.

### Phase 4: Movement, Performance, And Human Review D

In progress on 2026-08-21.

- Surface windows now retain a bounded query-local catchment cache. A
  21-by-21 tile halo reconstructs each intersecting graph once rather than
  once per lattice point, while point facts and cache-reset output remain
  exact. Cache contents never escape the request.
- The same cold native confluence composition improved from 7,259 ms to
  6,404 ms target readiness on the Apple M4 Pro debug lane. Exact generation,
  meshing, and vegetation remained approximately 371 ms, 990 ms, and 92 ms;
  the remaining clipmap/direct-source cost still dominates review startup.
- The cost-only surface contract is now `mclone-continental-surface-v8` with
  native/Wasm witness
  `255ecfc364ffb5908be17695aa70d4d178a74cd1c46dfedeccf0309857ff6554`.

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
