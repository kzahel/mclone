# Tactical 333: Mclone Overworld V3 Terrain Reset

Status: **implementation active 2026-08-23. The independent source,
target-only exact lowering, direct shared procedural compiler integration,
real selectable-profile integration, first landform-density correction, and
ordinary exact/LOD forest language are complete. Live composed pixels now
prove quiet spawn country, grounded forest margins, and a mostly bare
snow-capped massif. Platform validation and Human Review V3-A remain.**

Topic: `mclone-overworld-v3-terrain-reset`
Topic: `world-generation-profiles`
Topic: `procedural-horizon-clipmap`

## Instruction Synthesis

Mclone Overworld V1 has attractive distant silhouettes but weak walking-scale
terrain. V2 proves useful negative space, clearings, continental context,
catchments, causal ecology facts, and one exact/procedural source, but its
ordinary lived terrain is dominated by broad quiet surfaces. Its rare mountain
proof does not establish a persistent terrain language, and regeneration plus
retained movement remain materially more expensive than V1 on CPU-authored
paths.

Create a new experimental Mclone Overworld V3 rather than continuing to tune
V2 in place. Do not fork-copy V2's implementation. Reuse shared profile,
generation, persistence, exact/LOD composition, renderer, and platform
contracts while giving V3 a new terrain source whose causal order is:

```text
continental context
  -> strong lived-scale landform backbone
  -> valleys, basins, and drainage opportunity
  -> quiet country and clearings
  -> substrate and ecology response
  -> exact blocks and direct procedural LOD
```

Commit as implementation proceeds. Preserve V1 and V2 as selectable controls
until V3 earns a later retirement or default decision.

## Product Decision

Allocate `mclone-overworld-v3` as a new internal, experimental profile with a
new shared source. V3 is not a copy of V2 and does not inherit V2's current
continental surface evaluator as its terrain foundation.

- V1 remains the normal new-world default and a useful distant-silhouette and
  performance control.
- V2 remains selectable during this tactical as the negative-space,
  catchment, regional-ecology, and CPU-cost control.
- V3 may reuse independently useful V2 mechanisms through narrow contracts,
  especially bounded catchments, exact/procedural identity, and semantic
  ecology outputs. It must not call the complete V2 point evaluator and then
  add another height layer on top.
- V3 output is internal and unshipped. Its profile rules, source revision,
  fixtures, and disposable worlds may change during review.
- Making V3 the default, retiring V1 or V2, or promising cross-build V3 save
  compatibility is explicitly deferred.

## Product Question

Can one shared, directly queryable terrain source combine memorable mountains,
massifs, plateaus, valleys, basins, and quiet clearings at useful travel scales,
remain attractive from walking distance through continental LOD, and refill at
approximately V1-class cost?

## Failure Boundary

This tactical treats the following as the V2 surface failure to replace:

- continental, province, ecoregion, and mosaic ownership cells are useful
  planning context but are too large to serve as the primary lived terrain
  grammar;
- most ordinary V2 height comes from broad low-amplitude fields, while large
  mountain lift is gated to a special catchment range;
- ecology and regional-archetype weights participate too early in terrain
  shape, so clearings and vegetation identity are stronger than landform
  silhouette;
- V2 point queries reconstruct a large semantic and hydrographic bundle even
  when an exact or procedural caller needs a compact surface answer; and
- the LOD renderer correctly removes unrepresentable local frequency, exposing
  that the surviving V2 macro silhouette is usually quiet rather than
  impressive.

The tactical does not treat the fixed-residency geometry clipmap, exact
frontier, async native compilation, movement safety, profile persistence, or
whole-tree ownership as failed architecture.

## V3 Terrain Contract

### Scale roles

V3 keeps continental context but separates it from lived landform authority.
Initial scale bands are hypotheses to review, not compatibility constants:

- `16-64 km`: land/ocean and broad climatic context;
- `2-12 km`: persistent range, massif, plateau, basin, lowland, and major
  valley compositions;
