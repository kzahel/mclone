# Mclone Macro Landscape Planning

Topic: `mclone-macro-landscape-planning`

Status: **Active design direction as of 2026-07-26. The original Mclone
Overworld has useful independent terrain, climate, bathymetry, river,
wetland, surface, and bounded-stream facts, but it does not yet have a
holistic macro landscape plan that coordinates their topology, precedence,
overlap, and downstream feature permissions. This topic owns that planning
view. Tactical 259 has now grounded coastal character in Alpha, Beta, Java
1.17.1, Java 26.2, and measured current Mclone evidence; coast implementation
remains the next selected planning slice.**

The motivating July 2026 visual review found four related regional-scale
weaknesses:

- sandy material forms a nearly universal coastal collar, while rocky terrain
  seldom continues directly into water;
- major rivers remain legible as smooth bands of a noise field even after
  useful local width, depth, bank, substrate, and outlet corrections;
- broad inland shallow ponds are underrepresented; and
- river connections rarely create small islands, confluence landscapes,
  anabranches, deltas, or other compound water-and-land forms.

Those observations are not one missing noise octave or surface recipe. They
show why terrain, hydrology, geology, ecology, and landmarks need a shared
eagle-eye plan before local blocks and decoration make the result expensive
to reinterpret.

A companion performance review adds another requirement: Mclone should gain
volumetric terrain selectively without losing the cheap, broad sampling that
makes Terrain Lab and World Explorer useful. Geographic extent, sample
resolution, semantic fidelity, and exact 3D realization are separate costs.
The planning contract must state which representation is authoritative at
each scale rather than making every distant sample perform exact chunk work.

## Scope

This topic owns:

- the spatial scales at which landscape decisions become authoritative;
- the dependency and precedence relationships between terrain, water,
  geology, ecology, landmarks, structures, and routes;
- the semantic planning facts that several downstream systems need to share;
- the distinction between provisional terrain, planned modifiers, and final
  realized terrain;
- macro and regional overlap, exclusion, transition, and reservation rules;
- the scale-dependent sampling and summary obligations shared by exact
  generation, research tools, and distant terrain;
- the performance contract for adding bounded volumetric terrain without
  imposing universal 3D density work;
- the review maps, distributions, and player-experience questions needed
  before a family becomes block output; and
- the recommended cross-system order for future Mclone Overworld campaigns.

It deliberately does not replace:

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md), which
  owns current implementation truth, field revisions, mechanism boundaries,
  performance receipts, and tactical history;
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md), which owns the
  content-family and regional-vocabulary ledger;
- [`world-generation-profiles.md`](world-generation-profiles.md), which owns
  generator identity, persistence, and compatibility safety;
- [`../structures.md`](../structures.md), which owns generic structure
  starts, references, pieces, placement, and persistence architecture;
- renderer and distant-terrain topics, which own presentation mechanisms; or
- the reference-locked Java 1.17.1 Overworld.

This is an original-profile planning document. Minecraft and studied
community generators supply evidence and useful vocabulary, not rule tables
or a seed-parity target.

## Why A Separate Planning Layer Is Needed

Individual procedural systems can all be locally competent while the world
still feels assembled:

- a mountain field can make attractive peaks;
- a river field can make attractive curves;
- a biome classifier can make attractive regions;
- a structure planner can make attractive buildings; and
- feature tables can make attractive local detail.

If those systems do not agree about the large-scale landscape, the river cuts
across an unrelated ridge, the beach coats every coast, the forest hides a
signature formation, the road ignores the pass, the settlement flattens a
wetland, and local decoration conceals rather than reinforces regional
identity.

The planning layer exists to answer questions that final blocks cannot answer
reliably:

- Is this coast depositional, rocky, marshy, frozen, or cliff-backed?
- Is this low line a true connected watercourse, a dry valley, or only a
  coincidental contour?
- Which direction is downstream, and what are the source and sink?
- Is this depression an open valley, a closed basin, a lake, or a wetland?
- Which geological formation family owns this silhouette?
- Where should a forest open into meadow, talus, floodplain, or settlement?
- Which crossing is a ford, bridge site, waterfall, gorge, or impassable face?
- Which areas should remain quiet negative space?

These facts should be explicit while they are cheap to inspect. They should
not be reconstructed later from block materials, biome IDs, or whatever
feature happened to win a final write.

## Equal Importance, Different Authority

Landform, hydrology, geology, ecology, and generated history are all
first-class parts of the world identity. Equal design importance does not
mean that every subsystem can modify every other subsystem at the same time.

The generation relationship must remain acyclic:

- provisional terrain gives water and sites something meaningful to evaluate;
- hydrology and geological intent reshape final terrain;
- final terrain, climate, water, and geology select ecology and surfaces;
- landmarks, settlements, and routes consume the resulting site facts and
  may apply bounded, declared grading or reservations; and
- local decoration fills permitted space without redefining the regional
  plan.

This order still permits strong interplay. It prevents circular ownership and
request-order-dependent results.

## Planning Scales

Scale names describe decision authority, not separate engines or mandatory
storage formats.

| Scale | Approximate span | Owns | Examples |
|---|---:|---|---|
| world / continental | 2-8 km and larger | dominant land/ocean organization and broad climatic/geological provinces | continents, ocean basins, major ranges, broad arid or humid belts |
| macro / regional | 512-2,048 blocks | connected landscape systems and major regional identity | mountain systems, drainage basins, major rivers, coast families, plateaus, large forests |
| meso / landform | 64-512 blocks | recognizable places and transitions within a region | tributaries, ponds, coves, passes, gorges, tors, groves, clearings, deltas |
| local / walking | 8-96 blocks | bounded features, surface response, and navigational detail | boulders, riffles, bank rocks, small ruins, tree groups, springs, fords |
| block / material | 1-16 blocks | final discretization and texture | substrate patches, ledges, individual plants, gravel bars, snow and soil depth |

