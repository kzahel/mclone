# Multiscale Terrain Representation

Topic: `multiscale-terrain-representation`

Status: **Narrowed research record after Human Review R1 on 2026-08-20.
Tactical 272's semantic parent/child witness and Tactical 273's isolated
terrain reconstruction remain technically successful: their pinned suites
pass native/Wasm traversal, partition, thread, tile-boundary, cache, and
periodic-topology checks, and parent/regional queries do not construct hidden
children. Human review found the resulting schematic range/basin terrain
slightly disappointing and did not select either family, a frozen-foundation
trial, or multiscale reconstruction as the product worldgen direction.
Production Mclone terrain remains field revision 21 and consumes none of
these facts. Preserve the exact identity, direct coarse-query,
cache-independence, topology, and selective-3D lessons; route the accepted
continental/ecoregional product direction through
[`continental-ecoregion-planning`](continental-ecoregion-planning.md).
LOD is a downstream consumer and performance obligation, not the reason to
choose Mclone's geography.**

## Scope

This topic owns one focused question:

> Can Mclone preserve cheap continental random access while gaining explicit
> geographic organization and genuinely volumetric local terrain by giving
> each scale a direct, semantically consistent representation?

It connects three concerns that are useful separately but potentially more
valuable together:

1. a deterministic multiscale plan owns continents, ranges, basins, major
   routes, coast relationships, and quiet space;
2. a cheap two-dimensional surface remains the everywhere-available terrain
   spine while bounded or regionally selected three-dimensional density adds
   shelves, undercuts, arches, irregular cliffs, and multiple solid/air
   transitions; and
3. distant products query direct coarse facts and conservative summaries
   instead of evaluating the detailed generator at sparsely separated points.

Human Review R1 answered only the first composition attempt. It did not reject
top-down planning, cheap broad queries, semantic summaries, or selective 3D
terrain. It rejected promotion of these particular schematic parent/child
range and basin courses and the assumption that one multiscale representation
should organize geography primarily to satisfy LOD. Future product work may
reuse the mechanisms independently or choose a different representation.

This is not the owner of:

- the macro-landscape vocabulary and precedence rules, which remain in
  [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md);
- streamed-planner determinism and bounded-dependency evidence, which remain
  in
  [`deterministic-streamed-landscape-planning.md`](deterministic-streamed-landscape-planning.md);
- exact Java 1.17.1 density-column preview behavior, which remains in
  [`vanilla-terrain-lod.md`](vanilla-terrain-lod.md);
- renderer clipmap residency, which remains in
  [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md);
- world-height and section-residency architecture, which remain in
  [`world-height-and-volumetric-streaming.md`](world-height-and-volumetric-streaming.md);
  or
- canonical/work/presentation lifts and periodic adjacency, which remain in
  [`bounded-world-topology.md`](bounded-world-topology.md).

## Current Motivation

The accepted Mclone macro path is unusually cheap because one absolute
horizontal coordinate produces one surface sample. Increasing sample spacing
lets a fixed output lattice cover much more physical ground. The same
heightfield constraint also contributes to the reviewed visual problem:
broad terrain can read as scalar contours and repeated hills even after
ordinary inland roughness improves.

Minecraft Alpha, Beta, Java 1.17.1, and modern Java demonstrate the local and
regional shape available from coarse three-dimensional density, but they do
not expose persistent ridge, drainage, basin, or outlet objects. The current
planner research explores those relationships without volumetric realization.
Treating either mechanism as a complete replacement for the other asks it to
solve the wrong problem:

```text
relational plan     -> where major geography belongs and how it connects
planned heightfield -> cheap surface, circulation, water, ecology, and sites
selective density   -> how accepted local terrain occupies three dimensions
semantic LOD        -> what must remain legible at the requested scale
```

## Cost Model

For a fixed vertical range, a heightfield and a density field differ more by a
large multiplier than by their horizontal asymptotic order:

```text
heightfield work ~= horizontal output samples
density work     ~= horizontal lattice samples * vertical density nodes
```