- `256 m-2 km`: branches, shoulders, escarpments, piedmont, secondary valleys,
  broad clearings, and forest/open-country mosaics; and
- `8-256 m`: walking-scale ridges, benches, gullies, hummocks, local quieting,
  and substrate response.

The middle bands own the recognizable skyline. The broadest context may guide
where a family is appropriate but must not stretch every visible landform to
continental size.

### Landform grammar

The first grammar is deliberately compact:

1. **range or massif**: a persistent oriented high axis, asymmetric faces,
   multiple peaks, branching shoulders, saddles, and a bounded foothill apron;
2. **plateau or escarpment**: coherent high interior, authored rim, broken
   faces, benches, and a low apron rather than another rounded hill;
3. **valley or basin**: broad negative counterform tied to adjacent high
   structure, with a low axis, outlet opportunity, and room for quiet travel;
4. **rolling connector**: subordinate relief between strong forms without
   filling every gap with equal hills; and
5. **plain or clearing country**: intentionally quiet, traversable negative
   space whose value comes from adjacency to stronger terrain.

Landform identities and broad summary facts remain stable across exact and
coarse queries. Finer queries elaborate the same feature rather than selecting
a different camera-dependent world.

### Causal order

Terrain and water precede ecology:

- a provisional landform backbone supplies high axes, divides, lows, passes,
  exposure, and drainage cost;
- bounded drainage or analytic valley facts may revise the final surface;
- substrate and moisture consume final terrain and water;
- openness, forest, clearings, and later creature habitat consume those
  accepted facts; and
- biome IDs remain useful runtime outputs, not the terrain generator.

V2 clearing and ecology semantics may be adapted only after V3's terrain facts
exist. Ecology must not erase mountain faces, fill valleys, or flatten a
plateau to satisfy a regional label.

### Direct query and cost shape

One V3 plan instance supports:

- a direct canonical point query for tests, spawn, and sparse consumers;
- a bounded window query that constructs or fetches compact landform facts
  once and evaluates many samples without repeating graph construction;
- a spacing-aware coarse window that preserves feature identities and
  silhouette while removing only genuinely sub-grid detail; and
- exact chunk lowering from the same spacing-one samples.

The first CPU implementation must keep its continuous field and analytic
feature evaluator simple enough for later GPU or SIMD realization. It must not
perform a full drainage traversal, allocate per point, enumerate exact chunks,
or query fine vegetation while compiling procedural terrain.

Optional caches change cost only. Point, window, cache-reset, target order,
native, and Wasm queries must produce identical canonical facts.

## First Review Landscape

The initial proof is one deterministic 8-16 km mountain-to-lowland landscape,
selected by a coordinate-independent scan rather than a coordinate-specific
generation branch. It must contain:

- a range or massif with more than one summit, a recognizable axis, at least
  one saddle, different face character, and a substantial elevation span;
- a broad valley or basin whose low country reads as related counterform;
- a plateau, escarpment, or secondary high form distinct from the main range;
- meaningful plains and forest clearings rather than continuous roughness;
- one plausible source-to-lowland drainage opportunity, with V2's bounded
  catchment mechanism reused only if it fits the new backbone cleanly;
- walking-scale terrain that is more than a smooth height ramp; and
- a skyline that remains unmistakable in 512-block, 8,192-block, and
  65,536-block review views.

The review package must include the same location in exact-only,
horizon-only, composed, natural, height, slope, landform-identity, and
coverage views. A dramatic locator chosen far away does not excuse a dull
spawn or ordinary journey distribution.

## Spawn And Distribution Policy

V3 spawn selection is a product-quality query, not merely a dry-column test.
The selected safe area should have:

- traversable local slope and a valid dry standing surface;
- nearby quiet or clearing country;
- visible or reasonably reachable strong terrain;
- no unavoidable water or cliff trap; and
- bounded selection cost from coarse V3 summaries before exact chunks exist.