The boundaries overlap deliberately. A river may be selected at macro scale,
receive bend and width character at meso scale, gain riffles and bank rocks at
walking scale, and finally resolve into blocks. A local octave may perturb its
edge; it may not silently create or erase the river's source, sink,
confluence, or island.

Every decision should be made at the coarsest scale that honestly owns it.
This keeps topology stable and prevents local noise from impersonating
landscape structure.

## Target Planning Flow

The target is a staged composition, not a universal graph runtime:

```text
stored profile, seed, dimension, and topology
  -> stable raw fields
       continentalness, climate, relief, ruggedness, ridges,
       substrate/geological province, independent selector domains
  -> provisional landscape
       unmodified surface, slope, curvature, valley potential,
       ocean intent, traversability
  -> macro planners
       landform region, coast, drainage/water bodies,
       formation permissions, corridors and candidate sites
  -> composition and arbitration
       accepted plans, strengths, transitions, exclusions,
       reservations, bounded overlap rules
  -> final terrain and water
       height/density, bathymetry, reaches, beds, banks,
       basins, volumetric formations
  -> regional presentation
       biome/ecology, surface/subsurface palette, vegetation intent,
       ambience and spawn intent
  -> landmarks, settlements, routes, and bounded site grading
  -> local features and decoration
  -> shared lighting, publication, persistence, and runtime simulation
```

This does not require every stage to allocate a large world object. Pure
point samples, bounded canonical plans, cached tiles, and structure records
may coexist behind typed queries. The contract is semantic: downstream work
must be able to consume the accepted upstream decision rather than infer an
approximation from final blocks.

## Raw, Classified, Planned, And Realized Facts

Keep four kinds of information distinct:

1. **Raw facts** are continuous, independently seeded inputs such as
   continentalness, temperature, moisture, ruggedness, and geological
   selectors.
2. **Classified intent** gives a coherent name and permissions to a
   combination of raw facts, such as rocky exposed coast, sheltered wet
   lowland, or fractured alpine range.
3. **Plans** have topology or bounded identity: a river reach, drainage basin,
   lake, anabranch, route, formation start, settlement site, or reserved
   corridor.
4. **Realized facts** describe final geometry and content: surface height,
   density, water plane, bed, material, blocks, and placed pieces.

Do not discard the earlier form when a real consumer needs it. A biome should
not need to guess whether a final low surface was naturally low or carved by
a river. A bridge planner should not detect a watercourse by scanning water
blocks. A distant terrain evaluator should not approximate an arch or lake
from final chunk residency.

Conversely, do not add speculative fields to a shared sample merely because a
future system might use them. Promote a planning fact when at least one real
planner and one downstream consumer establish its meaning.

## Core Landscape Concerns

### Continental and ocean organization

Continentalness should establish more than a binary land mask. It should
support:

- coherent continent and island scales;
- bays, peninsulas, straits, archipelagos, and inland distance;
- shallow margins, shelves, shelf breaks, and deep basins;
- room for mountain systems and drainage basins to terminate naturally; and
- exact finite or periodic topology when selected by the dimension.

Ocean geometry and land geometry must meet as one composition. Bathymetry is
not a texture underneath an independently chosen coastline.

### Relief, ranges, valleys, and traversability

Total elevation range is only one terrain characteristic. The macro plan also
needs:

- range orientation and connectedness;
- valley continuity and likely passes;
- broad versus narrow lowlands;
- local relief at several walking and viewing scales;
- plateaus, escarpments, high basins, isolated massifs, and quiet plains;
- slopes that create meaningful approach, ascent, and descent; and
- enough traversable corridors that dramatic terrain does not make every
  journey an arbitrary cliff climb.

Terrain should provide both destinations and circulation. The best mountain
is not only a silhouette; it has foothills, approaches, saddles, sheltered
spaces, exposed faces, and reasons to move around or through it.

### Hydrology

Hydrology includes several related but non-interchangeable systems:

- coast classification and coastal distance;
- ocean floor and receiving-outlet geometry;
- drainage basins and major river topology;
- named reaches, sources, sinks, confluences, and bounded drops;
- tributaries, streams, floodplains, wetlands, and springs;
- closed basins, shallow ponds, and larger lakes;
- deltas, distributaries, braided reaches, and anabranches;
- stable water planes, spill levels, beds, banks, and containment; and
- frozen, arid, tropical, marsh, and other climate responses.

Water must participate in terrain planning before surface materials. It also
needs stronger topology than a surface material or binary river mask when a
feature depends on source, destination, flow, crossing, or enclosure.

### Geology and volumetric formation

Geology gives terrain material and structural history:

- substrate and strata;
- exposed rock, scree, talus, and boulder families;
- cliff, tor, fin, hoodoo, stack, arch, shelf, and overhang permissions;
- erosion-resistant versus depositional coast behavior;
- cave, ravine, chamber, and underground-water relationships; and
- material and formation reasons for visiting high, low, coastal, and
  underground places.

A regional geological family may affect provisional relief or final 3D
density. Signature formations should remain mutually legible rather than
appearing wherever independent thresholds overlap.

### Ecology, surfaces, and regional identity

Ecology should respond to accepted terrain, climate, water, and geology while
remaining a coordinated landscape author:

- endpoint regions need explicit transition shoulders;
- vegetation density and grouping should follow exposure, wetness, substrate,
  altitude, disturbance, and landform;
- surfaces, trees, understory, clutter, ambience, and spawning should read as
  one regional bundle;
- clearings, forest edges, riparian bands, alpine treelines, and marsh
  transitions need meso-scale structure; and
- local diversity should reinforce rather than obscure the dominant region.

Biome IDs remain useful runtime facts, but they are not the complete planning
language and should not be the sole owner of terrain geometry.

### Landmarks, structures, paths, and generated history

Placed content should participate in the landscape rather than arrive after
it:

- natural landmarks should expose stable site identity and bounds;
- settlements should read slope, water, shelter, resources, routes, and
  hazard facts;
