# Tactical 324: Continental Surface World Explorer

Status: **implementation complete and awaiting Human Review B as of
2026-08-20. The shared broad surface, explicit native/browser World Explorer
source, source-qualified cover, causal arid contrast, and six deterministic
review journeys are drawable with zero exact or production-vegetation work.
Production remains unchanged.**

Topic: `continental-ecoregion-planning`

## Instruction Synthesis

Proceed from the accepted Terrain Lab continental/ecoregional plan into
navigable terrain. Begin with a deliberately small characteristic vocabulary
that already produces materially different travel stories, but keep the
system open to later alpine, glacial, volcanic, karst, dune, delta, and other
geography. Commit the tactical first, then land implementation milestones and
their evidence as separate commits until the broad three-dimensional candidate
is ready for human review.

The first review package uses five terrain-character families and six review
journeys. These are not a biome catalog and must not become five mutually
exclusive height functions. Forests, clearings, wetlands, habitat routes, and
surface cover modify compatible terrain; they do not become embossed bands or
independent continents.

## Product Question

Can the accepted top-down plan remain recognizable as compelling,
three-dimensional geography from continental overview through regional
fly-through, while direct coarse queries remain fixed-cost, deterministic, and
independent of clipmap residency or exact chunk generation?

## Objective

Add one Rust-owned candidate surface source that:

1. lowers continental, province, ecoregion, mosaic, clearing, water, and route
   facts into broad height, water, substrate, and cover facts;
2. composes five curated characteristic terrain families without averaging
   every place into the same noisy fabric;
3. runs through the shared `mclone-terrain-view` procedural-horizon source
   boundary on native and browser World Explorer;
4. keeps current field-revision-21 production terrain selectable as a control;
5. provides six matched, ordinary and signature review journeys; and
6. stops for Human Review B before any exact-chunk or new-world-default work.

## Characteristic Vocabulary

Separate two counts:

- a **terrain-character family** is a reusable surface grammar; and
- a **review domain** is a selected geographic journey that exercises several
  plan and surface facts together.

The initial five terrain-character families are:

| Family | Primary plan drivers | Required three-dimensional character |
|---|---|---|
| coast and headland | land weight, inland distance, exposure, province relief | submerged shelf, beach or bluff transition, headland variation |
| rolling interior | lowland or open province, quiet ecoregion, clearing structure | broad traversable floors, long low-amplitude rolls, negative space |
| fluvial and wetland | river/lake story, major-water opportunity, wetland and route facts | graded valley, flat water ownership, floodplain and wet shelf |
| upland and escarpment | exposed-upland role, relief, continental story | long ridge, bench, scarp, pass, and basin transition |
| arid rain shadow | leeward exposure, low moisture, basin position, drainage permanence | dry basin, rocky shelf, wash, sparse cover, and coherent desert surface |

The first four form the first drawable temperate milestone. The fifth is part
of the complete Human Review B package, but it must add coherent arid planning
facts rather than recolor or dry out a temperate heightfield.

Forest core, forest edge, clearing, wetland, and habitat-route facts are
cross-cutting realization inputs. They may change cover, roughness, substrate,
soil moisture, and bounded local relief. They may not cut a uniform trench,
raise a wooded ribbon, or override the owning surface family merely to remain
visible.

## Six Review Journeys

The candidate is not accepted from one favorable coordinate. A deterministic
review-site selector must publish stable coordinates and receipts for:

1. coast or headland into wooded interior;
2. broad clearing between forest cores;
3. a long forest-edge traverse;
4. connected river, wetland, and lake country;
5. quiet rolling ordinary interior; and
6. upland or escarpment descending into an arid rain-shadow basin.

Each journey needs a map locator, overview, oblique frame, horizon/fly frame,
named plan identities, surface-family weights, height and water ranges, and a
10-50 km sequence summary. Selection must be derived from plan facts and
bounded scans, not hard-coded renderer branches or a showcase-only world.

## Shared Surface Contract

`mclone-worldgen` owns a revisioned candidate surface descriptor and direct
point/window sampler. One point publishes at least:

- stable continent, province, ecoregion, clearing, and route identities;
- terrain-family weights and the dominant diagnostic label;
- final height plus separately inspectable continental, province, hydrologic,
  and local contributions;
- land/ocean intent, solid surface, display surface, and explicit water level;
- substrate/surface family;
- openness, forest core/edge, wetland, clearing, and route realization; and
- exact work counts proving no chunk, density volume, feature batch, or
  exploration-dependent plan was constructed.