Permanent distribution witnesses must measure landform-family coverage,
feature span, relief, slope, saddle/pass presence, quiet-space coverage, and
distance from representative safe starts to strong terrain. A hand-picked
mountain screenshot is insufficient.

## Ownership

- `mclone-worldgen` owns V3 landform identities, compact plans, direct/coarse
  sampling, exact lowering, substrate/biome outputs, deterministic witnesses,
  and source-specific performance receipts.
- `mclone-server` owns the stored V3 profile identity, generation dispatch,
  spawn admission, scheduler integration, and persistence behavior.
- `mclone-terrain-view` owns V3 source selection, fixed-budget procedural
  compilation, exact composition, mono/per-eye/multiview presentation, and
  retained movement diagnostics.
- `mclone-scene`, `mclone-app-runtime`, and `mclone-ui` own shared live-game
  orchestration and profile selection. Platform apps remain adapters.
- Terrain Lab and World Explorer own review controls and diagnostics only.

## Performance Gates

Use matched release workloads and report work as well as elapsed time.

1. **Surface sampling:** ordinary and strong-landform V3 windows compare with
   V1 and V2 at spacing 1 and representative LOD spacings.
2. **Exact generation:** cold and retained/warm 5-by-5 windows report target,
   dependency, non-air-block, cache, and elapsed facts.
3. **Procedural refill:** cold fill and a retained entering strip report tile
   submissions, CPU compile time, ready latency, stale work, render tails, and
   fixed residency.
4. **Steady presentation:** native and Quest report complete exact and horizon
   readiness before frame percentiles are accepted.
5. **Browser:** Wasm uses the same semantic compiler contract and must not hide
   an unbounded main-thread refill behind eventual completion.

Initial target: V3 warm exact and retained procedural refill should be within
25% of matched V1 on the review host, with no stable frame or convergence
regression over V1's accepted envelope. A miss may be reviewed if the visual
gain is decisive and the measured owner has a bounded recovery path. A result
near V2's current 2-3x warm/refill cost does not pass merely because it is
bounded.

## Phases And Commit Gates

### Phase 0: Tactical, Baselines, And Source Boundary

- Commit this tactical and append the new commit topic before implementation.
- Record fresh V1/V2 exact and surface baselines at ordinary and jungle sites.
- Allocate V3 as internal-mutable in the compatibility ledger but do not make
  it the default.
- Define the V3 point/window/coarse-window types without copying the V2 sample
  bundle.
- Commit the tactical separately from source implementation.

### Phase 1: Shared V3 Landform Source

- Implement the compact landform grammar and direct/window/coarse queries in
  shared Rust.
- Add deterministic feature identities, family weights, high/low axes,
  landform summaries, substrate, openness, forest opportunity, and surface
  height.
- Add native/Wasm-capable unit tests for point/window equivalence, reordered
  queries, negative coordinates, bounded work, cache reset, feature continuity,
  and spacing-aware silhouette retention.
- Add one deterministic review-site selector and distribution receipt.
- Commit before profile or renderer integration.

### Phase 2: Exact Lowering And First Pixels

- Lower V3 spacing-one samples into ordinary generated chunks with water,
  surface strata, biome IDs, heightmaps, and bounded feature dependencies.
- Prove partition and request-order independence.
- Render the first exact-only mountain, valley, and clearing frames and inspect
  them before proceeding.
- Record cold/warm exact generation beside V1 and V2.
- Commit the exact vertical slice.

### Phase 3: Procedural LOD And Composition

- Add V3 to the shared terrain preview/source enum and procedural compiler.
- Feed every clipmap spacing from the V3 coarse-window query while preserving
  feature identity and major silhouette.
- Compose exact V3 chunks with the existing complete frontier and vegetation
  ownership contracts; do not add a V3 renderer fork.
- Capture and inspect horizon-only and composed views at walking, regional,
  and continental scales.
- Measure cold fill and retained movement before profile promotion.
- Commit shared LOD integration.

### Phase 4: Selectable Persisted V3