If horizontal extent and world height all scale together, the familiar
quadratic-versus-cubic comparison applies. Ordinary chunk and viewport
workloads keep height finite. Minecraft reduces density cost by evaluating a
coarse three-dimensional lattice and interpolating through it rather than
running the complete noise stack at every block.

The repository already records useful, non-normalized evidence:

- a 65-by-65 Mclone Base lattice takes about 2.0-4.7 ms on the recorded warm
  CPU paths;
- the Mclone production point sampler covers a 499,712-block square at
  spacing 2,048 with 60,025 outputs in a 33.283 ms plane median;
- the Java 1.17.1 exact column sampler consumes 33 vertical density nodes per
  retained lattice column;
- its Fast Macro representation consumes nine and measured a 3.45-3.87x
  compile speedup over Sampled Exact on its fixed corpus; and
- the default World Explorer horizon keeps fixed tile and sample counts while
  increasing physical spacing across ten clipmap levels.

These receipts do not define a universal 2D/3D multiplier. A future density
experiment must separately report ordinary skip cost, selected-region cost,
bounded-formation cost, exact chunks, near procedural representation, and
far-summary cost.

## Direct Generative LOD

Sparse point sampling and semantic level of detail are different contracts.

Sparse sampling asks the detailed generator for fewer exact points:

```text
fine generator -> wider point spacing -> interpolate or draw retained values
```

It is exact at those retained points but may alias or omit a narrow river,
island, ridge, opening, or isolated high formation.

Direct generative LOD asks for a representation owned by the requested scale:

```text
continental facts
    -> ranges, ocean envelopes, principal basins, major outlets
regional facts
    -> range branches, valleys, lakes, secondary drainage
local facts
    -> tributaries, secondary ridges, bounded formations
exact realization
    -> surface, selected density, blocks, materials, and features
```

The coarse picture may be geometrically different. It may smooth, merge,
omit, exaggerate, or conservatively bound detail. It must tell the same
geographic story.

### Semantic consistency

An accepted representation family should preserve:

- **identity:** a major feature has one stable identity across representations;
- **topology:** principal land/water connectivity and major outlets do not
  change with view scale;
- **containment:** child facts remain within influence authorized by their
  parent;
- **scale ownership:** any feature large enough to matter at a coarse scale is
  declared or conservatively summarized there;
- **bounded error:** skyline, water coverage, height range, and important
  openings have declared geometric or coverage error;
- **refinement:** finer representations elaborate accepted coarse facts rather
  than contradicting them; and
- **view independence:** camera, requested LOD, traversal, cache, and Worker
  schedule affect representation and residency only, never canonical terrain
  identity.

LOD is therefore not a world-generation input. Exact terrain remains a
function of the stored profile, seed, dimension, topology, revision, and
canonical location. A coarse query is a deterministic projection of those
facts, not an alternate world that can later change exact chunk output.

### Revision 1 mechanism result

[`Tactical 272`](../tactical/272-multiscale-semantic-refinement-witness.md)
now supplies the first executable Mclone answer. One canonical 6,144-block
owner creates a range axis and an explicit-sink basin route. Parent facts
split into two regional and four local segments. Direct parent and regional
queries stop at their requested level; detailed queries include byte-identical
parent facts rather than recomputing a different coarse picture.

The exact witness
`4ac9b52c6e04697378c6ff0dfc1b9bb25a3903f9138cf4f19e633bd54db6a78d`
passes 105 traversal/cache/window comparisons and nine seed-by-topology
cross-level corpora on native and Wasm. It reports zero projection,
parent-resolution, containment, endpoint-continuity, terminal, and hidden
detail failures. A target examines at most four parent owners; the fixed
corpus observed no more than 22 facts against the declared 57-fact bound.

This closes a mechanism question, not the landscape question:

- top-down parent/child refinement can be canonical and directly queryable;
- the same feature language and identities work on the plane, X-cylinder,
  and torus without seam suppression;
- viewport, cache, and presentation LOD need not become generation inputs;
- coarse construction can omit children, although fixed overhead dominates
  this tiny witness; and
- schematic axes and routes do not establish useful terrain, drainage,
  coastline, or volumetric quality.