The family blend is a curated compatibility composition, not an unrestricted
parameter soup. A new family can be added later without changing the renderer,
but new causal geography must add the necessary typed planning fact instead
of smuggling policy into presentation noise.

Point and window queries must agree exactly under traversal, partition,
threading, plane/cylinder lifts, native, and Wasm execution. Local fields are
coordinate-pure irregularization keyed by stable source and plan identity.
Camera distance, clipmap level, residency, request order, and cache warmth are
not generation inputs.

## Water Contract

Water is geometry, not a color mask:

- ocean surfaces use one stable sea level and a continuous shelf/coast
  transition;
- lake basins own a stable flat water level derived from their plan identity;
- riparian routes own a bounded, consistently graded valley/channel envelope;
- wetlands occupy shallow compatible shelves around owned water; and
- every displayed water point names its owner and agrees with the solid
  surface height.

This tactical need not solve global erosion or a full watershed. It must not
paint sloping lakes, route water uphill, or reuse the visually disappointing
multiscale reconstruction merely because it already exposes a hierarchy.

## Terrain-View And World Explorer Boundary

Expose the candidate as a named procedural source through the existing shared
terrain representation. `mclone-terrain-view` consumes packed broad-surface
facts, plans fixed clipmap residency, uploads them, and renders terrain, water,
and candidate-compatible proxy vegetation. It does not reconstruct continents
or choose family weights.

World Explorer native and browser hosts select the source, seed, view, and
review journey. They may expose URL/CLI labels and diagnostics but may not own
terrain recipes or platform-specific lowering. Current production remains a
separate selectable source. Candidate mode begins horizon-only: production
exact coverage must not be composited over candidate terrain or suppress it.

The source identity must qualify terrain, water, material, and vegetation
products together. Candidate vegetation may begin as bounded proxy structure
derived from candidate cover facts; it must never silently reuse production
forest decisions under a candidate terrain identity.

## Phases And Commit Gates

### Phase 1: Shared Candidate Surface

- Add the descriptor, five-family vocabulary, packed surface sample, point and
  window queries, work accounting, canonical native/Wasm harness, and direct
  performance receipt.
- Implement coast, rolling interior, fluvial/wetland, and upland/escarpment
  realization first.
- Prove flat water, safe signed coordinates, supported periodic lifts,
  partition equality, and zero exact-chunk work.
- Commit before adding a World Explorer selector.

### Phase 2: First Drawable World Explorer Source

- Add a typed candidate source identity and route its reference tiles through
  the ordinary clipmap refill/upload path.
- Keep production as the default and candidate as an explicit selection.
- Disable incompatible exact composition and production vegetation explicitly.
- Capture and inspect the first native image before extending complexity, then
  capture browser pixels and a short fly-through.
- Record broad query, refill, retained bytes, upload, draw, and completed-frame
  time separately.

### Phase 3: Candidate Cover And Water Composition

- Render source-qualified ocean, flat inland water, substrate families, and
  forest/open/wetland structure.
- Add deterministic candidate proxy vegetation only through the shared
  terrain-view vegetation contract.
- Inspect coast, clearing, edge, and wetland pixels independently before the
  complete composition.

### Phase 4: Arid Rain-Shadow Contrast

- Extend the plan with explicit leeward exposure, aridity, drainage
  permanence, and compatible substrate/cover facts.
- Implement the fifth family without a palette-only threshold.
- Re-run the exact plan/surface witness and atlas control, then inspect the
  matched temperate/arid contrast in map and World Explorer.

### Phase 5: Human Review B Package

- Select and publish all six journey receipts.
- Capture matched native and deployed-browser overview, oblique, horizon, and
  representative fly frames.
- Record recurrence/repeated-scene alarms and whether plan identities remain
  legible across clipmap spacings.
- Update the topic and this execution record, commit, push, deploy the exact
  revision, and stop for the review decision.

## Performance Evidence

Report descriptive measurements before setting new budgets:

1. direct point cost and complete 65 km/131 km fixed-resolution windows;
2. per-family and complete sampler work counts;
3. native versus Wasm semantic witness;
4. clipmap cold fill, warm retained movement, upload bytes, retained bytes,
   and completed-frame time;
5. browser Worker and presentation time;
6. journey selection cost and scan bounds; and
7. exact chunks, fine children, and feature batches constructed, which must
   remain zero.

The current LOD remains a downstream fixed-budget consumer. Do not reduce
terrain quality based on clipmap level or make coarse visibility canonical
geography.

## Execution Record

### Phase 1: Shared Candidate Surface — complete