- Allocate the shared server profile label, stable internal binary tag,
  catalog/UI entry, launch codecs, dedicated-server parsing, worker dispatch,
  persistence reopen, scene source identity, and platform build boundaries.
- Implement quality-aware spawn selection from bounded V3 summaries.
- Reach a real locally playable V3 world through the ordinary integrated
  authority while V1 remains the default and V2 remains selectable.
- Commit profile integration.

### Phase 5: Performance Recovery And Human Review

- Remove repeated planning, allocation, and CPU compilation at the measured
  owner without weakening terrain or shrinking the view.
- Run matched V1/V2/V3 exact, refill, movement, memory, and complete-frame
  comparisons on the same host.
- Run native, headed WebGPU, flat Android, and XR build/render boundaries
  affected by the source and profile enum.
- Package sequential native/browser/live review artifacts outside the
  repository and inspect every required pixel milestone.
- Update living worldgen, profile, macro-planning, and LOD topics.
- Stop for Human Review V3-A before adding more ecology, structures, caves, or
  regional archetypes.

## Automated Acceptance

- V1 remains the default; V1 and V2 profile identities and output remain
  unchanged.
- V3 has one profile/source identity across native, Web, dedicated, Android,
  XR, persistence, exact, and procedural paths.
- Point, window, coarse-window spacing one, and exact chunks agree on surface,
  water, substrate, biome, and landform identity.
- Coarse queries retain major range, massif, plateau, basin, and valley
  identities without sampling hidden exact chunks.
- V3 contains persistent strong terrain and intentional quiet space across a
  bounded multi-seed distribution, not only at the review coordinate.
- Exact/procedural composition has complete spacing-one support and no source,
  material, water, or ownership mismatch accepted as a terrain seam.
- Cold teleport and retained movement perform bounded work independent of
  travel distance.
- No cache, Worker schedule, request order, or presentation LOD changes
  canonical V3 terrain.

## Human Review V3-A

Review V1, V2, and V3 side by side at the same seed and comparable view
descriptors. Judge:

1. Does V3 have a memorable skyline and persistent terrain axes rather than
   isolated scalar hills or rare landmark stamps?
2. Are its plains and clearings meaningful negative space rather than most of
   the world?
3. Does the same place remain attractive and navigable from block scale
   through continental LOD?
4. Do valleys, basins, and water opportunity relate visibly to neighboring
   high terrain?
5. Is regeneration and movement cost close enough to V1 that the terrain can
   remain enabled as an experiment?

The decision is **continue**, **revise**, or **reject**. Continue authorizes a
later ecology and regional-breadth campaign; it does not make V3 the default.

## Non-Goals

This tactical does not:

- merge V1, V2, and V3 or migrate existing worlds;
- preserve current V2 surface output inside V3;
- implement a complete erosion simulation, tectonic plate simulation, or
  arbitrary geological history;
- implement all future biomes, ecology, structures, caves, arches, or
  volumetric formations;
- make the renderer choose canonical geography;
- increase exact render distance or hide transitions with fog; or
- accept a beautiful hand-picked mountain while ordinary starts remain flat.

## Execution Record

### Phase 1: Shared V3 Landform Source

The first independent source is
`mclone-worldgen::mclone_overworld_v3`, currently at internal-mutable schema
`mclone-overworld-v3-terrain-v6`. It does not call
`ContinentalSurfacePlan`. Eight stable gradient fields provide broad context
and subordinate detail while each point examines a fixed 3-by-3 neighborhood
of 3,072-block analytic landform owners. Range, plateau, basin, rolling, and
plain recipes publish stable identities plus explicit high-axis, saddle,
escarpment, valley, clearing, openness, substrate, and forest-opportunity
facts.

The exact point and spacing-aware window contracts are implemented. Coarse
queries retain feature, water, and substrate identity while progressively
removing rolling, local, walking, and micro frequencies. A 131-kiloblock
distribution test requires nontrivial strong and quiet land coverage and high
terrain. Point/window equivalence, query order, cache-free repeat, negative
coordinates, invalid bounds, coarse identity, and refinement tests pass.