The permanent Planner-atlas panel is the review instrument. Parent, regional,
local, and conservative bounds are independent presentation toggles over one
Rust-owned fact set. It should remain available even if the geometry is
rejected, because it makes future multiscale claims inspectable.

### Revision 1 reconstruction result

[`Tactical 273`](../tactical/273-semantic-terrain-reconstruction-sandbox.md)
now turns the same facts into one deliberately small heightfield experiment:

```text
flat or quiet substrate
    + compact-support range uplift
    - compact-support basin-route carve
    = isolated semantic terrain
```

Parent, regional, and local requests each select one course. Children replace
their parent course rather than stacking on it. The sandbox publishes all
three surfaces, both immediate corrections, simple visual water occupancy,
feature and distance-evaluation counts, and quantized semantic/terrain
checksums. It has no profile, chunk, biome, material, vegetation, coast, or
production-terrain integration.

The exact suite
`14250ea1a92a72246abfd256d3ffb2caca021a5805a4be692299bfecc5017f8e`
passes three seeds by all three topologies on native and Wasm. It covers
raster, reverse, even/odd, and shuffled traversal; adjacent and independently
partitioned viewports; serial/parallel compilation; and periodic lifts.
Direct coarse construction was corrected to stop before hidden children.

At 4,225 output samples, the recorded release run measured parent/regional/
local at 0.538/0.756/1.267 ms across 6,144 blocks and
7.939/16.314/34.993 ms across 65,536 blocks. The physical-extent cost grows
with bounded owners in view; refinement cost grows with one, two, and four
segments per feature. This is cheap enough for interactive research but is
not a claim about exact chunks or 500 km generation.

The permanent `Semantic terrain` Terrain Lab pane is now the review
instrument. Desktop compares four panels in a 2-by-2 grid; phone stacks them.
Map/3D navigation, flat/quiet substrate, range/basin/combined families,
plane/cylinder/torus topology, immediate correction choice, and feature
guides are independent URL state. The Wasm Worker recomputes the pinned suite
at startup and Rust supplies typed surfaces; browser code does not own
reconstruction semantics.

The first inspected view establishes that the mechanism is legible: the broad
parent corridor persists while regional and local bends move the ridge or
carve within localized correction areas. It does not establish that the
schematic corridors compose into natural geography. Independent stamp-like
forms, basin meaning, owner-boundary character, and the value of the quiet
substrate remain subjective review questions.

Human Review R1 subsequently found the result technically clear but slightly
disappointing as terrain. The isolated axes and compact corrections remain too
schematic to establish the authored continental, physiographic, ecoregional,
clearing, hydrological, and ecological composition now sought for Mclone
Overworld. No additional Revision 1 shape iteration or production trial is
selected. Keep the pane as an inspectable research artifact and use the
accepted mechanisms only when a future concrete product plan needs them.

## Candidate Representation Stack

The current hypothesis is a finite coarse-to-fine dependency DAG:

```text
coarse canonical plan facts
       |
       v
regional canonical plans and bounded graphs
       |
       v
local planned surface and bounded formations
       |
       v
exact block realization
```

Each level has canonical identities derived from the seed, dimension,
topology, planner revision, scale, and owner coordinate. Coarse facts may
constrain finer facts; no finer or discovery-time state may revise an already
accepted parent. A point or chunk consults a bounded number of facts at a
fixed finite set of levels rather than following an unbounded upstream graph.

Candidate B from the streamed-planner research is relevant because its shared
hierarchy could provide directly queryable coarse facts. Candidate C is
relevant because feature ownership reconstructs complete bounded graphs
without request-order state. Neither current candidate establishes the stack:
B visibly exposes a square hierarchy, C consists of short independent graphs,
and no accepted child-conditioning, summary, continuous reconstruction, or
selective-density contract exists.

## Tailored Precedent Survey

### Survey result

The broad architecture is not novel:

```text
coarse typed geography
    -> deterministic semantic refinement
    -> view- or query-selected detail
    -> sparse local volumetric amplification
```