- bridges and mills should consume explicit crossings and water reaches;
- roads should prefer valleys, passes, ridges, fords, gates, and destinations;
- ruins may reserve circulation and imply former land use;
- bounded grading must be declared and measured; and
- overlapping sites need deterministic arbitration rather than last-writer
  wins.

Generated history can add later layers—abandonment, collapse, regrowth, path
wear, or reuse—only after the natural and built plans are stable.

### Negative space

Not every eligible region should receive a landmark or maximum decoration.
Quiet areas are part of the composition:

- open water between islands;
- uninterrupted plain between forests and mountains;
- a long empty ridge before a tor or ruin;
- calm reaches between cascades;
- sparse coast between coves and settlements; and
- ordinary woodland that makes an ancient grove distinctive.

Plans should carry density budgets or exclusion zones where needed. Adding
more feature attempts is not a substitute for intentional pacing.

## Cross-System Precedence

The desired relationships are:

| Producer | May influence | Must not do |
|---|---|---|
| continent/ocean intent | land mask, inland distance, coast and basin opportunity | select all local surfaces and features directly |
| provisional terrain | drainage cost, slope, exposure, passes, candidate sites | become immutable before planned water and geology respond |
| hydrology plan | valleys, beds, banks, basins, wetlands, coast outlets, crossings | infer unbounded topology during a column sample |
| geological intent | silhouette, density permissions, substrate, coast character | place every formation independently in one region |
| final terrain/water | ecology, surfaces, sites, local feature permissions | erase the raw or planned facts that explain it |
| ecology plan | vegetation, surface bundle, ambience, spawning, local clearings | redefine major mountains, rivers, or lakes |
| route/site plans | bounded grading, crossings, reservations, structure pieces | globally flatten terrain or replace natural hydrology |
| local decoration | fill accepted substrate and ecological opportunities | create macro identity or overwrite reserved plans |

When two upstream systems interact, prefer a bounded dialogue over a write
priority. For example:

- terrain supplies a valley cost to the river planner;
- the accepted river plan then reshapes final terrain;
- geology supplies resistant or erodible permissions to a coast;
- the accepted coast plan chooses rocky face, gravel margin, or sand
  deposition;
- a settlement reads the river crossing and may place a bridge, but cannot
  delete the reach; and
- vegetation reads a ruin reservation and may overgrow permitted pieces
  without obscuring the entire circulation plan.

## Water Review: Current Diagnosis

The July 2026 review is a useful example of why the planning distinction
matters.

### Coastlines

Current Mclone beach selection is primarily an elevation band around sea
level. Mountain strength also fades as continentalness approaches the coast.
The combination produces two separate symptoms:

1. the surface material becomes sand almost everywhere along the shore; and
2. the terrain itself tends toward a low coastal shelf, so merely replacing
   sand would often produce grass rather than a rocky face.

A future coast plan should classify a coherent alongshore family using some
combination of:

- land slope and curvature approaching water;
- coastal signed distance and shoreline orientation;
- shelf width and seabed grade;
- ruggedness and geological substrate;
- exposure versus shelter;
- climate and wetness; and
- regional transition continuity.

Sand should mean deposition on a suitable coast, not merely “top block near
Y63.” Rocky, gravel, snowy, marsh, and sandy coasts need both material and
geometry responses.

#### Cross-era coast evidence and selected direction

Tactical
[`259`](../tactical/259-modern-and-historical-coast-reference-survey.md)
turns that diagnosis into an implementation contract.

- Alpha v1.1.2_01 and Beta 1.7.3 obtain varied water edges from continuous
  sea-level geometry plus broad sand/gravel masks, without an explicit shore
  classifier.
- Java 1.17.1 inserts categorical shore biomes from four-neighbor biome
  adjacency. Its exact seed-74739 Stone Shore fixture proves tall, steep
  direct-water faces rather than a mere gray surface swap.
- Java 26.2 integrates a coast continentalness band into terrain/climate
  classification. Stony Shore, Beach, Snowy Beach, shattered coast, ordinary
  middle terrain, and river outcomes can all occupy the band.
- Current Mclone's low elevation Beach rule and coastward mountain fade make
  roughly 90% of sampled coast-adjacent land Beach on all three measured
  6,144-block regions. Every remaining sample was a river/wetland recipe;
  Exposed Stone, Eroded Slope, and Grass/Soil were all zero.

The first Mclone vocabulary is therefore sandy depositional, gravel
transitional, rocky/exposed, a cold modifier, and ordinary terrain allowed
directly at water. A broad topology-aware alongshore selector should create
coherent runs, but terrain, shelf, substrate, climate, and outlet suitability
constrain its choices. The result should be one small shared semantic coast
fact consumed separately by geometry, surface, review, and distant terrain.
It does not require universal 3D noise or new persisted biome IDs.

### River topology and terrain relation

Current major rivers use the near-zero contour of a warped gradient-noise
field. Relief and ruggedness perturb its coordinates, and the accepted
corridor then carves beds and banks into terrain. That is real coupling, but
it is mostly one-way and local: the route does not follow a persistent
source-to-sink drainage plan.

A regular contour is degree two almost everywhere. It naturally forms smooth
continuing lines and closed loops, but not a hierarchy of tributaries,
confluences, distributaries, and split/rejoin reaches. More edge noise can
make the same topology rougher without solving the regional signature.

Three macro skeletons deserve map-level comparison before another major river
implementation:

1. **Hierarchical boundary network**
   - derive irregular connected boundaries from repeatedly refined seeded
     region labels, inspired by the useful topology of Java 1.17.1 biome
     layers without copying their rules;
   - naturally creates junctions, loops, and fractal-like regional edges;
   - still needs terrain and hydraulic coupling.
2. **Terrain-derived drainage**
   - evaluate provisional height on canonical coarse tiles, resolve
     depressions, accumulate downhill flow, and promote sufficiently supplied
     paths;
   - gives the strongest basin, tributary, and terrain relationship;
   - has the highest cross-tile, periodic, lake-spill, and performance design
     cost.
