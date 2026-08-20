# Tactical 324: Continental Surface World Explorer

Status: **in progress 2026-08-20. Phases 1-3 are complete: the shared broad
surface and explicit native/browser World Explorer source are drawable with
source-qualified cover and zero exact or production-vegetation work. Continue
through arid contrast and six journeys, then stop at Human Review B.**

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
- At the seed-12,345 cover checkpoint, native retained 473 instances in 48
  ready vegetation tiles for 45,408 bytes. The worker spent 4.5 ms compiling
  the complete settled coverage while all exact counters remained zero.
  Headed WebGPU retained the same source contract through movement and
  reported 1,229 records at the second site.
- Individual proxy geometry draws only at views of 2,048 blocks or closer.
  Broader views retain the records and use continuous forest/open/wetland
  surface cover, avoiding a rectangular near-record island in continental
  overview pixels.

Phase 4 is next. The fifth family is still deliberately absent: current
`aridity` is only inverse moisture and `leeward_exposure` remains zero, so no
desert-looking pixels are accepted until the plan owns the rain shadow.

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