Published systems establish each arrow, and two terrain systems establish
most of the first three together. The survey therefore rejects any claim that
Mclone invented semantic generative LOD, graph-before-terrain construction,
or heightfield terrain augmented with sparse implicit 3D features.

The useful research gap is narrower. None of the inspected sources
demonstrates the complete conjunction Mclone needs:

- one canonical authority serving both exact block chunks and direct coarse
  queries;
- bounded, deterministic random access without first materializing a complete
  unbounded plane or cylinder;
- request-, traversal-, cache-, partition-, Worker-, and thread-independent
  results;
- topology-native plane, cylinder, and torus ownership with seam agreement;
- conservative far summaries for water connectivity, skyline, coverage, and
  important voids;
- a near-zero-cost ordinary heightfield path plus selected implicit or density
  work; and
- measured fixed-output continental queries beside exact chunk realization.

This absence in a focused survey is a reason to run a Mclone experiment, not
proof of academic novelty.

### Closest precedent: hydrologic hyper-amplification

[Cortial et al. 2020](https://hal.science/hal-02967067v1) is the closest
representation precedent. It begins with a complete low-resolution spherical
triangulation at roughly 50 km precision. Vertices and edges carry terrain,
river, lake, sea, gully, wetness, elevation, and other landform facts. Typed
subdivision rules then add tributaries, valleys, lakes, hills, plateaus, and
mountain character at scale-appropriate levels. The paper demonstrates
roughly 50 cm final resolution and explicitly makes stochastic compute nodes
deterministic across subdivision runs with stable vertex seeds.

This is direct generative LOD, not sparse sampling of a finished fine terrain.
The coarse river network and control facts already exist; refinement
elaborates them. It is also finite whole-planet generation:

- the CPU first constructs a complete spherical base mesh;
- the reported 230,000-vertex, 460,000-triangle base took 9.2 seconds, while
  river-network parameters took about 0.5 seconds;
- the adaptive GPU representation retained more than 2 GB;
- an update averaged about 80 ms every ten frames and exceeded 100 ms in the
  most detailed views; and
- the realized terrain remains a single-valued surface mesh rather than a
  block or volumetric authority.

The rules are highly relevant to Mclone's candidate feature language. The
complete planet mesh and view-owned adaptive representation are not directly
acceptable as the authority for an unbounded block world.

### Explicit exploration-order independence

[Derzapf et al. 2011](https://www.bjoern-ganster.de/pdf/planets.pdf) builds a
complete coarse planet with continents, coasts, initial river networks, water
elevations, typed edges, and stable seeds, then refines and collapses its
surface mesh according to the camera. River-edge refinement preserves the
river type and conditions nearby valleys and mountains.

The paper identifies the exact trap relevant to Mclone: naively consuming
random values as adaptive refinement occurs can make terrain depend on the
path of exploration. It claims the same terrain for every exploration path
by deriving the base mesh deterministically and tying edge splits to stored
seeds and split levels. Its reported base mesh had about 5,000 faces and took
0.27 seconds to create.

This is strong evidence that adaptive procedural refinement can be
order-independent. It achieves that result for a finite planet whose complete
coarse mesh already exists. It does not establish bounded arbitrary access on
an unbounded domain, direct block generation, or volumetric landforms.

### Graph-before-terrain and direct hierarchical evaluation

[Génevaux et al. 2013](https://www.cs.purdue.edu/homes/bbenes/papers/Genevaux13ToG.pdf)
first constructs a hierarchical drainage graph, watershed cells, and crest
lines for a finite user-bounded domain. It then creates a continuous terrain
from compactly supported primitives in a construction tree. Portions of the
terrain can be evaluated after the whole finite construction tree exists.
This strongly supports relational planning before continuous realization,
but not streamed unbounded graph discovery.

[Génevaux et al. 2015](https://www.cs.purdue.edu/homes/bbenes/papers/Genevaux15CGF.pdf)
represents mountains, ridges, valleys, rivers, lakes, and roads as a compact
hierarchical construction tree. A point query prunes compact-support
primitives through a bounding hierarchy. Explicit LOD operators select or
blend subtrees, while continuous primitives omit high-frequency terms at low
detail.

That is close to the proposed semantic query representation. Its placements
and parameters are primarily authored, the result is a heightfield, and the
paper explicitly leaves automatic feature placement and 3D terrain as other
work. It proves a useful representation, not a canonical streamed geography
generator.

### Selective 3D is established prior art

[Paris et al. 2019](https://doi.org/10.1145/3342765) is a direct precedent for
the selective volumetric half of the sketch. It converts an input heightfield
to an implicit construction tree, places compactly supported 3D primitives
only where landforms require them, indexes those primitives with a bounding
hierarchy, and extracts the surface through a vertically pruned grid.
Unaffected columns recover elevation directly from the heightfield rather
than performing general implicit root finding.

The paper demonstrates cliffs, overhangs, arches, caves, karst networks,
hoodoos, canyons, and floating islands. In its examples, volumetric coverage
is sparse; its optimized polygonizer reduces field queries and reports up to
12x extraction speedup over exhaustive traversal. Construction-tree
generation still takes seconds over finite input terrains, and mesh extraction
also takes seconds. Feature placement uses whole-input analyses, sampling,
grammars, and erosion-like processes rather than canonical streamed owner
queries.

The lesson is nevertheless concrete: Mclone should not treat universal 3D
noise as the only route to volumetric terrain. A heightfield fast path,
compact-support formation facts, bounded vertical ranges, and direct
elevation extraction outside affected columns are established techniques.

### Adjacent, non-equivalent precedents

- [LayerProcGen](https://runevision.com/tech/layerprocgen/) demonstrates
  deterministic infinite layered generation, coarse provider layers,
  differently sized layer chunks, and statically bounded dependency padding.
  It is the closest software-architecture precedent for a streamed
  coarse-to-fine dependency DAG, but deliberately supplies no terrain,
  hydrology, or scale-consistency algorithm.
- [Geometry Clipmaps](https://hhoppe.com/geomclipmap.pdf) demonstrate
  fixed-complexity nested presentation grids, incremental residency, smooth
  transitions, and optional coarse-to-fine procedural synthesis. They do not
  supply geographic feature identity or canonical plan authority.
- [Wavelet Noise](https://www.cs.jhu.edu/~misha/ReadingSeminar/Papers/Cook05.pdf)
  supplies nearly band-limited multiresolution noise whose unrepresentable
  frequency bands can be omitted. It is useful for local residual detail and
  anti-aliasing, not for rivers, basins, containment, or feature identity.
- [Transvoxel](https://transvoxel.org/) stitches neighboring voxel meshes at
  different resolutions from local samples. It can solve a future adaptive
  density-mesh seam, not canonical terrain planning or distant summaries.
- [Veloren world generation](https://book.veloren.net/internals/worldgen/worldgen.html)
  separates a finite coarse geological and erosion stage from local filling
  and reshaping. It reinforces the utility of coarse facts but persists a
  complete finite world result.
- [drainage-constrained DEM generalization](https://doi.org/10.1016/j.cageo.2012.05.002)
  preserves the terrain skeleton and drainage structure better than
  unconstrained coarsening, while
  [hydrology-preservation metrics](https://doi.org/10.1145/1517463.1517470)
  show why height RMS and maximum error alone do not measure whether a
  reconstructed terrain retained the same drainage story. These are
  bottom-up analyses of complete data, not direct generators.

### Consequences for Mclone

The precedents change the research posture in four ways:

1. **Adopt and test, rather than defend broad novelty.** The interesting work
   is fitting known semantic-refinement and sparse-implicit ideas to Mclone's
   stronger random-access and topology contracts.
2. **Separate finite and unbounded authority.** A torus can, in principle,
   own a complete finite coarse graph like the planet systems. A plane and
   cylinder cannot require a complete domain; they need bounded canonical
   owners or another proven lazy representation.
3. **Keep canonical generation independent of presentation.** A view may
   decide which deterministic facts to realize or retain, but exact blocks
   and feature identities cannot depend on view refinement, mesh collapse, or
   residency.
4. **Make sparse 3D an explicit representation contract.** Ordinary columns
   should prove direct height extraction and zero formation queries. Selected
   columns should expose bounded vertical ranges and far summaries before
   density realization.

## Selective Volumetric Realization

A useful conceptual composition remains:

```text
density(x,y,z)
  = planned_surface(x,z) - y
  + regional_gate(x,z) * volumetric_modifier(x,y,z)
  + sum(bounded_formation_density(x,y,z))
```

The formula is not a locked API. It records the intended budget:

- the planned surface is cheap and available everywhere;
- ordinary regions can prove a near-zero volumetric skip path;
- a selected region evaluates density only in a bounded vertical envelope
  around the planned surface;
- bounded landmarks have finite support and expose conservative far facts;
- exact chunks interpolate a coarse density lattice;
- distant terrain may omit hidden undersides while preserving important
  silhouette, height range, coverage, and openings; and
- caves and other subsurface systems remain separately seeded and budgeted
  even if they reuse density-composition machinery.

Three-dimensional density supplies volumetric morphology, not geographic
explanation. The plan must still own coast, drainage, traversability, basin,
and scale relationships.

## Topology Obligations

Every level uses the dimension topology rather than introducing a global
planar exception:

- canonical plan and feature identities wrap through the topology context;
- parent/child ownership and boundary adjacency agree across periodic seams;
- summaries describe canonical facts while presentation selects an
  observer-relative lift;
- periodic levels use compatible cells or an explicitly proven alternative;
- a cylinder may route drainage across its periodic axis while remaining
  unbounded on the other;
- a torus requires explicit oceans, lakes, wetlands, or closed sinks because
  it has no external drainage edge; and
- features are not suppressed around a seam merely to avoid implementing
  periodic realization.

The maximum meaningful planning scale is profile- and topology-owned. An
unbounded plane still uses a fixed finite set of terrain-authoritative scales;
an infinite hierarchy whose every level influences exact local terrain would
make a point query unbounded.

## Remaining Research Questions

The first witness and reconstruction answer the narrow identity,
boundedness, and continuous-realization portions for two simple segment
families. They do not prove a complete torus base graph, arbitrary
boundary-crossing geography, useful hydrology, or production terrain. The
next review should answer:

1. Is the visible parent-to-child relationship clear and useful enough to
   justify reconstructing one influence family?
2. What must a parent store so a coarse query can preserve major river,
   island, coast, range, and opening identity without generating its children?
3. Does the implemented parent terrain remain useful when it is judged as a
   surface rather than colored line geometry?
4. What scale-consistency metrics are practical for coasts, range axes,
   basin/outlet topology, water coverage, skyline, and openings?
5. How should adjacent parent features cooperate so the result becomes
   geography rather than independent corridors without introducing unbounded
   graph discovery?
6. What are the ordinary, hotspot, exact-chunk, and fixed-output continental
   costs of one sparse implicit formation family?

## Evidence Standard

The research must distinguish adjacent but non-equivalent precedents:

- rendering LOD over an already complete heightfield;
- filtered or band-limited procedural detail;
- bottom-up mipmaps or roll-ups requiring fine children first;
- top-down procedural refinement whose parent exists independently;
- finite whole-world erosion or hydrology;
- streamed bounded feature ownership;
- adaptive volumetric meshing; and
- a complete multiscale geographic authority.

A source belongs in the durable ledger only when its primary paper,
maintained source, or official technical documentation was inspected.
Record whether concepts or code are reusable, the domain and finiteness
assumptions, what is actually stable across scale, and what the source does
not prove.

## Current Decisions

- Do not promote either reconstructed Revision 1 family; Human Review R1 did
  not find its schematic terrain compelling enough for another isolated
  revision or frozen-foundation trial.
- Do not require the continental/ecoregional direction to use parent/child
  terrain courses or to organize world generation around LOD.
- Preserve direct coarse queries, exact parent identity, containment,
  topology, native/Wasm equivalence, and cache-independent reconstruction as
  reusable mechanisms rather than a selected terrain design.
- Do not add universal 3D noise to production terrain.
- Do not call a sparse exact point sampler semantic LOD.
- Do not allow camera scale to affect exact world generation.
- Do not claim that a visually similar coarse image preserves topology.
- Do not require detailed child generation merely to answer a continental
  query.
- Do not claim novelty from the narrower gap: the initial survey is not an
  exhaustive prior-art search.
- Do not assume the finite-planet base-mesh strategy transfers unchanged to
  an unbounded plane or cylinder.
- Do not import a view-adaptive mesh as canonical world state.

## Research Ledger

| Source or experiment | Direct coarse generation | Semantic stability | Volumetric role | Domain assumption | Current conclusion |
|---|---|---|---|---|---|
| local Mclone and Java 1.17.1 baselines | mixed | partial | reference density only | streamed plane; selected periodic Mclone profiles | establishes the cost and representation question, not the solution |
| Derzapf et al. 2011 | yes, complete typed base mesh | explicit exploration-path reproducibility; river types refine | none; surface mesh | finite planet | strongest order-independent adaptive refinement precedent |
| Génevaux et al. 2013 | whole graph before continuous terrain | drainage, watersheds, and crests own the surface | none; heightfield | finite bounded domain | strong graph-before-terrain precedent, not streamed authority |
| Génevaux et al. 2015 | authored construction-tree levels | explicit LOD operators and compact-support queries | proposed as other work | finite/authored terrain | close semantic query representation without automatic placement |
| Paris et al. 2019 | input heightfield plus sparse generated formations | one construction tree; localized support | direct implicit 3D precedent | finite input terrain | validates heightfield fast path plus bounded 3D features |
| Cortial et al. 2020 | complete roughly 50 km typed planet mesh | seeded deterministic subdivision across scales | none; surface mesh | finite planet | closest direct hydrologic generative-LOD precedent |
| Geometry Clipmaps | coarse pyramid; optional procedural synthesis | geometric transition only | none | finite or procedural presentation | fixed-budget renderer precedent, not geography authority |
| Wavelet Noise | direct frequency-band evaluation | band limits, not semantic identity | usable inside density | stationary procedural field | scale-aware detail tool only |
| LayerProcGen | provider layers can be much coarser | deterministic chunks and bounded declared dependencies | framework-neutral | infinite chunked domain | closest streamed dependency architecture, no terrain proof |
| Transvoxel | no | local crack-free mesh transition | adaptive voxel meshing | bounded local neighborhoods | realization technique only |
| Veloren | complete coarse geology/erosion stage | downstream local generation consumes saved facts | local caves and reshaping | finite precomputed world | production-style coarse-to-local precedent |
| drainage-aware DEM generalization | no; simplifies complete fine data | preserves drainage better than filtering | none | finite source DEM | suggests topology metrics, not a generator |
| Tactical 272 Revision 1 witness | yes; parent and regional queries stop directly | exact parent identity, containment, continuity, and explicit sink propagation | none | bounded owners on plane/cylinder; finite canonical torus | mechanism succeeds on native/Wasm and remains inspectable in Planner atlas |
| Tactical 273 semantic terrain sandbox | yes; each requested course reconstructs directly and counts its work | exact identity plus bounded parent-to-regional and regional-to-local surface corrections | none; continuous heightfield only | pannable plane/cylinder/torus viewports | native/Wasm invariants and interactive cost pass; Human Review R1 found the terrain slightly disappointing and did not promote the shapes |

## Recommended Next Work

1. Preserve the Planner-atlas and Semantic-terrain panes as research evidence;
   do not spend another tactical tuning Revision 1 without a new concrete
   question.
2. Begin the continental/ecoregional campaign in Terrain Lab and World
   Explorer without requiring its plan to descend from this experiment.
3. Reuse the pinned exact-invariance and direct-query suites when a concrete
   continental, province, ecoregion, river, clearing, or formation plan gains
   parent/child facts.
4. Add coarse presentation summaries only after product geography exists and
   only as downstream projections of that geography.
5. Revisit selective 3D terrain separately when an accepted geological or
   landform family establishes a bounded volumetric need and an ordinary
   zero-query path.