3. **Hybrid planned network**
   - select bounded sources, sinks, and ocean outlets, then route a hierarchy
     through a coarse cost field that prefers valleys and downhill progress;
   - retain explicit reach levels, drops, confluences, identities, and
     structure-compatible facts;
   - fits the existing bounded stream planner and fixed-point water direction
     most directly.

The hybrid is the leading architectural fit, but it is not selected until
same-seed topology, terrain-alignment, cost, periodicity, and pixel evidence
compare the alternatives.

### Inland shallow water

Current wetland pools are subordinate to low-gradient major-river margins and
the global sea-level water family. They do not constitute a general inland
basin model.

Keep at least three distinct water-body families:

- incidental sea-level lowland depressions, useful for broad shallow water;
- planned closed basins with an explicit spill level, irregular shallow bowl,
  shore treatment, and optional outlet; and
- small bounded feature ponds and springs, useful as local accents.

Large ponds and lakes should consume basin facts. Expanding a local lake
feature or river-bank mask cannot establish a spill point or explain why the
water belongs in the surrounding landform.

### Small islands and compound water forms

A merging drainage tree does not by itself create many river islands. Land
becomes enclosed when water deliberately splits and rejoins, wraps a basin
high, or combines with another water boundary.

Useful explicit plan roles include:

- low-gradient anabranches around a preserved island core;
- braided reaches with bounded secondary channels;
- delta distributaries near a receiving coast;
- lake islands and wetland hummocks;
- oxbows and cutoff channels; and
- confluence wedges connected to a nearby secondary channel.

These should be selected meso-scale landform families, not accidents required
from the global network. The existing bounded planned-structure pattern is a
good candidate for proving one stable anabranch or delta family after the
macro skeleton direction is selected.

## Choosing A Mechanism For A Feature

“Feature” is an overloaded term. Select the mechanism from the feature's
spatial and semantic obligations.

| Need | Appropriate mechanism |
|---|---|
| continuous response available everywhere | pure raw or derived field |
| one classified regional grammar | regional intent/router |
| connected source, sink, route, basin, or corridor | canonical bounded macro plan |
| rare bounded identity spanning chunks | structure start/reference/pieces |
| repeated regional solid or void response | gated 3D density modifier |
| local replace-if-valid decoration | placed/configured feature |
| final material response | surface/subsurface recipe |

Promote work out of local decoration when any of these are true:

- topology matters;
- another subsystem needs to query it before blocks exist;
- it spans an unbounded or variable number of chunks;
- it needs stable identity, bounds, reservations, or persistence;
- it changes terrain silhouette or circulation;
- it owns a source, destination, spill level, or crossing; or
- overlapping instances require arbitration.

Do not promote every boulder, tree, or puddle into a macro plan. Bounded local
features remain valuable when their neighborhood validation honestly captures
the whole behavior.

## Composition, Claims, And Overlap

Accepted macro and meso plans should be able to expose:

- stable descriptor-scoped identity;
- bounds and optional influence envelope;
- typed role and family;
- strength and transition weight;
- final geometry or a bounded recipe for reconstructing it;
- claimed, excluded, and conditionally shared spaces;
- material/ecology permissions;
- route, crossing, or landmark facts needed downstream; and
- summary facts required by review tooling and distant terrain.

Overlap must be resolved semantically:

- one signature regional formation family should usually dominate;
- a river may cut a geological region but should inherit substrate and
  resistance response;
- a lake may flood a low basin but not a protected major structure;
- a bridge may occupy a declared crossing while preserving water clearance;
- a settlement may perform bounded grading inside its site plan;
- vegetation may soften edges and overgrow permitted ruins; and
- local decoration must avoid claimed paths, water planes, structure
  interiors, and reserved sightlines.

Avoid a generic global priority number when typed relationships are clearer.
“River cuts rock,” “bridge spans river,” and “tree avoids road” are more
durable contracts than three unrelated writer priorities.

## Determinism And Runtime Constraints

Macro planning must preserve the established shared-worldgen contract:

- results are pure functions of the stored profile descriptor, seed,
  dimension, topology, and absolute coordinates;
- native threads and browser Workers reconstruct identical plans;
- request order, chunk partition, cache residency, and generation timing do
  not affect output;
- the ordinary plane and exact 6,144-block X-periodic cylinder remain
  supported by every accepted Mclone macro fact;
- bounded worlds and later topologies declare their canonical planning
  coordinates explicitly;
- planners use canonical fixed-size tiles, finite halos, stable starts, or
  another demonstrably bounded scheme;
- no column sample performs an arbitrary upstream walk, flood fill, ocean
  search, or global structure query;
- caches are bounded and keyed by every output-affecting descriptor fact;
- stored plan records become necessary only when reconstruction, release
  compatibility, player mutation, or discovery semantics require them;
- exact chunk generation and every distant representation consume one
  accepted semantic plan; and
- app/platform crates remain unaware of terrain families.

Performance limits affect mechanism selection, not the importance of the
landscape system. A cheap macro selector plus sparse bounded 3D work is often
better than evaluating a universal dense field in every block.

## Volumetric Terrain And Scale-Aware Sampling

### Current representation truth

The original Mclone terrain is currently a two-dimensional surface-height
field. A terrain sample produces one `surface_y` for `(x,z)`, and exact chunk
realization fills solid material beneath that height before applying bounded
water, surface, feature, and structure work. Those later systems can place
three-dimensional blocks, but the underlying regional landform remains a
heightfield. It cannot directly express a true overhang, arch, suspended
shelf, natural window, sea cave, or undercut outcrop.

Java 1.17.1 supplies a useful performance and quality reference. Its ordinary
Overworld evaluates a coarse three-dimensional density lattice and
interpolates it through the chunk rather than evaluating full octave stacks at
every block. The direct Terrain Lab sampler still needs 33 vertical density
nodes for one exact column; its accepted fast-macro approximation uses nine.
That cost buys multiple solid/air transitions and volumetric silhouette, but
it does not by itself create coherent drainage, better coast classification,
or a regional composition. Three-dimensional density is a local and regional
geometry mechanism, not a substitute for the macro plan.

