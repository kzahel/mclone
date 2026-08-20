# Streams Reflowing Reference Study

Topic: `streams-reflowing-reference`

Status: **Streams Reflowing 2.12.6 for Minecraft 1.20.1 Forge was
inspected on 2026-08-20 from its public description, release notes, and
decompiled distribution jar. It constructs finite regional drainage plans,
persists compact river networks, carves ordinary Minecraft water, and carries
downstream current in a separate per-column vector field. This is useful
evidence for semantic water current and compact reach reconstruction. Its
regional preprocessing, finite plate boundaries, caching, and generation
stalls do not satisfy Mclone's cheap arbitrary-point Distant Terrain contract.
No production planner, terrain, water, simulation, or renderer change is
authorized by this study.**

## Scope And Short Answer

This topic records a comparative implementation study of
[Streams Reflowing](https://www.curseforge.com/minecraft/mc-mods/streams-reflowing),
a Minecraft mod by NiceJohn. It answers three separate questions:

1. how the mod creates streams that follow terrain, join lakes, and reach
   lower water;
2. how ordinary source water can visibly and physically carry downstream
   current; and
3. which parts of that architecture fit Mclone's deterministic streamed
   world and procedural-horizon LOD contracts.

The compact answer is:

```text
sample a finite regional height grid
  -> Priority-Flood and D8 receiver forest
  -> flow accumulation, lakes, reaches, and outlets
  -> refined and indexed segment network
  -> per-column bed, bank, surface, and downstream-vector query
  -> carve ordinary water plus containment and repair

ordinary water blocks
  + separate per-column downstream direction
  -> animated surface current and entity drift on source water
```

The first half solves a different problem from Mclone's current LOD. It makes
queries cheap only after an expensive regional plan exists. Mclone needs to
sample an arbitrary coordinate, scale, and topology lift cheaply without
constructing every intervening drainage basin. The second half is broadly
portable as a clean design concept: water level and downstream current do not
need to be the same representation.

This is design evidence, not a dependency or porting proposal. Streams
Reflowing is All Rights Reserved. No implementation, constants, authored
tables, assets, or decompiled code may be copied into Mclone.

## Specimen And Reproduction

| Field | Receipt |
|---|---|
| Project | [CurseForge project page](https://www.curseforge.com/minecraft/mc-mods/streams-reflowing), project ID `1581408` |
| Inspected version | 2.12.6 for Minecraft 1.20.1 Forge |
| Artifact | `StreamsReflowing-1.20.1-forge-2.12.6.jar` |
| Download | [Pinned Modrinth CDN artifact](https://cdn.modrinth.com/data/oLS8HdJ1/versions/aheBMj3k/StreamsReflowing-1.20.1-forge-2.12.6.jar) |
| SHA-512 | `074421e9583db8b0b4dbd9b5d520ca6353d40ddaf20d018d4070aa297f1b7819be3213ac423622d4c95d334a4245a86f02cb32b17b1731a9c250472958ef9acd` |
| License | All Rights Reserved, as declared by CurseForge and the jar metadata |
| Inspection tool | [CFR 0.152](https://www.benf.org/other/cfr/) |

The ignored local research specimen lives under
`reference/streams-reflowing/`. It includes the artifact, CFR jar, decompiled
output, and a local provenance note. Those files are deliberately excluded
from version control.

The specimen can be reproduced outside the repository:

```bash
study_dir=$(mktemp -d /tmp/mclone-streams-reflowing.XXXXXX)

curl -fsSL \
  'https://cdn.modrinth.com/data/oLS8HdJ1/versions/aheBMj3k/StreamsReflowing-1.20.1-forge-2.12.6.jar' \
  -o "$study_dir/StreamsReflowing-1.20.1-forge-2.12.6.jar"
shasum -a 512 \
  "$study_dir/StreamsReflowing-1.20.1-forge-2.12.6.jar"

curl -fsSL 'https://www.benf.org/other/cfr/cfr-0.152.jar' \
  -o "$study_dir/cfr-0.152.jar"
java -jar "$study_dir/cfr-0.152.jar" \
  "$study_dir/StreamsReflowing-1.20.1-forge-2.12.6.jar" \
  --outputdir "$study_dir/decompiled"
```

CFR output is not buildable source. It can lose local names, duplicate
declarations, or imperfectly reconstruct complex control flow. Findings below
therefore rely on class responsibilities, data flow, repeated behavior across
call sites, constants, persistence formats, and public release descriptions
rather than on isolated decompiler syntax.

## Public Product Claims

The project page says streams read existing terrain, descend through varying
elevations, pool into lakes, and drain toward rivers and oceans. It also gives
existing vanilla or modded rivers an optional seaward current and lets water
continue across source blocks. These are three separate mechanisms in the
inspected code, not one fluid simulation.

The public performance contract is equally important. On first load the mod
queries terrain height over the spawn area, can leave world creation appearing
stalled, can delay chunks while entering unexplored regions, recommends world
pregeneration, and offers quality presets that change terrain accuracy and
watershed extent. The inspected Forge config describes approximate one-time
costs on heavy Tectonic terrain from four seconds per region at `POTATO` to
about nineteen seconds at default `MEDIUM`, one minute at `HIGH`, and much
longer at the highest settings. Those estimates are supplied by the mod, not
independent Mclone benchmarks.

Release 2.6.0 changed the normal representation to plain
`minecraft:water`. Its
[release notes](https://www.curseforge.com/minecraft/mc-mods/streams-reflowing/files/8377589)
say direction is computed on the server per chunk and imposed on ordinary
water, with smooth angles, bank following, width flare, and softened current
edges. The old custom directional fluid remains dormant for compatibility in
that release line. The inspected 2.12.6 Forge path still normally returns
ordinary water; a compatibility path can select the registered stream fluid
when the separate Flowing Fluids mod is present.

## Observed Ownership

The main implementation responsibilities are:

| Class or family | Observed responsibility |
|---|---|
| `PlateLattice`, `PlateDomain` | jittered finite regional ownership and bounds |
| `LowResGrid` | sampled or estimated terrain heights on the regional grid |
| `NetworkTracer` | fill, receivers, accumulation, lakes, reach extraction, refinement, and mouth handling |
| `RiverNetwork`, `RiverSegment`, `Lake` | retained compact hydrology facts |
| `NearestRiverIndex` | spatial lookup of nearby segments |
| `RiverEngine` | continuous per-column channel, bed, surface, distance, grade, and flow queries |
| `RiverCarverFeature` | terrain excavation, water placement, banks, containment, drops, and repairs |
| `StreamFlowGrid` | current angles for generated streams |
| `RiverFlowField`, `RiverFlowFields` | current orientation for existing biome rivers |
| `FlowingFluidMixin` | ordinary-water flow override and source-water spill behavior |
| `StreamFlowNet` | server-to-client flow-grid publication |
| `StreamPrefetcher`, `ChunkStatusDefer` | background preparation and chunk-generation admission |
| `PlateRiverCache`, `FileNetworkStore` | in-memory construction deduplication and saved regional plans |

This ownership separates four products that can be evaluated independently:

1. drainage topology;
2. continuous stream geometry and water levels;
3. realized blocks and containment; and
4. presentation and gameplay current.

## Regional Drainage Planning

### Finite Jittered Plates

`PlateLattice` assigns a coordinate to the nearest deterministically jittered
lattice seed. Each `PlateDomain` supplies one finite solve boundary. The
default `MEDIUM` preset uses:

- 16-block hydrology grid spacing;
- 768-block plate scale;
- 32-block terrain sampling spacing;
- an 8-block valley-snap search radius; and
- one neighboring plate of prefetch reach.

The presets range from a 24-block grid over 512-block plates at `POTATO` to a
6-block grid over 2,048-block plates at `MAX`. Higher quality therefore
increases both resolution and potential drainage extent.

These plates are not merely cache tiles over an independently final terrain
function. Their finite boundaries participate in the drainage construction.
Boundary cells and cells at or below sea level seed the flood. Optional
nearby-water connection can examine neighboring plans, but it is disabled by
default and is explicitly described as an added cost. This is a practical
regional approximation, not proof of one exact unbounded watershed.

### Priority-Flood And Receivers

`NetworkTracer.build` performs the following coarse solve:

1. Sample the terrain height for every in-domain grid cell.
2. Seed a priority queue with ocean and domain-boundary cells.
3. Flood inward over eight-neighbor adjacency. Each newly reached cell takes
   the greater of its actual height or its predecessor's filled height plus a
   small epsilon. This removes undrained pits and gives flats strict descent.
4. Select the steepest lower filled neighbor as each cell's D8 receiver,
   accounting for diagonal distance.
5. Sort cells from high to low and add each cell's wetness contribution to its
   receiver.
6. Mark above-sea cells as channels once accumulated flow exceeds the
   configured threshold.
7. Prune touching or parallel artifacts, detect branches and confluences, and
   extract downstream chains.

The crucial property is that accepted cells already belong to a directed
receiver forest. Downstream order is established before any final water
column is carved. This avoids the failure mode of selecting a locally lower
step that later has to rise because a farther sink was never part of the
decision.

### Lakes And Spills

The fill difference identifies candidate depressions. The planner groups and
qualifies them, rejects unsuitable sizes and depths, and runs finer local
flood work for retained basins. A `Lake` stores its occupied mask, surface
level, and relationship to incoming or outgoing reaches. Climate wetness can
lower or eliminate water in dry basins.

This makes lake surface and outlet elevation explicit plan facts. A lake is
not inferred later from coincidentally flat terrain or from fluid spreading.

### Reach Refinement

Coarse downstream chains are converted to continuous river segments through
several deterministic passes:

- move intermediate points laterally toward lower nearby ground;
- force reach elevations to remain non-increasing downstream;
- follow ground and clamp against bank constraints;
- resample more densely than the planning grid;
- apply multi-frequency lateral meander, reduced on steep grades;
- optionally search sideways to keep the path between terrain banks;
- smooth the polyline repeatedly; and
- extend mouths across shallow coasts toward deeper receiving water.

Widths grow with accumulated discharge and are clamped by configured base and
maximum widths. `RiverSegment` then retains endpoints, endpoint elevations,
and endpoint widths. `NearestRiverIndex` permits a point query to find nearby
segments without following the receiver forest.

This is an important representation lesson: expensive topology is compressed
into spatially bounded primitives before local realization begins.

## Column Reconstruction And Carving

`RiverEngine.plan(x, z)` queries nearby segments and produces a continuous
column plan containing facts such as:

- distance from the reach centerline;
- local width and channel/corridor membership;
- interpolated downstream grade;
- integer water surface and bed levels;
- bank influence and blend;
- source, terminal, junction, and outlet context; and
- horizontal downstream direction.

The carver consumes those facts after the host terrain generator has supplied
the landscape. It digs a parabolic-like bed and banks, fills ordinary source
water to the selected surface, clears space above it, and applies separate
handling around lakes, sources, confluences, lips, drops, and mouths.

The implementation also contains extensive defensive realization work:

- containment berms around exposed water;
- support beneath gravity-affected bed materials;
- cave-roof and bank-breach plugs;
- local surface and water-level correction;
- cross-chunk deferred seam work; and
- scheduled fluid updates.

This differs from Mclone's accepted watercourse fixed-point standard. Mclone
has required bounded source planes, static containment, complete authoritative
wakes, and zero subsequent fluid mutations. Streams Reflowing is willing to
repair terrain after carving and modify host fluid behavior. Its attractive
results do not show that the same block realization would pass Mclone's
stronger closure gate.

## Current Is Independent Of Water Level

This study's most portable finding is the separation of water occupancy from
horizontal current.

`StreamFlowGrid` retains one encoded angle per X/Z column in a chunk. For the
mod's own streams, it derives the raw direction from the local segment and
column plan. It then expands coverage beyond the exact centerline, smooths
angles, follows bank curves, accounts for changing width, and feathers the
transition into still water. The server prebuilds and sends the finished grid
to watching clients.

`FlowingFluidMixin` consults that field when ordinary water's flow vector is
queried. A source block can consequently return a horizontal downstream
vector even though its block state remains a source. The same semantic flow
query feeds water presentation and ordinary water/entity interactions. Other
mixins tune boats and item drift, while aquatic mobs can be exempted.

A separate interception changes the host's water-hole decision so water does
not necessarily stop spreading horizontally as soon as it encounters source
water below. This permits stepped cascades to continue across source pools.
It is a Minecraft-specific rules patch and should not be confused with the
flow-grid representation itself.

The useful general contract is:

```text
WaterSurface {
    occupancy,
    surface_level,
    downstream_vector,
    current_strength,
    fall_or_rapid_context,
}
```

Flat or stepped water can remain statically contained while rendering,
physics, particles, audio, debris, and AI consume a coherent current. A
sloped mesh or unstable non-source fluid state is not required to communicate
movement.

## Existing-River Current

The mod does not derive vanilla-river current from the generated stream
network. `RiverFlowField` builds another bounded grid over biome-classified
river, land, and ocean cells. Connected river components select an ocean
contact or fallback orientation, propagate distance from the chosen outlet,
and turn the predecessor relation into a smoothed direction field.

This is useful evidence that current can be layered over water whose original
generator did not expose source and sink metadata. It remains a regional
orientation heuristic. It does not turn an arbitrary existing river mask into
an exact continental drainage graph.

## Persistence, Prefetch, And Admission

Regional networks are expensive enough to be lifecycle objects:

- `PlateRiverCache` deduplicates concurrent construction through futures and
  retains a bounded set of networks;
- `FileNetworkStore` saves segment and lake facts with format, seed, and
  configuration fingerprints;
- `StreamPrefetcher` uses background and urgent worker pools to prepare plans
  around and ahead of players; and
- `ChunkStatusDefer` can delay host generation stages until relevant plans
  are ready, including an explicit compatibility path for aggressive parallel
  chunk loaders.

The viewport or player does not seed geography, and persisted plate identity
can make repeat visits stable. Nevertheless, an arbitrary cold query cannot
be answered from coordinates alone at point-sample cost. It may have to build
a complete plate, wait for terrain samples, load neighboring plans, or block a
chunk stage.

## Comparison With Mclone

| Concern | Streams Reflowing | Mclone contract or evidence |
|---|---|---|
| Drainage order | finite plate Priority-Flood, D8 receivers, and accumulated flow | Tactical 267 proved the same bounded representation; production pointwise rivers do not own a global receiver forest |
| Query after planning | indexed segment lookup is cheap | compact primitives and indexed reconstruction are already selected research directions |
| Cold arbitrary query | can build or load a regional network and may stall generation | procedural-horizon LOD must sample distant arbitrary points cheaply, including after teleport |
| Boundaries | plate edges seed the solve; optional neighbor connection repairs some terminals | plane, cylinder, and torus require canonical identity, shared facts, and lift independence |
| Cache meaning | persisted plans are required amortization | cache state must affect time only, never semantic output |
| Water levels | monotone reach grade, integer surfaces, explicit drops and lakes | stable integer source planes and explicit drop transitions remain compatible concepts |
| Current | separate per-column vector over ordinary water | no equivalent shared semantic current contract exists yet |
| Containment | berms, repairs, scheduled updates, and modified fluid rules | accepted Mclone water seeks static closure and zero post-wake mutations |
| Terrain relationship | inspect host terrain, then carve and repair a stream through it | selected Mclone direction lets shared landscape facts shape both terrain and water |
| LOD | no cheap coordinate-pure equivalent of an unbuilt plate | current Distant Terrain is a direct procedural-horizon geometry clipmap over semantic terrain sources |

### Relationship To Failed Production Streams

Mclone field revision 10 attempted local quantized river levels selected from
fixed probes and drop stencils. A locally downhill decision could later rise
because no persistent source-to-sink order existed. Streams Reflowing avoids
that exact failure by completing its receiver forest and monotone reach grade
before column realization.

Mclone's later bounded valley streams deliberately narrowed the claim. They
find one nearby major-river sink, search a 48-to-96-block route, and assign
monotone integer reaches backward from known sea-level water. They provide a
stable spring, creek, and fall family, not continental drainage.

### Relationship To Tacticals 267 And 270

Tactical 267's fixed 6,144-by-6,144 research plan already demonstrated:

- Priority-Flood-like drainage from explicit ocean and protected sinks;
- acyclic receivers and accumulated flow;
- stream order, confluences, basins, spills, and divides;
- compact indexed reconstruction; and
- exact periodic reconstruction for its fixed cylinder corpus.

Streams Reflowing independently validates that this bounded representation can
produce a convincing game feature. It does not resolve the production blocker
found afterward: moving the finite solve changes facts at the same absolute
coordinate.

Tactical 270 therefore asks a different question. Its canonical shared
hierarchy and feature-owned bounded graphs must remain exact under request,
travel path, schedule, cache, viewport, partition, and topology lift changes.
The mod's regional plate construction is evidence for the value of retained
reach primitives, but not evidence that those stronger invariants are met.

### Relationship To Distant Terrain

The current LOD is the shared procedural-horizon geometry clipmap in
`mclone-terrain-view`. It may ask for thousands of samples across kilometers
of previously unseen land and must support a cold teleport without generating
the chunks between the old and new positions.

Its useful conceptual query remains:

```text
sample(seed, dimension, topology, x, z, scale) -> terrain summary
```

Streams Reflowing instead provides:

```text
plan = build_or_load_plate(seed, finite terrain samples)
sample(plan, x, z) -> river summary
```

The second line can be fast, but the prerequisite is not a cheap arbitrary
sample. Making a plate build an implicit miss path beneath the LOD evaluator
would turn zoom, pan, teleport, and high clipmap levels into unpredictable CPU,
memory, I/O, and scheduling work. Pregeneration would sacrifice one of
Mclone's useful product distinctions.

## Inspiration Worth Carrying Forward

### 1. Add A Shared Semantic Current Contract

Water occupancy, water surface elevation, and current should be separate
facts. A future shared contract could expose at least:

- horizontal direction;
- strength or discharge class;
- still, flowing, rapid, and fall context;
- owning reach or water-body identity where available; and
- validity or confidence for analytically approximated water.

The authoritative server can use it for entity drift and gameplay. Rendering,
audio, particles, vegetation, debris, and diagnostics can consume the same
fact without reconstructing current from mesh slope or block state. Shared
ownership must be selected before implementation; this study does not assign
the contract to an app crate.

### 2. Preserve Compact Reach Descriptors

Even a bounded feature-owned Mclone river should compile into endpoint,
elevation, width, discharge, bounds, and downstream-direction primitives.
Point and LOD queries can enumerate a finite possible-owner neighborhood,
reject non-overlapping bounds, and evaluate only nearby reaches. Runtime
queries should never walk upstream until they find a source or downstream
until they find an outlet.

### 3. Make Lakes And Spill Levels Explicit

Protected basin identity, rim, floor, water level, lowest spill, receiving
reach, and closed/open state are useful shared facts even when Mclone is
generating hydrography rather than deriving an exact watershed. This is more
stable than asking fluid simulation or a late terrain scan to discover a
lake.

### 4. Let Discharge Affect More Than Width

An accumulated or generatively assigned discharge class can coordinate:

- channel and floodplain width;
- bed depth and substrate;
- current strength;
- bank vegetation and debris;
- rapid, pool, and waterfall frequency; and
- distant visual salience.

The exact physical accumulation need not be global. A bounded graph can carry
a stable synthetic discharge budget from its owned sources and joins.

### 5. Keep Planning And Realization Separate

The mod's compact network is more valuable than its Minecraft-specific
carver. Mclone should let accepted hydrology shape provisional terrain and
then reconstruct both from the same primitives. It should not make a generic
post-terrain trench plus berm-and-repair layer the primary landscape
authority.

### 6. Treat Preparation Cost As A Product Contract

If Mclone later adopts any retained regional summary, its cold cost, maximum
dependency set, native/Wasm behavior, cache cap, persistence identity,
teleport latency, and LOD interaction must be measured explicitly. Background
prefetch improves common traversal but cannot serve as a correctness or
boundedness proof.

## Directions Not Supported By This Study

Do not infer authorization to:

- put a Priority-Flood plate build beneath every distant terrain query;
- use discovery, viewport, or cache residency as geographic input;
- treat finite plan edges as natural sinks on the plane, cylinder, or torus;
- copy the mod's decompiled implementation or constants;
- reintroduce a custom water block merely to store current direction;
- weaken Mclone's static containment and complete-wake acceptance without a
  separately reviewed gameplay reason;
- hide exact-versus-LOD river disagreement with a visual feather; or
- claim exact hydrology for a bounded generative graph.

## Recommended Next Experiment

If this direction is resumed, the smallest informative experiment is not a
new watershed planner. Extend Tactical 270's accepted bounded feature-owned
graph candidate with clean-room synthetic discharge and current facts, then
measure one shared point-query surface against the current procedural-horizon
workload.

The experiment should require:

1. canonical feature ownership and a declared finite maximum influence;
2. exact equality under the existing plane, cylinder, torus, request, path,
   cache, schedule, partition, and lift corpus;
3. a point query that returns reach distance, water level, width, discharge,
   and downstream vector without graph traversal;
4. native and Wasm cost receipts at the actual Distant Terrain sample counts;
5. a cold teleport that cannot initiate an unbounded or transit-wide plan;
6. a structural atlas review before terrain reconstruction; and
7. no production water, renderer, or simulation adoption until the semantic
   facts justify their complexity.

Candidate C is currently a closer starting point than importing the mod's
plate model. Candidate B's hierarchy may supply broad outlet or orientation
facts only if its visible lattice and dependency cost become acceptable.

## Open Questions

- Can a bounded feature-owned river family feel connected enough without
  pretending to expose exact contributing area?
- What is the smallest current representation shared by exact water,
  procedural-horizon water, physics, and presentation?
- Can one compact reach descriptor support CPU exact generation and GPU or
  batched LOD evaluation without divergent semantics?
- Should synthetic discharge be conserved at joins, and how is its maximum
  bounded within one feature-owned graph?
- Which lake and spill relationships require a shared hierarchy rather than
  one bounded owner neighborhood?
- How should current degrade when far LOD has only a broad river summary?
- Is current authoritative gameplay state, a deterministic derived query, or
  both at different realization scales?
- Does the visual and navigational benefit justify adding current before a
  stronger production river topology exists?

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`deterministic-streamed-landscape-planning.md`](deterministic-streamed-landscape-planning.md)
- [`lod.md`](lod.md)
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md)
- [`multiscale-terrain-representation.md`](multiscale-terrain-representation.md)
- [`../tactical/222-mclone-valley-streams.md`](../tactical/222-mclone-valley-streams.md)
- [`../tactical/225-mclone-watercourse-morphology-and-coastal-outlets.md`](../tactical/225-mclone-watercourse-morphology-and-coastal-outlets.md)
- [`../tactical/265-macro-landform-grammar-research.md`](../tactical/265-macro-landform-grammar-research.md)
- [`../tactical/267-hybrid-macro-landform-plan-prototype.md`](../tactical/267-hybrid-macro-landform-plan-prototype.md)
- [`../tactical/270-deterministic-streamed-landscape-planner-research.md`](../tactical/270-deterministic-streamed-landscape-planner-research.md)