The coordinate-independent review selector scans the fixed owner corpus and
chooses an ordinary generated range. At seed `12345` it currently selects
`(5179, 2074)` with `218.4` blocks of sampled relief (`Y29.6..248.0`),
alongside nonzero high-axis, saddle, valley, and openness facts. The receipt is
written outside the repository by `mclone_overworld_v3_review`; the accepted
tuning receipt is `/tmp/mclone-overworld-v3-review-v5.json`.

The complete `mclone-worldgen` library suite passes: 487 passed, one ignored.

### Phase 2: Target-Only Exact Lowering

`McloneOverworldV3ExactGenerator` retains one shared terrain plan and requests
one contiguous 16-by-16 spacing-one window for each target chunk. It lowers
those 256 samples directly into bedrock, stone and surface strata, water,
heightmaps, and canonical biome payloads. At this checkpoint V3 had no
decoration stage, so full generation had no feature halo: its work receipt
recorded 256 samples, 2,304 fixed landform-owner evaluations, 2,048 field
evaluations, and one exact target. Phase 6 adds target-clipped vegetation
without changing those terrain-source work counts.

Focused tests prove source/block/water/biome agreement at ordinary and selected
mountain coordinates, direct adjacent-boundary lowering, and request-order
independence. The shared profile performance receipt now compares V1, V2, and
V3. On the development M4 Pro, seven release iterations over 25 origin chunks
produced these elapsed totals:

| profile | surface | cold exact | repeated-target exact |
| --- | ---: | ---: | ---: |
| V1 | 129.8 ms | 308.9 ms | 35.3 ms |
| V2 | 120.6 ms | 301.8 ms | 80.8 ms |
| V3 | 68.8 ms | 67.7 ms | 68.0 ms |

At the selected V3 mountain center the corresponding V3 totals are 59.8,
57.9, and 60.0 ms. V3's cold target-only path is already substantially below
both controls. The repeated-target comparison remains open: V1 and V2 reuse
cached input surfaces in that lane, while V3 currently recomputes target
columns. The later server/compiler-session integration must establish whether
ordinary persistence caching makes that difference irrelevant or whether V3
needs a bounded source/chunk cache before Human Review V3-A.

Exact rendered pixels remain the next Phase 2 milestone; this lowering commit
does not claim visual acceptance. With the exact tests included, the complete
`mclone-worldgen` library suite passes: 491 passed, one ignored.

### Phase 3: Direct Procedural Source And Failed Pixel Gate

`TerrainPreviewProfile::McloneOverworldV3` now carries the V3 schema identity
through the shared preview carrier, CPU clipmap compiler, terrain source
identity, canonical exact compiler, native/Wasm vegetation codec boundary,
profile uniforms, and World Explorer native/browser source parser. Each
procedural tile makes one spacing-aware V3 window query including the existing
stitched-normal halo. Spacing one has exact source, water, material, and biome
agreement tests. V3 publishes forest-opportunity mass but requests no proxy
tree records until an ordinary exact producer exists.

The first release World Explorer captures were produced and inspected under
`/tmp`. They are failure evidence, not review artifacts:

- the ordinary origin horizon renders, but reads as broad green facets with
  large black or uncovered-looking regions rather than a resolved landform;
- the selected mountain center does not produce meaningful horizon depth in
  the horizon-only capture at either one- or eight-kiloblock framing;
- a 17-by-17 exact composition at the selected center renders a finite exposed
  volume with a broad snowy/stone interior, but reads as a cutaway box rather
  than a mountain; and
- closer composed views show visible exact/procedural rectangular ownership,
  a straight gray connector or boundary strip, and black gaps.

The selected source still passes numeric relief and coarse-identity gates, so
these pixels demonstrate that those gates are insufficient. Do not mark Phase
2's first-pixel milestone or Phase 3's composition milestone accepted. The
next live-profile slice must distinguish review-host presentation defects from
source defects, then revise the terrain grammar or composition contract based
on inspected live pixels. No ecology or regional-breadth work is authorized by
these frames.

