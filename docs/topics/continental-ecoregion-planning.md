# Continental And Ecoregional Landscape Planning

Topic: `continental-ecoregion-planning`

Status: **Accepted product direction as of 2026-08-20. Tactical
[`322`](../tactical/322-continental-ecoregion-explorer.md) is authorized to
build the first Rust-owned plan atlas and gate a World Explorer realization.
Its disconnected shared plan, native/Wasm exact harness, fixed-cost Terrain
Lab atlas, decomposed field-revision-21 control, and sampled transition,
clearing, recurrence, and habitat-graph distributions now pass at both 65 km
and 131 km.
The first pixel review rejected a cellular composition; Revision 2 increases
the ecoregion and clearing hierarchy and is the active candidate. Its current
initial habitat graph linked only about one quarter of typed patches, which
selected a named revision rather than acting as an inferred quality score.
Human Review A first selected that revision on 2026-08-20: retain the candidate
hierarchy and clearings, author adjacency-specific ecotones, and replace the
single corridor band with a typed spine-and-branch habitat network.
The ecotone correction is implemented in plan revision 3: same-kind owner
boundaries disappear, unlike neighbors carry explicit 0.8-2.6 km widths, and
cover/climate blend continuously to their boundary. Plan revision 4 completes
the route-network correction with stable typed continental spines and bounded
cross-links. The same graph rule now links 44.7% of sampled habitat patches,
up from roughly one quarter; that remains descriptive review evidence rather
than a migration proof or acceptance threshold. Human Review A accepted the
corrected macro grammar on 2026-08-20. It also required responsive retained-map
panning and less regular, anchor-responsive route realization. These are
follow-up representation and geometry corrections, not another rejection of
the continental hierarchy. Arid contrast and World Explorer realization are
now permitted while those corrections remain explicit.
Terrain Lab now implements the interaction correction by moving the retained
accepted raster immediately and coalescing complete Worker rebuilds behind a
100 ms settle window. No cache affects plan facts or checksums; bounded atlas
tile reuse remains optional future performance work. Anchor-responsive route
geometry is the remaining accepted Review A correction.
Mclone should combine
top-down continental and ecoregional planning with bottom-up procedural
realization so the Overworld contains large, recognizable, causally related
places rather than more labels over the same local terrain fabric. The first
proof belongs in the terrain-exploration tools: plan and distribution review
in Terrain Lab, followed by navigable three-dimensional review in World
Explorer. This is a world-generation direction, not an LOD campaign. The
current multiscale semantic-terrain experiments were technically useful but
visually disappointing and are not the selected terrain design. Production
`mclone-overworld-v1` is internal-mutable, and a reviewed implementation may
change its terrain, water, climate, surfaces, ecology, fixtures, and existing
internal saves substantially.**

## Motivation

Current Mclone Overworld terrain looks plausible locally and reads clearly
from the procedural horizon, but the broad view exposes limited authored
composition. Its largest live continental and climate bands are still only
kilometre-scale, climate changes biome/surface/decoration more readily than
landform, and most journeys reuse the same scalar hill vocabulary. A desert
threshold over that foundation would mostly create familiar hills covered in
sand. Suppressing trees through another local density threshold would create
holes in a forest, not memorable large clearings.

The product goal is many distinctive, realistic-feeling regions without
requiring hundreds of nominal biome IDs. Realism here means causal agreement:
landmass, relief, water, climate, geology, vegetation structure, disturbance,
and animal geography explain one another. Authorship means each region has a
dominant story, compatible subordinate variation, deliberate transitions, and
quiet space rather than an unconstrained product of independent noise fields.

## Scope And Ownership

This topic owns:

- the product-scale distinction between continents, physiographic provinces,
  ecoregion instances, landscape patches, and exact local realization;
- the authored regional grammar that produces many distinct places from a
  manageable vocabulary;
- the combination of top-down semantic constraints and bottom-up procedural
  texture;
- continental-scale land/ocean and climate organization suitable for large
  habitat networks and animal migration;
- explicit meso-scale clearings, forest cores and edges, wetlands, dune
  fields, disturbance patches, refuges, and ecological corridors;