Mclone's present preview speed has four separate causes:

- the natural macro source is an absolute-coordinate, primarily 2D sampler;
- one fixed output lattice can span more ground by increasing sample spacing;
- Terrain Lab and World Explorer do not generate every covered chunk; and
- the GPU evaluator processes many independent samples in parallel.

Do not attribute all current speed to GPU execution or all future cost to
geographic extent. Sample count, work per output, content stage, cache state,
packing, transfer, and presentation all matter.

### Recorded sampling baseline

The following receipts establish useful orders of magnitude. They are not a
normalized benchmark suite: they cover different revisions, stages, hosts,
and timing boundaries.

| Product and workload | Measured boundary | Recorded result |
|---|---|---|
| [Mclone Base, one 65x65 tile](../tactical/228-production-large-fields-in-terrain-lab.md) | warm CPU production reference | 4,225 outputs in about 2.0-4.7 ms; the same lattice at spacing 1,024 spans 65,536 blocks |
| [Mclone Cover, 65,536-block view at effective spacing 512](../tactical/244-lod-native-vegetation-presentation.md) | cache-off CPU reference | 26 tiles, 109,850 outputs, 549,250 terrain evaluations, 439,400 forest evaluations, 953.4 ms generation, and 23.1 ms packing/upload; a warm return reached target in 40.0 ms |
| [World Explorer fixed horizon](../tactical/249-cross-platform-procedural-horizon-proof.md) | native offscreen and movement receipt | ten levels and 160 slots retained 86,551,040 fixed terrain bytes, reached first coarse/target in 22.95/218.82 ms, and moved at 4.20 ms average with 11.48 ms P95 |
| [Vanilla fast macro versus sampled exact](../tactical/246-vanilla-fast-macro-terrain-preview.md) | native 65x65, 2,048-block grids | fast macro took 47.5-74.8 ms; sampled exact took 163.8-289.7 ms |

Host-side encode/submit and dispatch-to-validation clocks are not pure GPU
execution times. Future GPU comparisons should use timestamp queries when the
target supports them and otherwise retain the existing truthful boundary
labels. In particular, the same Cover receipt measured 3,250.6 ms from GPU
dispatch through validation readiness. That is slower than its CPU reference
boundary but includes validation and readback, so it proves neither a slow GPU
kernel nor an automatic GPU advantage.

The default World Explorer clipmap has four 64-cell tiles per axis at each of
ten levels. Its coarsest spacing is 512 blocks, so that outer level spans
131,072 blocks while all 160 resident tiles contain exactly 676,000 lattice
entries. Its fixed cost grows primarily with level count and samples per
level, not with the number of chunks geometrically covered.

### Defining a broad-preview workload

“A 500 km preview” is incomplete without a sample spacing or output
resolution. Assuming a 500,000-block-wide square, a single non-tiled lattice
contains approximately:

| Sample spacing | Points per axis | Total output points |
|---:|---:|---:|
| 2,048 blocks | 245 | 60,025 |
| 1,024 blocks | 489 | 239,121 |
| 512 blocks | 977 | 954,529 |
| 256 blocks | 1,954 | 3,818,116 |
| 1 block | 500,001 | 250,001,000,001 |

Tile borders, progressive parents, footprint taps, and comparison lanes can
increase the actual resident and evaluated count. Physical width can
therefore grow almost for free only when the output lattice stays fixed and
the coarser representation remains visually and semantically honest.

There is no recorded 500 km benchmark yet. Terrain Lab currently accepts at
most 131,072 blocks across. The default clipmap also reaches about 131 km.
With its current four-by-four outer geometry, a 524,288-block horizon would
need a spacing-2,048 level: two coarser levels beyond the current spacing-512
default. That would raise the fixed slot count from 160 to 192, only 20
percent, but the preview sampler currently caps spacing at 1,024 and the
coarser filtering contract is not yet sufficient. This is a promising design
projection, not measured performance or permission to extend the horizon
before coarse quality is proved.

Those widths describe the unbounded plane. They do not transfer directly to
the supported 6,144-block X-periodic Mclone cylinder: 131,072 blocks span more
than 21 canonical laps and 524,288 span more than 85. A periodic map should
normally show its fundamental domain once, while an in-world horizon must cap
at one lift, deliberately instance several lifts of one canonical residency,
or use an accepted topology-specific presentation. Canonicalizing planar
clipmap samples is not by itself a periodic distant-terrain design.

Every future broad-preview receipt should state:

- physical width and height;
- output lattice dimensions, tile count, spacing, and footprint taps;
- content stage and CPU, GPU, exact, or approximate source;
- terrain, plan, forest, density, or other expensive evaluations per output;
- cold, warm, and cache-bypass state;
- first-coarse, target-ready, generation, packing, transfer, and validation
  boundaries;
- resident and transient bytes; and
- error or summary-agreement measures against a finer accepted source.

### Selective volumetric terrain direction

Preserve the accepted 2D macro landscape as the planning spine and cheap
default. A useful conceptual density composition is:

```text
density(x,y,z)
  = planned_surface(x,z) - y
  + regional_gate(x,z) * volumetric_modifier(x,y,z)
  + sum(bounded_landmark_density(x,y,z))
```

This is a design model, not a locked implementation API. Its intended
properties are:

- `planned_surface` retains cheap continental, relief, hydrology, coast,
  traversability, and site queries;
- `regional_gate` is zero outside explicitly selected formation regions;
- regional modifiers have finite X/Y/Z influence and normally evaluate only
  in a bounded vertical envelope around the planned surface;
- exact generation samples coarse 3D lattice nodes and interpolates between
  them rather than evaluating many octaves independently for every block;
- unaffected chunks and vertical sections skip volumetric work;
- rare arches, tors, sea stacks, hoodoos, and windows can use bounded
  constructive or signed-distance shapes with optional noise instead of
  forcing one global density language; and
- caves and other subtractive subsurface systems remain independently seeded
  and budgeted even if they later share density-composition machinery.