- `mclone-worldgen` owns
  `continental-ecoregion-candidate-v1`, five composable terrain-character
  families, explicit solid/display/water levels, substrate, cover facts,
  stable plan identities, and decomposed height contributions.
- The pinned native/Wasm corpus compares whole and partitioned windows,
  randomized point traversal, native threads, and 196,608-block cylinder
  lifts. Its broad cases exercise the four temperate families, water, and at
  least three substrates while reporting zero exact chunks, density volumes,
  and feature batches.
- First-pixel correction revised the shared plan and surface rather than the
  renderer: province-owned height terms converge to a common boundary
  surface, exposed stone needs local ridge evidence, low-frequency owner warp
  breaks straight ownership edges, and typed ecotone fans carry a 4,096-block
  base plus adjacency-specific width.

### Phase 2: First Drawable Source — complete

- `TerrainPreviewProfile::ContinentalEcoregionCandidate` routes the shared
  surface through a CPU/reference clipmap refill and a two-sample stitched
  height halo. The production source retains its GPU sampler unchanged.
- Native `--source continental` and browser `source=continental` are explicit
  horizon-only selections. Both reject exact composition and omit the
  production vegetation executor; production remains the default.
- A 1,280-by-720 native Metal capture at an 8,192-block oblique view filled
  all 160 fixed slots, retained 133,344,028 bytes, and reported zero exact
  chunks and zero vegetation. A warm complete fill was about 0.9 seconds;
  first runs after shader compilation also exposed a roughly 13-second
  pipeline cold-start and are not treated as sampler time.
- The headed Chrome/WebGPU candidate smoke reached all 160 slots, retained the
  same byte count, moved and rebased through the shared controls, and kept
  exact painted chunks and tree instances at zero. Pixel evidence lives under
  `/tmp/mclone-world-explorer-web-continental-*.png`.
- Native and Wasm both reproduce the pinned plan and surface witnesses, and
  the World Explorer all-target suite keeps the source and ownership locks
  green.

### Phase 3: Candidate Cover And Water Composition — complete

- The candidate's own cover facts now deterministically populate one fixed
  global proxy-tree lattice. Tile partitioning produces the same ordered
  records, and the carrier is tagged with a candidate vegetation revision.
- The ordinary native thread and browser Worker coordinator compile and move
  the same typed product. Candidate jobs never construct or query the
  production forest-plan cache; production and candidate vegetation source
  fingerprints are distinct.
- Human Review B uses one stable 24-block global proxy lattice across every
  candidate review record spacing. The same source-qualified carrier reaches
  spacing 16 and all 80 vegetation tiles, while production candidate cache
  requests remain zero.
- Individual proxy geometry draws only at views of 768 blocks or closer;
  broader views use continuous forest/open/wetland surface cover. Two rejected
  review attempts informed that boundary: a short-reach spacing-4 carrier
  exposed a roughly kilometre-square vegetation island, while rank-thinning
  independently at each spacing exposed camera-centred density rings. The
  shared global lattice removes both presentation artifacts without making
  camera distance a world-generation input.
- Across the six final habitat frames the fixed carrier retained 11,033 to
  33,927 records. Native and browser compile the same records through the
  ordinary coordinator, all exact counters remain zero, and no production
  forest-plan cache is queried.

### Phase 4: Arid Rain-Shadow Contrast — complete

- Plan revision 8 publishes prevailing wind, query-position rain-shadow
  potential, province leeward exposure, continuous moisture and temperature,
  aridity, and drainage permanence. Nearby continental owners blend their
  shadow potential, so a district boundary cannot become a desert wall.
- The fifth surface family lowers those facts into dry basin/shelf relief,
  suppresses permanent lake and channel water where drainage cannot support
  it, retains dry washes, opens vegetation, and uses sparse exposed sand,
  coarse soil, and rock amid continuously tinted dry ground.
- The first pixel attempt exposed province-sized rectangular substrate slabs.
  Review rejected that image. Moving climate and rain-shadow response from
  owner-center constants to continuous point fields removed the semantic
  cliff; typed tests pin the corrected ownership crossing and a causally dry
  review point.
- Terrain Lab atlas schema 7 exposes leeward exposure, aridity, and drainage
  permanence as a dedicated layer. At fixed 256-square atlases, the 65 km
  view is 3.1% arid / 1.8% strong rain shadow / 50.3% lasting drainage; the
  131 km view is 8.0% / 5.1% / 17.5%. Candidate and production controls both
  construct zero exact chunks.