After this integration the complete `mclone-worldgen` suite passes 493 tests
with one ignored, the complete `mclone-terrain-view` suite passes 158 with one
ignored, and `cargo check --workspace` passes (pre-existing dead-code warnings
remain in the dedicated server and Terrain Lab).

### Phase 4: Selectable Profile And Quality-Aware Spawn

`WorldGenerationProfile::McloneOverworldV3` now crosses the shared catalog,
server plan and worker, native/Web startup codecs, persistence codec, scene
source identity, offscreen client, and CLI. It is selectable after V2 with
stable internal binary tag `10`; V1 remains the default. V3 worker sessions
retain one seeded exact generator and accept target-only requests with no
seeded dependency chunks or manufactured cache diagnostics.

Spawn selection now makes one bounded 65-by-65 coarse source query over a
16,384-block review square. It scores dry open candidates with low local slope
against uphill relief and strong landform facts 256-512 blocks away, then uses
the ordinary loaded-chunk safe-surface gate. A candidate must remain below
Y100, outside the range/plateau/escarpment body, and have at least 120 blocks
of nearby uphill relief. Tests cover three signed seeds, deterministic
selection, source quality, exact generation, and safe loaded admission.

That gate exposed and corrected a shared terrain semantic defect: uncovered
space normalized to a `Plain` weight but retained zero internal plain and
openness strength. V3 schema revision 2 now treats space between explicit
features as intentional quiet plain country. This is an intentional
internal-mutable source change, not a relaxation of the spawn criteria. The
complete worldgen suite reaches 494 passed and one ignored; focused server,
catalog, scene, Web codec, and persistence tests pass. Live-world pixels and
stored reopen evidence remained open at that checkpoint.

The ordinary SQLite lane now generates both experimental profiles at their
selected spawns, shuts down, reopens, republishes byte-identical chunks with
`LoadedFromStore` residency, and retains the requested profile. The Web catalog
descriptor independently round-trips V3's persisted label; a real IndexedDB
browser reopen remains for platform closeout.

The first native live V3 capture exposed a separate LOD-admission defect. V3
correctly requests no proxy-tree records yet had constrained the shared
vegetation spacing bound to one, causing High's spacing-four coordinator
configuration to reject the entire terrain-view engine. V3 now retains the
ordinary preset bound while its profile-specific predicate continues to
request no record jobs; scene construction omits the unused vegetation
executor for such profiles. The repaired live capture reaches all 160 logical
High slots with no pending CPU or vegetation work, no composition errors, and
a complete spacing-one exact frontier.

Those repaired pixels still failed product review. At seed `12345`, both
origin and the then-selected spawn `(-160, 32)` were overwhelmingly flat, the
nearby strong form was only a small horizon sliver, and the procedural surface
had visible sky breaks at the near/far composition. This separated the
original missing-horizon host defect from the remaining source
scale/distribution and presentation defects.

### Phase 5: Lived-Scale Density Correction

The first source scale used 8,192-block ownership cells and landforms several
kiloblocks long. It could satisfy numeric relief tests while presenting only a
thin skyline from ordinary ground. Schema revision 5 contracts ownership to
3,072 blocks, keeps the same fixed nine owner evaluations per sample, and
uses compact ranges 1,100-2,200 blocks long and 300-650 blocks wide with
140-220 blocks of lift. Plateaus, basins, connectors, and explicit plains were
contracted in parallel. The existing quieting fields still create broad
walking-scale negative space rather than roughening every column.

The seed `12345` quality spawn is now chunk `(-96, -128)`, centered on open
ground near Y80. Its southeast coarse witness rises to Y242 within 512 blocks.
Inspected live composed captures show:

- `/tmp/mclone-v3-live-spawn-v5.png`: broad rolling spawn country with the
  persistent range on the horizon;