The intended quality gain is real volumetric topology: shelves, undercuts,
rock shelters, arches, perforated ridges, more expressive cliff silhouettes,
and geology that can contain several solid/air transitions in one column.
Ordinary rolling terrain should not pay the same cost merely because those
formations exist somewhere in the profile.

### Topology-aware context and periodic terrain

The reusable engine contract lives in
[`bounded-world-topology.md`](bounded-world-topology.md). Macro planning adds
these content and evaluation obligations.

Multiple dimensions may generate and simulate concurrently. There is no
global “current topology.” An immutable per-dimension context supplies seed,
profile descriptor, topology, and vertical facts to each job. Samplers and
planners retain the compact facts they repeatedly need; local interpolation,
density, and signed-distance calculations operate in one Euclidean work lift.
Only identity, neighborhood, bounds, displacement, caching, reads, writes, and
presentation lifts cross topology-aware interfaces.

Keep three positions distinct:

1. canonical identity for plan ownership, random identity, cache keys,
   persistence, and output;
2. a target-relative work lift for bounded planning and geometry; and
3. an observer-relative presentation lift for rendering.

This prevents passing a large context into every arithmetic leaf without
letting a planar assumption leak into a topology boundary.

Periodic density does not require higher-dimensional noise by default. An
X-periodic 3D lattice remains three-dimensional by wrapping its X lattice
identity; a torus also wraps Z. Values and horizontal derivatives must agree:

```text
D(x + period_x, y, z) = D(x, y, z)
dD/dx(x + period_x, y, z) = dD/dx(x, y, z)
```

The current wrapped-lattice method requires horizontal scales to divide the
period. The 6,144-block cylinder admits a useful vocabulary including 32, 48,
64, 96, 128, 192, 256, 384, 512, 768, 1,024, 1,536, and 2,048 blocks. Prefer
those compatible cells and fields. Circle embedding raises a cylinder-periodic
3D evaluation to 4D and a torus-periodic one to 5D; reserve it for an
exceptional field with measured value rather than making it the general
volumetric path. Every domain warp affecting a periodic coordinate must also
preserve the period.

Bounded formations use one canonical start and evaluate through the work lift
nearest the target. Influence diameters below half the relevant period avoid
nearest-image ambiguity. Do not suppress expensive formations near a periodic
seam as a general optimization: that creates a permanent featureless meridian
around an arbitrary coordinate convention. A deliberate tectonic scar, ocean
belt, finite cap, or excluded atlas region may be real landscape intent; a
family that is not seam-safe should otherwise reject the whole periodic
profile/topology pair until implemented.

Periodic macro planners use wrapped graph adjacency and shortest displacement,
not planar routes followed by final modulo. On a cylinder, rivers may cross X
while Z remains unbounded. A torus has no external edge, so drainage must end
in explicit oceans, lakes, wetlands, closed basins, or other sinks and must
detect directed cycles. A monotonically descending river cannot wrap a closed
loop and return to its starting elevation.

### Distant representation contract

Exact chunks, research tools, and distant terrain must consume the same
accepted plan, but they do not need identical geometry.

- Continuous fields should omit, filter, or summarize frequencies too small
  for the requested spacing instead of point-sampling them into aliases.
- Narrow planned rivers, ridges, islands, paths, and shore bands need
  footprint-aware coverage, extrema, or topology facts when one point cannot
  represent them.
- Volumetric formations should expose bounded far facts such as formation
  identity, top silhouette, conservative height range, dominant material,
  solid/void coverage, or important opening bounds.
- A distant heightfield may omit an arch underside or cave interior, but it
  must preserve the visible skyline and any opening large enough to matter at
  that scale.
- Near procedural terrain may evaluate a sparse density product; canonical
  chunks remain the exact block authority and replace it without revealing an
  unrelated shape.
- Caches and roll-ups must be keyed by every plan, field, filtering, and
  representation revision that affects output.

This is how broad extent can remain cheap without becoming dishonest. A
screen-sized 500 km view should evaluate screen-scale terrain and explicit
feature summaries, not billions of block-scale samples. Conversely, merely
increasing point spacing while allowing rivers, islands, or arches to vanish
is not an accepted performance optimization.

### Performance acceptance for new terrain mechanisms

Any first volumetric tactical should benchmark at least:

1. ordinary no-formation terrain, proving the selector and skip path;
2. a dense regional-modifier hotspot;
3. a bounded landmark hotspot;
4. exact chunk generation and a representative movement/streaming workload;
5. the nearest procedural representation that evaluates density; and
6. a coarse representation consuming only declared summaries; and
7. plane and periodic-cylinder controls plus a seam-crossing hotspot whenever
   the mechanism claims periodic support.

Record output points, 2D and 3D evaluations, affected sections, cache
behavior, time boundaries, bytes, and visual/error evidence. Compare the
ordinary control and hotspot separately: averaging sparse expensive work
across a favorable continent can hide an unacceptable local stall, while
benchmarking only a showcase hotspot can hide a universal selector tax.

The first standard broad footprints should remain 65,536 and 131,072 blocks
so they connect to existing receipts. A future 524,288-block, display-matched
receipt should be added only after spacing above 1,024 has a scale-aware
summary contract. Platform evidence should include native and browser hosts;
game-facing promotion must additionally retain the affected Android, XR, and
dedicated-generation boundaries.

No universal percentage budget is selected yet. A tactical must declare its
proposed ordinary-path overhead, hotspot cost, memory ceiling, and visual
benefit before implementation. The default design pressure is near-zero
volumetric work outside selected regions, bounded work inside them, and a
fixed-cost distant representation whose expense grows with visible samples
rather than covered chunks.

## Review From Eagle Eye To Walking Scale

Review must begin before final blocks.

### Plan and field maps

For the same positive and negative seeds, inspect:

- raw fields;
- provisional and final height;
- selected landform and formation families;
- coast family, coastal distance, and shelf grade;
- drainage basins, river graph, reach order, lakes, wetlands, and outlets;
- transition weights and direct endpoint adjacencies;
- site, route, landmark, reservation, and exclusion layers; and
- composite maps showing where several accepted plans interact.