- the separation between canonical plan facts, optional caches, coarse review
  representations, and exact generated blocks;
- the first terrain-exploration proof and the gate for production integration;
  and
- the topology and world-scale questions raised by optional small and large
  cylinders.

It does not replace:

- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md),
  which owns cross-system precedence, water/geology/site composition, plan
  claims, and the general mechanism-selection rules;
- [`deterministic-streamed-landscape-planning.md`](deterministic-streamed-landscape-planning.md),
  which owns exact request/path/cache/window/schedule/topology invariants and
  the reusable falsification harness;
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md), which
  owns live implementation truth and field revisions;
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md),
  which owns durable animals, active ecology, resource strata, and eventual
  unloaded population semantics;
- [`bounded-world-topology.md`](bounded-world-topology.md), which owns
  canonical finite and periodic world identity; or
- [`lod.md`](lod.md), which owns the current procedural-horizon terminology
  and routes implementation details to the renderer topics.

## Product Decision

Use a top-down and bottom-up composition:

```text
stored profile, seed, dimension, topology, and world-scale descriptor
  -> continental plan
       landmasses, oceans, climate belts, major range and basin opportunity
  -> physiographic provinces
       ranges, plateaus, basins, lowlands, rain shadows, major drainage
  -> ecoregion instances
       authored regional identity, transitions, vegetation structure,
       habitat and signature permissions
  -> landscape patch plans
       clearings, groves, wetlands, burns, dunes, corridors, refuges
  -> bottom-up realization
       continuous fields, bounded plans, local density, surfaces, blocks,
       vegetation, features, structures, and active ecology
```

Top-down does not mean eagerly generating or storing a complete infinite map.
It means that coarser facts have stable identities and direct queries before
finer realization. A plane may lazily reconstruct a finite ancestor chain and
bounded owner neighborhood for one requested location. A finite future world
may choose to compile a complete coarse atlas. Both routes must publish the
same kinds of facts to downstream consumers.

Bottom-up procedural work remains essential. Noise, analytic profiles,
bounded features, and selective density should irregularize and realize an
accepted place. They must not independently choose another continent, erase a
major corridor, or turn every regional archetype into the same scalar hills.

## Planning Scales

These spans are initial product-scale hypotheses for review, not persisted
constants or a promise that every mechanism uses square cells:

| Level | Approximate span | Authority |
|---|---:|---|
| continental | 32-128 km and larger | landmass/ocean organization, broad climate and geological provinces, principal ranges and basins |
| physiographic province | 4-32 km | range branches, plateaus, lowlands, rain shadows, major drainage and circulation |
| ecoregion instance | 1-12 km | dominant ecological identity, compatible landform/water/geology bundle, transition and negative-space policy |
| landscape mosaic | 128 m-3 km | clearings, forest cores/edges, wetlands, dune fields, disturbance, habitat corridors and refuges |
| walking/local | 8-256 m | banks, boulders, tree groups, riffles, groves, small sites and navigational detail |
| block/material | 1-16 m | exact substrate, ledges, individual plants, snow, soil and voxel discretization |

The present largest production fields do not establish these target scales.
The first exploration must vary them deliberately and judge complete journeys,
component sizes, adjacency, and recurrence rather than assuming that one
larger noise wavelength creates a continent.

## Regional Authorship Contract

Biome IDs remain useful runtime and presentation facts, but they are not the
macro author. An ecoregion instance should be able to record or reconstruct:

- a stable identity and one dominant authored archetype;
- its role inside a continent and physiographic province;
- climate normals, seasonality, exposure, and rain-shadow relation;
- geology, substrate, soil, and water-permanence families;
- terrain and hydrology permissions rather than final blocks;
- vegetation structure: open/closed cover, canopy pattern, treeline,
  riparian bands, and succession stage;
- explicit core, shoulder, ecotone, corridor, refuge, and exclusion facts;
- a small compatible set of signature formations, landmarks, and ambience;
- disturbance or landscape-history facts such as flood, fire, windthrow,
  grazing, abandonment, or regrowth; and
- negative-space and repetition budgets.