- The final plan witness is
  `a27b4f1b7547c86ffd7de3fe750d8abb421099125d5986683d9d6b61f08d8ee4`;
  the final surface witness is
  `69d083dbde36f497aefb766c975803a937f913d7469806e7afedfb7af6f9d3e9`.
  The fixed 65 km and 131 km direct windows measured 1,479 and 1,214 ns per
  sample on the review host. The headed browser atlas test and native
  World Explorer map/oblique captures passed and were visually inspected.

### Phase 5: Human Review B Package — complete

- `mclone-worldgen` selects six sites with one direct 131,072-block scan at a
  512-block step. The scan visits 66,049 samples, constructs no exact chunks,
  density volumes, or feature batches, and measured 170.7 ms total / 2,584 ns
  per sample on the review host. Selected centres remain at least 16,384
  blocks apart.
- The catalog schema is `mclone-continental-surface-journeys-v1`; its canonical
  hash is
  `f9ec52923b051081fc31c6cb610e62e01180625023b2f5189281c1c012001c6f`.
  Native, Wasm, CLI, and browser selectors use the same catalog:

  | Journey | Centre | Heading | Span | Sequence summary |
  |---|---:|---:|---:|---|
  | coast-to-wooded-interior | -13,824 / 9,216 | 0 / -1 | 16,384 | ocean coast to rolling interior and upland |
  | clearing-between-forest-cores | -30,720 / 10,752 | 1 / 0 | 12,288 | upland edge into a broad rolling clearing |
  | long-forest-edge | 26,112 / -57,344 | 1 / 1 | 23,168 | long rolling/upland forest-edge traverse |
  | connected-water-country | 15,360 / -13,312 | 1 / 1 | 23,168 | lake to land, lake, and river country |
  | quiet-rolling-interior | -50,688 / -56,832 | 0 / 1 | 16,384 | continuous quiet rolling interior |
  | upland-to-arid-basin | 4,096 / 28,160 | -1 / 1 | 20,272 | upland through rolling land to rain shadow |

- Each review selector publishes matched 65,536-block locator, 16,384-block
  overview, 8,192-block oblique, and 512-block habitat frames. The browser
  adds a deterministic 1,024-block fly step. Native captured 24 frames and
  the headed Chrome/WebGPU lane captured 30; all were visually inspected.
- All six sites filled 160 terrain slots and 80 candidate vegetation tiles
  with no pending work. Native cold completion was 2.09-2.49 seconds, coarse
  readiness 47-53 ms, vegetation compilation 52-54 ms, and resident memory
  134.4-136.6 MB including 133,344,028 fixed terrain bytes. Exact chunks and
  production vegetation-cache requests remained zero.
- The six final habitat and fly frames are materially distinct. The clearing
  reads as meadow enclosed by forest, the quiet interior preserves open
  negative space, the arid basin is sparse, and connected water is clearest
  in its broader sequence. No repeated camera-shaped vegetation boundary
  remains. This is implementation evidence, not the Human Review B decision.
- The ordinary all-target suites pass with 454 worldgen tests (one ignored),
  144 terrain-view tests (one ignored), and 14 World Explorer application and
  ownership-lock tests. Formatting passes. Production
  `mclone-overworld-v1`, exact chunks, and the new-world default are unchanged.
- The exact pushed implementation revision is `83b9a310e304`; the review UI is
  published under `https://mclone.kzahel.com/explore/`. Final deployment
  verification is recorded at handoff after the documentation commit is
  pushed and the same code bundle is republished from that exact revision.

## Non-Goals

This tactical does not:

- change `mclone-overworld-v1` production output or new-world defaults;
- generate or decorate candidate exact chunks;
- implement caves, arches, overhangs, structures, erosion simulation, or a
  global watershed;
- implement animal migration or population simulation;
- choose a default large cylinder period;
- make every planned biome or climate family; or
- retain a complete infinite-world plan or require a semantic cache.

## Human Review B

The decision is one of:

1. **accept** the broad surface and authorize a bounded exact-site probe;
2. **revise** named family, water, cover, scale, transition, or journey rules;
   or
3. **reject** the surface realization while retaining the accepted plan and
   its exact-query evidence.

Acceptance requires recognizable map-to-terrain continuity, materially
different journeys, plausible flat/graded water, meaningful quiet space, no
obvious plan-shaped stripes, direct broad generation without exact chunks,
and interactive native/browser review. Automated metrics and attractive
captures do not decide this gate.

## Related

- [`322-continental-ecoregion-explorer.md`](322-continental-ecoregion-explorer.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/lod.md`](../topics/lod.md)
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/habitat-driven-creature-ecology.md`](../topics/habitat-driven-creature-ecology.md)