Single-layer maps catch field defects. Composite maps catch worlds whose
individually plausible systems disagree.

### Useful distributions

Measure only facts that express the design:

- land/ocean fraction, component sizes, inland distance, and shoreline
  complexity;
- coast-family coverage, coherent run length, beach width, and cliff
  adjacency;
- mountain/range extent, valley continuity, slope, roughness, and pass
  availability;
- river connectivity, junction density, reach length, width, tortuosity,
  terrain-gradient alignment, and outlet completion;
- lake area, depth, spill level, island count, and open/closed-basin ratio;
- anabranch, delta, confluence, waterfall, wetland, and pond frequency;
- formation-family coverage, overlap, separation, and transition ratio;
- ecological endpoint/shoulder adjacency and local feature density;
- landmark spacing, route connectivity, reserved negative space, and bounded
  grading; and
- authoritative water closure, wake stability, and generated-fluid work.

These are diagnostic constraints, not an objective function. A measurement
can reveal smooth bands or accidental checkerboards; it cannot overrule an
obvious visual defect.

### Rendered review

Every accepted family needs:

- regional or horizon-scale silhouettes;
- oblique views that reveal coast, valley, water, and formation relationships;
- walking-scale approaches and transitions;
- more than one seed and more than one regional center;
- ordinary areas as well as showcase hotspots;
- first-drawable inspection before later decoration obscures geometry; and
- final production pixels after surfaces, vegetation, and landmarks arrive.

The human description worth preserving is evidence. “Peaceful spring-fed
creek,” “rocky exposed coast,” “broad shallow basin,” or “long quiet valley
before the ruined pass” is more useful direction than an unlabelled screenshot
collection.

## Measured Cost Baseline

Tactical
[`258`](../tactical/258-mclone-macro-terrain-performance-baseline.md)
establishes the first normalized same-host baseline. It measures the
production point sampler, bounded preview compiler and packer, exact
surface/decorated generation, and fixed-budget World Explorer streaming. It
does not time a synthetic noise stand-in or imply that a distance alone
defines a workload.

On the 2026-07-26 Ubuntu / Ryzen AI 9 365 host at `e1d0341b`, release plane
point sampling took:

| Footprint and spacing | Points | Median |
|---|---:|---:|
| 65,536 blocks at spacing 1,024 | 4,225 | 3.229 ms |
| 131,072 blocks at spacing 1,024 | 16,641 | 9.833 ms |
| 499,712 blocks at spacing 2,048 | 60,025 | 33.283 ms |
| 499,712 blocks at spacing 1,024 | 239,121 | 97.892 ms |
| 499,712 blocks at spacing 512 | 954,529 | 392.000 ms |

The equivalent 384-chunk X-periodic point lanes took 4.277, 11.452, 40.071,
111.483, and 448.237 ms respectively. The three largest products therefore
paid about 14-20% for the current periodic embedding. A production 131 km Base
preview compiled in 7.746 ms on the plane and 8.464 ms on the cylinder, then
packed its 2,130,048 bytes in 0.477 ms on the plane. A 65 km Cover preview
declared 4,225 lattice points and 21,125 terrain evaluations and compiled in
9.563 ms on the plane.

Exact generation remains a separate cost class. Across three independent
3-by-3 receipts, plane medians were 7.905 ms for surface, 32.643 ms for cold
decorated generation with 49 dependency chunks, and 3.504 ms for a warm
49-hit decorated request. Cylinder medians were 13.243, 55.748, and 3.928 ms.
The topology premium is therefore workload-dependent rather than one global
factor.

The completed native-window World Explorer smoke reached coarse terrain in
123.906 ms and its complete target in 394.627 ms. Movement frames averaged
1.952 ms with a 3.659 ms p95. All 160 slots were ready with zero pending work
at every inspected checkpoint; the six captured views showed continuous
complete terrain.

These are baselines, not budgets. Future 3D work must add ordinary-path,
regional-hotspot, bounded-landmark-hotspot, and far-summary lanes. GPU
execution, browser transfer, and presentation remain separate measurements;
none is inferred from CPU compile time.

## Current Capability And Gap Ledger

| Concern | Current capability | Planning gap |
|---|---|---|
| raw macro fields | continentalness, relief, ruggedness, ridges, mountain detail, climate, bathymetry, water morphology | no unified classified landscape intent |
| terrain | continuous heightfield with accepted mountain detail and derived slope/exposure | no explicit range, basin, pass, or terrain-corridor plan |
| sampling/representation | cheap absolute-coordinate CPU/GPU heightfield preview, a fixed ten-level 131 km horizon, and normalized point receipts through roughly 500 km | no filtered spacing-2,048 review product or sparse volumetric near/far contract |
| coast | land/ocean threshold, shelves, deep basins, sand beach recipe, schema-14 coast metrics, and a source-grounded first-family contract | no live coast classifier, geometry response, rocky direct-water outcome, or coherent alongshore transition |
| major rivers | warped zero-contour corridor with width/depth/bank morphology and receiving-outlet correction | no persistent macro drainage topology, tributary hierarchy, or named major reaches |
| streams | bounded 91-96-block valley-following source-to-river plan | one peaceful family, not a general network |
| ponds/lakes | river-adjacent wetland pools and shared local lake mechanism | no closed-basin or spill-level plan |
| islands/deltas | continental islands and accidental contour loops | no anabranch, braid, delta, lake-island, or compound-water family |
| geology | exposed stone response and reusable local/structure mechanisms | no regional formation intent or live 3D formation family |
| ecology | climate-aware conifer, alpine, steppe, meadow, and woodland recipes | limited coordinated transitions and no shared regional formation permissions |
| landmarks | bounded stream starts/pieces and generic structure architecture | no natural-landmark, route, claim, or cross-family arbitration layer |
| review | production field maps, cards, receipts, fingerprints, Terrain Lab, and exact chunks | no concrete multi-system review contract yet; add facts only when selected consumers need them |