Start with roughly 12-18 strong archetypes and several compatible regional
variants rather than a large nearest-parameter table. Seeded regional
fingerprints may vary proportions, orientation, history, and signatures only
inside their archetype's authored envelope. Direct endpoint adjacencies need
explicit rules: a cool wet ancient forest should not blend directly into an
interior dune sea merely because two independent fields cross a threshold.

The intended result is not one of everything near every spawn. A region may
remain ordinary or quiet for kilometres so a lake district, desert basin,
ancient forest, flowering clearing complex, or fractured alpine front has
enough contrast to become memorable.

## Clearings And Landscape Mosaics

A large clearing is a positive planned place, not only low tree density. Its
plan should establish:

- stable bounds or another compact influence representation;
- a core, shoulder, and forest-edge relation;
- an ecological or historical cause;
- target scale and openness without forcing a perfect blob;
- succession, ground cover, deadwood, shrub, and young-tree response;
- relationships to water, nearby cover, other openings, and corridors; and
- shared facts used by vegetation, habitat fitness, animal behavior,
  structures, local features, review maps, and distant presentation.

The same landscape-mosaic mechanism should support forest cores, burns,
storm gaps, meadows, floodplains, marshes, dune corridors, salt flats, and
other meso-scale regions without flattening their distinct rules into one
generic patch type.

## Continental Ecology And Migration

The macro plan should make ecological connectivity queryable before animals
materialize. Candidate facts include seasonal range patches, watering and
feeding areas, breeding or nesting grounds, passes, river crossings,
stopovers, bottlenecks, barriers, and route identities.

Current 64-by-64-block wildlife cells remain local initial-realization and
resource-accounting units. They are too small and independent to author a
continental population distribution or migration. A later migration system
may add a distinct unloaded herd/population lifecycle, but it must preserve
the existing rule that a materialized visible animal is a durable individual.
Terrain planning can land the habitat network first without prematurely
inventing aggregate simulation or teleporting live animals between regions.

## Plan, Cache, And Representation

Keep three concepts separate:

1. **Canonical plan facts** are deterministic geography derived from the
   stored descriptor, seed, dimension, topology, revision, and stable owner.
2. **Caches** retain recently reconstructed facts for speed. Eviction or
   traversal changes time only, never geography.
3. **Representations** expose the amount of accepted truth needed by a map,
   broad 3D review, exact chunk generator, habitat query, or renderer.

A coarse query must not recursively construct all fine children. A column
sample must not launch an unbounded watershed, route search, or continent
solve. A broad preview must not generate or downsample every exact chunk.
Direct continental and province summaries may simplify geometry while
retaining dominant land/ocean, height envelope, water coverage, major axes,
regional identity, and habitat connectivity.

Cache keys include every output-affecting descriptor fact and the canonical
planning level/owner. Initial internal profiles should prefer deterministic
reconstruction over persisted plan caches. Persist plans only when measured
cold cost, release compatibility, player modification, or discovery semantics
establishes a concrete need.

## LOD And Terrain-Exploration Boundary

This direction is not selected to improve LOD, and the earlier semantic
multiscale terrain shapes are not its required foundation. The world must
first become compelling in plan maps, regional three-dimensional views, and
walking-scale journeys. The procedural-horizon LOD is then a downstream
consumer with a non-negotiable performance contract:

- fixed-budget broad views must query direct coarse facts rather than exact
  chunks or a complete fine plan;
- major coast, water, range, forest/open, and regional-identity facts should
  remain legible when sparse height samples would miss them;
- exact generation and broad presentation must describe the same geography;
  and
- camera scale and residency must never become generation inputs.

Use Terrain Lab for plan layers, distributions, inspection, and A/B controls.
Use World Explorer for fast oblique, horizon, fly-through, and eventually
walking-scale composition review through the shared terrain source. A tool
prototype is not a separate generator: Rust worldgen owns every geographic
decision that could later enter production.

## Topology And World Scale

The ordinary Mclone startup remains an unbounded plane. The existing
6,144-block/384-chunk X-cylinder is an opt-in exact-periodicity proof, not the
continental scale target. Small cylinders should remain possible for tests,
novelty worlds, or deliberately compact profiles.