- `/tmp/mclone-v3-v5-peak-framed.png`: the same range centered from spawn; and
- `/tmp/mclone-v3-v5-peak-near.png`: an ordinary closer view with a substantial
  brown foothill face, snow-capped summit ridge, multiple peaks, and no
  exact/procedural gap.

All 160 High-preset procedural slots were ready in the live captures, with no
pending CPU work, stale results, vegetation work, or source failure. Total V3
CPU compile work was about 0.60 seconds for the complete 192-job initial fill.
The matched seven-iteration release exact receipt at this spawn records V3
surface/cold/repeated totals of 71.5/70.9/71.4 ms. V1 records
94.7/357.3/35.3 ms and V2 records 122.8/320.3/82.8 ms in the same run. The
fixed V3 work counts are unchanged because density does not expand the owner
neighborhood.

This accepts the first mountain/quiet-country live pixel milestone. The world
was still intentionally barren at that checkpoint; Phase 6 implements forest
versus clearing presentation from the published forest-opportunity facts.

### Phase 6: Ordinary Forests, Clearings, And Grounded LOD

V3 exact revision 2 and vegetation revision 1 now derive complete stable tree
records from a global 12-block lattice. Each lattice cell makes one bounded
terrain query, admits broadleaf or conifer trees from V3 forest opportunity,
and publishes one coordinate-stable ID, silhouette, and complete bounds.
Exact chunks request records whose full bounds intersect the target and
realize only the intersecting blocks through a radius-zero feature region.
Tests prove signed-coordinate partition stability, forest/clearing response,
and exact realization without introducing a dependency halo.

The shared procedural vegetation compiler is revision 4. V3 requests the same
complete records directly at spacings one, two, and four instead of borrowing
V1's vegetation cache. Native threads and browser Workers carry the same
source identity and codec. The natural exact-mesh query consumes the same V3
records, so exact and procedural ownership select one complete tree by stable
ID.

Initial live review correctly rejected two visual defects: mountain forest
opportunity produced a nearly continuous tree cover over the main massif, and
coarse procedural proxies could float above the rendered terrain. Schema
revision 6 attenuates forest opportunity by range strength and altitude
exposure, retaining wooded lower slopes while creating a sparse upper tree
line. The shared tree shader now removes an outer proxy as one complete
instance when its base enters a finer clipmap ring, and procedural proxies
anchor their trunks to a bilinear sample of the actual resident terrain tile.

The accepted native captures are:

- `/tmp/mclone-v3-live-vegetation-v5.png`: an open, traversable foreground,
  sparse grounded exact trees, and a wooded ridge without floating proxies;
- `/tmp/mclone-v3-vegetation-peak-v5.png`: a mostly bare brown massif with a
  snow-capped summit and sparse conifers limited to its lower shoulder.

The accepted spawn frame draws 43 procedural tree instances from four ready
spacing-two tiles. All 160 terrain slots are ready and the frame contains no
source failure or floating proxy. The matched seven-iteration release receipt
over 25 spawn chunks, including exact vegetation, records:

| profile | surface | cold exact | repeated-target exact |
| --- | ---: | ---: | ---: |
| V1 | 124.3 ms | 346.4 ms | 33.1 ms |
| V2 | 119.6 ms | 308.4 ms | 78.6 ms |
| V3 | 66.8 ms | 67.7 ms | 70.3 ms |

The receipt is `/tmp/mclone-overworld-v3-vegetation-perf-v1.json`. Exact V3
vegetation therefore does not regress the target-only cost envelope and
remains far below both controls for cold regeneration. Focused V3 worldgen,
terrain-vegetation, server generation/spawn/persistence, and the complete
terrain-view library suite pass. Platform package and browser reopen closeout
remain before Human Review V3-A.

## Related

- [`328-mclone-overworld-v2-regional-breadth.md`](328-mclone-overworld-v2-regional-breadth.md)
- [`329-v2-quest-performance-and-forest-continuity.md`](329-v2-quest-performance-and-forest-continuity.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