Do not hide these gaps by calling existing mechanisms “supported.” A shared
lake feature does not make basin lakes live. A structure kernel does not make
macro landmarks planned. A river biome and water blocks do not establish a
drainage network.

## Recommended Planning Sequence

This sequence is a decision and evidence order, not a promise to implement
every row before shipping any smaller improvement.

1. **Preserve the measured cost contract**
   - Tactical 258 establishes explicit 65.5 km, 131 km, and roughly 500 km
     sampling workloads plus exact and streaming controls;
   - rerun the named lanes when macro planners or selective 3D work change
     their production paths;
   - keep generation, packing, transfer, GPU execution, validation, and
     presentation separate; and
   - defer a generic “composite review map” until at least two concrete
     planned systems need facts that existing maps cannot show.
2. **Resolve coastal character**
   - Tactical 259 completed the cross-era survey, current three-seed baseline,
     and first bounded classifier contract;
   - implement geometry separately from surface material;
   - prove sandy, gravel, rocky, cold-response, and ordinary direct-water
     outcomes with coherent transitions; and
   - retain the plane/cylinder topology and Tactical 258 performance
     controls.
3. **Select major-water topology**
   - compare hierarchical boundary, terrain-derived drainage, and hybrid
     planned-network maps on the same terrain;
   - measure topology, terrain alignment, bounded cost, and periodic seams;
   - select the smallest system that produces convincing connected geography.
4. **Compose basins and compound water forms**
   - add one broad shallow basin/lake family with spill semantics;
   - add one anabranch, braid, or delta family that deliberately creates
     small islands;
   - retain fixed water planes and authoritative wake evidence.
5. **Add regional geological formation intent**
   - route a small number of mutually legible formation families;
   - compare placed rocks, structure-shaped volumes, and gated density;
   - benchmark ordinary controls and dense formation hotspots separately;
   - prove exact geometry and its declared near/far summaries together;
   - let geology participate in coast, river substrate, surface, and ecology.
6. **Expand coordinated ecological regions**
   - add endpoint recipes and explicit shoulders as bundles of surface,
     vegetation, clutter, ambience, and local landmarks;
   - preserve the terrain and water identity beneath them.
7. **Plan routes, landmarks, settlements, and history**
   - expose passes, crossings, shelter, water, resources, and reserved space;
   - prove deterministic overlap and bounded grading;
   - layer paths, ruins, settlements, and regrowth without erasing the
     landscape explanation.
8. **Grow subsurface relationships**
   - use overburden, geology, surface water, and regional intent to place
     caves, underground landmarks, and bounded connectors;
   - keep surface and underground topology inspectable rather than globally
     coupling every density term.

Each continuing implementation should remain a bounded tactical. This topic
supplies the shared map and precedence; it is not authorization for one
enormous “finish terrain” change.

## Open Decisions

- Which river skeleton supplies the best balance of convincing topology,
  terrain relation, deterministic bounded work, and stable water semantics?
- What selector scale, transition width, and suitability thresholds give
  coherent sandy, gravel, and rocky runs without hiding accepted terrain or
  outlets?
- What is the smallest canonical tile and halo that can support drainage
  basins, spill levels, and periodic seams without visible planning cells?
- Which facts belong in point-sampled regional intent versus bounded plan
  records?
- When does an internal-mutable reconstructed plan become a versioned stored
  record?
- How should geological resistance alter river and coast geometry without
  requiring a simulation?
- Which negative-space and landmark-density budgets belong to regional
  recipes?
- What summary is sufficient for a future multiscale terrain renderer to
  preserve lakes, arches, cliffs, islands, and routes?
- What ordinary-path overhead and dense-hotspot latency are acceptable for
  the first gated 3D density modifier?
- Which vertical envelopes, density-cell sizes, and octave cutoffs preserve
  formation identity without paying for invisible detail?
- What filtered summaries are sufficient before spacing 2,048 and a
  roughly 524 km horizon become valid review products?
- Which topology-context and lifted-query interfaces are sufficient for macro
  planners without becoming a general-purpose god object?
- Should the first periodic horizon cap at one lift, instance several lifts
  from canonical residency, or wait for topology-specific presentation?
- How should player-modified terrain invalidate, retain, or annotate discovery
  and locator facts derived from the original plan?

Resolve these through focused maps, bounded prototypes, and inspected pixels.
Do not answer them by building a general-purpose worldgen framework in
advance.

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`still-life-and-tectonic-reference.md`](still-life-and-tectonic-reference.md)
- [`modern-minecraft-reference.md`](modern-minecraft-reference.md)
- [`jjthunder-to-the-max-reference.md`](jjthunder-to-the-max-reference.md)
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md)
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md)
- [`vanilla-terrain-lod.md`](vanilla-terrain-lod.md)
- [`bounded-world-topology.md`](bounded-world-topology.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../structures.md`](../structures.md)
- [`../tactical/220-mclone-overworld-rivers-and-wetlands.md`](../tactical/220-mclone-overworld-rivers-and-wetlands.md)
- [`../tactical/222-bounded-valley-stream-structures.md`](../tactical/222-bounded-valley-stream-structures.md)
- [`../tactical/225-mclone-watercourse-morphology-and-coastal-outlets.md`](../tactical/225-mclone-watercourse-morphology-and-coastal-outlets.md)
- [`../tactical/228-production-large-fields-in-terrain-lab.md`](../tactical/228-production-large-fields-in-terrain-lab.md)
- [`../tactical/244-lod-native-vegetation-presentation.md`](../tactical/244-lod-native-vegetation-presentation.md)
- [`../tactical/246-vanilla-fast-macro-terrain-preview.md`](../tactical/246-vanilla-fast-macro-terrain-preview.md)
- [`../tactical/249-cross-platform-procedural-horizon-proof.md`](../tactical/249-cross-platform-procedural-horizon-proof.md)
- [`../tactical/259-modern-and-historical-coast-reference-survey.md`](../tactical/259-modern-and-historical-coast-reference-survey.md)