A serious ecological cylinder needs an explicit larger persisted period and
a compatible world-scale descriptor. Periods that are integer multiples of
the current 6,144-block vocabulary are useful candidates because they retain
existing field divisibility, but no 98 km, 197 km, or other default is selected
before continental maps and journeys are reviewed. Increasing the period
without increasing continental, climate, province, and habitat scales would
only create a larger container for the current samey terrain.

Do not require one terrain scale preset to fit both a tiny novelty cylinder
and a continental world. Profile/topology validation should declare supported
scale combinations. A future periodic-X plus finite-Z cylinder with polar or
otherwise inaccessible caps remains a separate bounded-world product option.

## Production-Change Posture

The profile is internal and unreleased. Current field revision 21, the eight
biome recipes, existing fingerprints, and disposable internal worlds are not
a reason to preserve the samey result. After the exploration proof identifies
a compelling mechanism, a focused production tactical may deliberately change
continentalness, climate, landforms, water, biomes, surfaces, vegetation,
spawn, ecology inputs, and LOD summaries together. It must update fixtures,
docs, persistence expectations, and every affected platform boundary.

Do not require the new direction to promote Tactical 272/273's reconstructed
range or basin courses. Reuse their exact identity, bounded-query,
parent/child-consistency, native/Wasm, and cache-independence lessons where
helpful. The product mechanism may instead use direct coarse fields, canonical
regional plans, feature-owned graphs, a bounded hybrid, or another reviewed
composition.

## First Exploration

Tactical
[`322`](../tactical/322-continental-ecoregion-explorer.md) owns the first
bounded campaign. It should:

1. Preserve current field revision 21 as a visible control, not a protected
   output target.
2. Add a Rust-owned continental/ecoregional plan view over at least 65 km and
   131 km domains, with later roughly-500-km coverage only after the coarse
   query is direct and measured.
3. Show landmass and inland distance, physiographic province, dominant
   ecoregion, transition shoulders, vegetation openness, large clearings,
   major water, habitat patches, and corridor connectivity independently and
   in composition.
4. Prove one temperate forest/open-land province using existing deer, rabbit,
   bee, mallard, and squirrel habitat vocabulary. Include kilometre-scale
   open country or clearings, forest cores and edges, riparian/wetland
   connections, and meaningful negative space.
5. Compare same-seed current production, plan-only reconstruction, and the
   first realized candidate in maps, obliques, fly-through journeys, and a
   small number of exact walking-scale sites.
6. Record cold plan work, warm/batched queries, retained bytes, cache
   independence, native/Wasm equivalence, and broad-preview time separately.
7. Stop for human product review, then either integrate a substantial
   `mclone-overworld-v1` revision, narrow the mechanism, or reject it.
8. If the temperate proof succeeds, add an arid rain-shadow/desert province as
   the first strong contrast rather than multiplying nominal biome labels.

## Acceptance

The direction succeeds only if review can identify and remember places, not
merely observe more colored fields. Evidence should include:

- landmass, province, ecoregion, and clearing component-size distributions;
- direct endpoint adjacency and transition-width distributions;
- recurrence distance for regional signatures and quiet-space coverage;
- 10-50 km journey sequences with dwell length and repeated-scene alarms;
- habitat-patch and proposed migration-corridor connectivity;
- major coast/water/range agreement across plan, broad 3D, and exact views;
- no exploration-, cache-, window-, schedule-, or topology-lift dependence;
- direct coarse-query cost with zero fine-child or exact-chunk work;
- ordinary as well as showcase regions; and
- human descriptions worth preserving, such as “broad flowering clearing
  between old forest cores” or “rain-shadow basin crossed by an ephemeral
  drainage chain.”

## Related

- [`Tactical 322`](../tactical/322-continental-ecoregion-explorer.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md)
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md)
- [`deterministic-streamed-landscape-planning.md`](deterministic-streamed-landscape-planning.md)
- [`multiscale-terrain-representation.md`](multiscale-terrain-representation.md)
- [`bounded-world-topology.md`](bounded-world-topology.md)
- [`lod.md`](lod.md)
