# Deterministic Streamed Landscape Planning

Topic: `deterministic-streamed-landscape-planning`

Status: **Active research direction as of 2026-07-27, paused at Tactical 270
Human Review R0. Tactical 267 proved that a bounded hybrid landform plan can
produce useful drainage, basin, divide, and quiet-space structure with
deterministic reconstruction over one fixed study domain. Tactical 268 made
that plan inspectable and exposed the unresolved production question: the
same absolute location must retain the same plan when reached through
different windows, chunk requests, workers, caches, paths, and topology
lifts. Tactical 270 Phase 0 found no precedent for exact unbounded on-demand
hydrology: exact tiled methods retain a complete finite global meta-problem.
It therefore proposes bounded generative hydrography with a fixed maximum
scale, finite dependency DAG, stable feature/facet ownership, and the
coordinate-pure fallback. No streamed planner, harness, adjacent-region
implementation, or production terrain consumer exists yet.**

## Purpose

This topic owns the methodology and evidence for deciding whether Mclone can
use a novel relational landscape planner without sacrificing deterministic
world generation or cheap broad sampling.

The desired planner would let terrain, rivers, basins, divides, coasts, and
later systems consume shared geographic facts. It must still behave like a
seeded world generator rather than a discovery-time simulation:

```text
same stored descriptor + seed + dimension + topology + absolute location
    -> same accepted plan facts and realized terrain
```

Player travel, viewport centers, chunk request order, Worker scheduling,
cache state, and prior exploration must not appear on the right side of that
function.

This is a research workstream, not a commitment to promote the Tactical 267
prototype. A well-supported rejection and return to coordinate-pure terrain
plus bounded deterministic features is a successful outcome.

## Relationship To Other Owners

- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
  owns the desired landscape vocabulary, cross-system precedence, scale
  hierarchy, and visual-quality questions.
- This topic owns whether a streamed relational plan can satisfy exact
  determinism, boundary continuity, topology, bounded execution, caching,
  reconstruction, and evidence standards.
- [`bounded-world-topology.md`](bounded-world-topology.md) owns canonical
  addresses, work and presentation lifts, periodic adjacency, and the rule
  that a topology seam is not a content boundary.
- [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md)
  owns the reference-locked Java 1.17.1 status and write-order model. This
  research may reuse its target-local start/reference pattern but does not
  change vanilla generation.
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md) owns live
  Mclone Overworld implementation truth. Production terrain remains field
  revision 21 until a later tactical explicitly changes it.
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) and the terrain-view
  topics own distant representation and presentation. They may consume
  accepted summaries later; they do not define planner semantics.

## Current Evidence And Its Limit

Tactical 267 constructed one complete 6,144-by-6,144-block plan at 32-block
cells. For the fixed three-seed plane/cylinder corpus it proved:

- deterministic rebuilds and stable checksums for an identical seed, domain,
  topology, and planner revision;
- complete acyclic receiver forests with protected sinks and valid spills;
- drainage hierarchy, divergent divides, basin ownership, and quiet space;
- indexed local reconstruction without upstream traversal at query time;
- exact periodic reconstruction at the cylinder seam; and
- measured cold construction, retained memory, warm point-query, and far
  summary costs.

Tactical 268 extracted a compact plane summary and builds it once per seed in
a Terrain Lab Worker. Pan, zoom, inspection, and overlay changes query that
retained summary without rebuilding it.

Those results establish a useful bounded representation. They do not prove:

- equality for one point when it is solved through two overlapping domains;
- compatible facts from independently constructed neighboring regions;
- request-, path-, cache-, or schedule-independent lazy construction;
- a bounded dependency rule for upstream contributing area on an unbounded
  plane or cylinder;
- native/Wasm cross-host checksum equivalence for a streamed representation;
- torus planning; or
- an acceptable production cache, batching, summary, persistence, or
  compatibility policy.

The fixed Terrain Lab boundary is therefore honest evidence, not an
implementation inconvenience to hide with automatic recentering.

## Determinism Vocabulary

The word “order” is overloaded. Research receipts must name the property they
test.

| Property | Required meaning |
|---|---|
| rebuild determinism | rebuilding the same canonical plan identity produces byte-identical semantic facts |
| request-order independence | requesting chunks or plan regions in any permutation produces the same final facts |
| path independence | raster, spiral, random-walk, reverse, or teleport exploration cannot change geography |
| schedule independence | native threads and browser Workers may complete in any order without changing output |
| cache independence | cold, warm, evicted, partially resident, and reconstructed caches affect time only |
| window independence | a viewport or diagnostic window is not an input to geographic identity |
| partition independence | changing chunk batching or asking through overlapping regions cannot change a point |
| lift independence | equivalent canonical locations reached through periodic lifts produce the same facts |
| revision determinism | a fixed planner revision defines stable tie-breaking and construction traversal |

The final row is intentionally different. A planner may define one canonical
internal traversal, stable sort, and tie-break rule. The algorithm need not be
mathematically invariant under arbitrary implementation rewrites. Its output
must be invariant under runtime request and execution order while its
revision remains fixed.

Randomness follows the same rule. Every choice derives from the stored seed,
typed domain, canonical owner, stable feature identity, and local choice
index. No mutable random stream may advance according to discovery order.

## Historical Warning: Alpha And Beta Population

Alpha v1.1.2_01 and Beta 1.7.3 illustrate both sides of the contract.

Their base terrain is target-local and coordinate seeded.
`OverworldChunkGenerator.getChunk` reseeds from chunk X/Z before density,
surface, and target-chunk caves. That terrain does not become different
because the player approached from another direction.

Their delayed population is the warning. Population begins after a 2-by-2
neighborhood becomes available, then lakes, trees, ores, and other features
write into the live world and may cross chunk boundaries. Historical
post-population state can therefore depend on which chunks were present and
which writer ran first.

Mclone's Alpha/Beta-inspired profiles deliberately preserve terrain and
feature vocabulary without reproducing those chunk-load-order side effects.
The streamed planner must follow the target-local/canonical-owner lesson, not
the delayed-population mechanism.

## Research Question

Can Mclone construct a world-indexed relational landscape that:

1. produces visibly stronger geographic composition than coordinate-pure
   scalar terrain;
2. is exactly independent of request, traversal, scheduling, cache, window,
   partition, and topology lift;
3. gives adjacent regions shared river, basin, divide, level, and feature
   identity rather than merely similar edge pixels;
4. has a finite, declared dependency radius or hierarchy at every query and
   construction level;
5. retains cheap local reconstruction and scale-aware far summaries; and
6. remains understandable enough to test, version, cache, and maintain?

An exact physical watershed over an unbounded plane may fail the fourth
requirement because contributing area can depend on arbitrarily distant
terrain. Research must not disguise that with a large but unjustified halo.
Candidates may instead define bounded procedural catchments, a finite
hierarchy, or explicit coarse-scale flow contracts.

## Candidate Families

The candidates are hypotheses, not steps in a predetermined implementation.

### A. Canonical supertiles with owned interiors

A stable world grid identifies a larger construction area. The planner solves
that supertile plus a finite halo, then publishes only canonically owned
interior cells and primitives.

This is attractive because it resembles ordinary target-local feature
generation and makes cache identity straightforward. A halo alone does not
solve arbitrarily long drainage or agreement at supertile boundaries.

### B. Hierarchical boundary facts

A coarser deterministic level supplies major outlets, river crossings,
high/low axes, or other boundary conditions. Finer regions elaborate between
those shared facts and publish stable interiors.

This can preserve long features across many regions while bounding each
solve. It risks exposing a planning lattice, overconstraining local form, or
creating a complex multilevel invalidation and identity model.

### C. Feature-owned graphs

Canonical coarse cells own stable river, basin, ridge, or corridor starts.
Each feature has a stable identity and bounded influence. A target query
enumerates possible owners in a finite neighborhood, reconstructs their
features, and applies typed deterministic composition.

This follows the successful start/reference/clipped-realization shape used by
structures and carvers. Automatic convincing placement, large connected
networks, and basin semantics may be harder than in a raster solve.

### D. Bounded hybrid atlas

A likely experiment combines the three mechanisms:

- coordinate-pure fields provide broad intent;
- a finite coarse hierarchy provides shared cross-region facts;
- canonical regional solves derive bounded detail; and
- compact feature-owned primitives reconstruct the accepted plan.

The hybrid is not the default winner merely because Tactical 267 used a
hybrid inside one fixed domain.

### Fallback: coordinate-pure grammar plus bounded starts

Retain field revision 21 or a later coordinate-pure scalar/density grammar.
Add rivers, lakes, formations, and other identities as independent bounded
starts with deterministic owner-neighborhood queries.

This may provide less globally derived hydrology, but its determinism,
streaming, topology, and performance contracts are much simpler. Every
candidate must be compared with this fallback rather than only with the
bounded Tactical 267 plan.

## Boundary Contract

“Smooth the seam” is insufficient because geometry and meaning have different
requirements.

Semantic continuity is exact:

- a crossing river has one stable identity, direction, order, level, and
  upstream/downstream relationship on both sides;
- a basin fragment agrees about basin identity, outlet or sink, and spill
  semantics;
- divide and ridge continuations agree about ownership and direction;
- each feature, claim, and plan cell has one canonical owner; and
- neighboring plans do not both publish or both omit the same owned fact.

Geometric continuity is measured:

- equivalent boundary samples reconstruct identical height and water facts;
- adjacent slopes and normals do not show an abnormal boundary impulse;
- curvature and feature-density distributions near implementation boundaries
  remain comparable with matched interior transects;
- compact-support influences query all possible canonical owners before
  composition; and
- visual review finds no square plan cells, clipped valleys, feature deserts,
  duplicated ridges, boundary collars, or unexplained straight runs.

Scalar residuals may blend. Basin IDs, graph connectivity, source/sink
relationships, and water levels may not.

## Topology Contract

Every experiment receives an immutable dimension context containing the
stored profile, seed, planner revision, and topology. There is no global
current topology.

- Canonical plan cells divide a periodic extent where practical.
- Adjacency, halos, owner neighborhoods, and region corners wrap and
  deduplicate.
- Routing uses shortest topology displacement and a stable half-period tie.
- A cylinder seam is an ordinary crossing, not an outlet or feature
  exclusion.
- A torus has no external drainage edge; accepted water ends in explicit
  ocean, lake, wetland, closed basin, or another typed sink.
- Plan identity uses canonical addresses; reconstruction uses a coherent
  target-relative work lift.
- Plane, cylinder, and torus experiments use one semantic planner interface.
  Unsupported candidates fail explicitly.

## Research Method

### Source discipline

Every material external influence must enter the reference ledger with:

- a stable primary-source link or local source path;
- the exact algorithm, representation, invariant, or warning it contributes;
- what it does not establish for Mclone;
- whether code, pseudocode, parameters, or only a concept was inspected; and
- any licensing or clean-room constraint relevant to implementation.

Secondary summaries may help discover sources but do not establish a
mechanism. Do not attribute a persistent drainage graph to Minecraft unless
its source actually contains one.

### Falsification before visual promotion

An experiment first attempts to break determinism, boundedness, semantic
continuity, and topology. A visually attractive candidate that fails an exact
invariant does not advance to subjective review.

The harness must generate identical target facts through:

- cold rebuild and repeated rebuild;
- forward, reverse, randomized, and parallel region requests;
- raster, spiral, random-walk, teleport, and two-front exploration paths;
- empty, warm, partially populated, and repeatedly evicted caches;
- independent neighboring-region construction in both orders;
- overlapping diagnostic windows and different chunk partitions;
- region edges and four-region corners; and
- canonical and lifted queries across cylinder and torus seams.

Tests compare schema-versioned semantic arrays and stable checksums. Aggregate
metrics alone cannot prove equality.

### Comparable evidence

All candidates use the same seeds, target coordinates, topology cases,
request permutations, reconstruction samples, and timing boundaries where
their representation permits. Each result reports:

- canonical plan identity and every output-affecting descriptor;
- commit and schema revision;
- exact command and host/toolchain facts;
- construction extent, owned extent, cell size, halo, and hierarchy levels;
- dependency fanout and maximum search/traversal bounds;
- cold construction, warm lookup, batch query, and far-summary time;
- resident, transient, transferred, and persisted bytes;
- cache hit/miss/eviction state;
- graph, basin, sink, and boundary-conservation facts;
- full semantic checksums for every order/path case; and
- geometric seam metrics normalized against interior controls.

Generated images and raw receipts stay under `/tmp` unless a small fixture is
required by a test. Durable docs retain commands, schema, summarized tables,
inspected artifact names, decisions, and limitations.

### Performance posture

No plan query may perform an arbitrary upstream walk, flood fill, ocean
search, or world scan. Construction cost must be bounded by canonical plan
identity rather than exploration history.

Measure at least:

1. cold plan construction;
2. warm point lookup;
3. warm chunk/region batch lookup;
4. cache miss, hit, eviction, and reconstruction;
5. exact terrain realization consuming the plan;
6. a coarse summary used without exact reconstruction;
7. representative movement/streaming; and
8. plane, cylinder, and torus seam cases.

The existing Tactical 258 production sampler and Tactical 267 bounded-plan
receipts are controls, not automatically accepted budgets. Before production
promotion, a separate tactical must select explicit latency, memory, and
ordinary-path overhead budgets.

## Promotion And Rejection Gates

A candidate cannot advance beyond research if any of these remain true:

- one point changes under request, path, cache, partition, window, schedule,
  or lift permutations;
- neighboring regions disagree semantically and only a visual blend hides it;
- correctness depends on exploration-time mutable state;
- a query or cache miss has an undeclared or unbounded traversal;
- periodic seams require a featureless exclusion belt;
- far representation silently loses accepted rivers, basins, islands, or
  silhouettes;
- the architecture cannot state stable plan identity and invalidation keys;
  or
- the improvement over the fallback is not visible in comparable regional and
  walking-scale evidence.

Passing those gates authorizes only a production-integration proposal. It
does not change terrain output.

The final research decision must choose one:

1. **Promote:** one representation passes exact, topology, performance, and
   human-quality gates; write a separate production tactical.
2. **Narrow:** retain only a smaller bounded use, such as planned lakes or
   major river starts, while ordinary terrain remains coordinate-pure.
3. **Continue research:** one blocker has a bounded, evidence-backed next
   experiment.
4. **Reject:** archive the planner as a diagnostic result and adopt the
   coordinate-pure plus bounded-start fallback.

## Human Review Policy

Human review occurs only after the evidence appropriate to that question is
ready.

- **Research Review:** inspect the primary-source ledger, candidate
  specifications, invariants, corpus, and fallback before substantial code.
- **Structural Review:** after exact invariant tests pass, freely pan an
  interactive plan atlas across ordinary boundaries and topology seams;
  compare topology, hierarchy, negative space, repetition, and boundary
  legibility against the fallback.
- **Terrain Review:** inspect same-seed reconstructed terrain at macro,
  oblique, journey, and walking scales, with and without subordinate detail.
  Include ordinary regions, boundaries, corners, and periodic seams.
- **Decision Review:** examine exact failures, quality findings, cost tables,
  complexity, and maintainability together; select promote, narrow, continue,
  or reject.

No screenshot can waive an exact determinism failure. No metric can overrule a
clear geographic or walking-scale defect.

## Reference Ledger

Phase 0 inspected papers, published framework documentation, selected
implementation source, and real generator source. Its main result is a useful
negative one: **tiling an exact drainage analysis does not make each tile an
independently finalizable streamed plan.**

### Exact Hydrology Retains A Global Meta-Problem

Barnes's parallel Priority-Flood divides a finite DEM into tiles. Consumers
solve local watersheds and return edge labels plus a spillover graph. A
producer makes labels globally unique, joins adjacent edge and corner facts,
solves the global spillover graph, and returns corrections so consumers can
finalize their tiles. The parallel flow-accumulation algorithm has the same
shape: local accumulation and perimeter links are not final until a producer
connects every tile's perimeter graph and returns global offsets.

This is excellent evidence that exact finite hydrology can compress global
coupling. It is also evidence against pretending a large halo solves the
streaming problem:

- the algorithms know the complete finite DEM and its real external edges;
- all tile summaries participate in one global meta-problem;
- no arbitrary tile is final before that global solve; and
- an unbounded plane or cylinder does not provide a last tile at which the
  meta-problem becomes complete.

Depression hierarchies and Fill-Spill-Merge make nested sinks, spill saddles,
and lake connectivity explicit and efficient after a finite DEM is known.
They improve the representation vocabulary but do not remove the complete
domain prerequisite.

Mclone must therefore distinguish two goals:

1. **analytical hydrology**, which derives the exact drainage of a complete
   provisional surface; and
2. **generative hydrography**, which creates a bounded, revisioned river,
   basin, sink, and level grammar that terrain then realizes.

The first is admissible for a finite whole-domain profile or offline authored
atlas. The streamed plane/cylinder research must pursue the second unless a
new source or proof overturns this conclusion.

### Contextual Streaming Requires A Finite Effect Distance

LayerProcGen is the strongest implementation precedent found for deterministic
contextual generation in an on-demand infinite plane. Its contract separates
each layer's inputs from outputs, declares provider dependencies before a user
chunk runs, and forms a directed acyclic dependency graph. A viewport or
player position creates a top-level residency request; it does not become
geographic input.

Its integrity rule is equally important: padding must cover the maximum
distance at which lower-layer input can affect accepted output. Multiple
iterations add their effect distances. If an operation such as pathfinding
has no useful natural bound, the implementation must impose one, such as a
fixed corridor around the endpoints. Insufficient padding may remain
repeatable while ceasing to be seamless.

The accompanying terrain sample also demonstrates two different spatial query
contracts:

- **owned within bounds** selects one stable anchor owner for an operation
  that must happen once; and
- **overlapping bounds** returns every bounded feature whose influence a
  consumer must reconstruct.

These mechanisms strongly support Mclone's candidate identity and dependency
rules. LayerProcGen explicitly brings its own algorithms, however, and neither
its documentation nor inspected sample proves exact watershed or unbounded
network connectivity.

### Generator Precedents Choose Different Bounds

The inspected generators occupy three useful points in the design space:

- Java 1.17.1 generates structure and carver starts from canonical chunks,
  then scans a fixed possible-owner neighborhood and clips their realization
  to the target. This is the strongest fallback precedent.
- The Cluster uses finite spatial dependencies, larger planning layers, stable
  ownership, and bounded paths in an on-demand world. This is the strongest
  relational streaming precedent, but it plans game regions and paths rather
  than hydrology.
- Veloren stores a finite `WorldSim` over a power-of-two map, runs erosion and
  drainage over its complete arrays, derives water/flux/rivers, and then
  generates detailed chunks. This is the strongest open game precedent for
  relational terrain and water, but whole-world precomputation is precisely
  the boundary that Mclone's plane/cylinder experiment cannot assume.

No inspected source demonstrates exact derived hydrology over an unbounded
world with arbitrary random-access finalization. The novel question is
therefore narrower and more honest: can a finite-scale generative hydrography
produce enough of the same geographic relationships to justify its cost?

### Detailed Source Ledger

| Source | Inspection depth and reuse posture | Contribution | Limit for Mclone |
|---|---|---|---|
| [Alpha v1.1.2_01 `OverworldChunkGenerator`](../../reference/minecraft-a1.1.2_01/src/net/minecraft/world/gen/chunk/OverworldChunkGenerator.java) and [`ServerChunkCache`](../../reference/minecraft-a1.1.2_01/src/net/minecraft/server/world/chunk/ServerChunkCache.java) | local implementation source inspected; reference behavior only | coordinate-seeded target terrain contrasted with delayed cross-chunk population | delayed live-world writes are an order-dependence warning, not a relational planner |
| [Beta 1.7.3 `OverworldChunkGenerator`](../../reference/minecraft-b1.7.3/src/net/minecraft/world/gen/chunk/OverworldChunkGenerator.java) and [`ServerChunkCache`](../../reference/minecraft-b1.7.3/src/net/minecraft/server/world/chunk/ServerChunkCache.java) | local implementation source inspected; reference behavior only | repeats the terrain/population distinction with climate and lake-era content | does not make historical population order-independent |
| [Java 1.17.1 `ChunkGenerator`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`StructureFeature`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), and [`StructureStart`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructureStart.java) | local implementation source inspected under the existing reference-porting policy | canonical coordinate-seeded starts, fixed 17-by-17 carver and structure-reference searches, stable start identities, bounding boxes, and target-clipped placement | bounded features, not an exact drainage graph or evidence that every later ordered block write commutes |
| [Barnes, Lehman, and Mulla 2014, *Priority-Flood*](https://arxiv.org/abs/1511.04463) | paper algorithm and pseudocode inspected; concepts only, no code copied | efficient finite-domain depression filling, watershed labels, and flow directions by flooding inward from real DEM edges | forces drainage to finite edges and does not preserve intentional lakes or solve unbounded accumulation |
| [Barnes 2016, *Parallel Priority-Flood*](https://arxiv.org/abs/1606.06204) | complete paper algorithm inspected; concepts only, no code copied | exact tiled local solves, edge/corner reconciliation, compressed global spillover graph, then tile finalization | requires summaries from the complete finite DEM and one global graph solve; a tile is not independently final |
| [Barnes 2017, *Parallel Non-divergent Flow Accumulation*](https://arxiv.org/abs/1608.04431) | complete paper algorithm inspected; concepts only, no code copied | local tile accumulation plus perimeter flow/link summaries, a global inter-tile graph and offsets, and fixed communication phases | assumes known flow directions and a complete finite tile set; exact upstream area remains globally coupled |
| [Barnes, Callaghan, and Wickert 2020, *Depression Hierarchies*](https://doi.org/10.5194/esurf-8-431-2020) and [2021, *Fill-Spill-Merge*](https://doi.org/10.5194/esurf-9-105-2021) | open CC BY 4.0 papers and algorithms inspected; representation concepts only | binary depression forests, nested sinks, ocean links, spill saddles, and efficient fill/spill/merge routing without deleting lakes | builds topology from a complete finite raster and does not provide independent streamed construction |
| [LayerProcGen 0.4.0](https://runevision.github.io/LayerProcGen/) documentation and [repository](https://github.com/runevision/LayerProcGen) at `c13d64e53f0068ea9b24d761996406d4228adbaf` | architecture, effect-distance, ownership, internal-level docs and selected dependency/sample code inspected; MPL-2.0; clean-room concepts only | deterministic contextual chunks through immutable lower-layer inputs, declared finite padding, a dependency DAG, stable ownership anchors, overlap queries, multiscale planning, and cache/residency separation; used by released game The Cluster | framework supplies no planner algorithm; pathfinding must be artificially bounded; padding mistakes can preserve determinism while breaking integrity; no periodic topology or hydrology proof |
| Teinemaa, Riemer, and Shaker 2015, [*A Procedural Approach for Infinite Deterministic 2D Grid-Based World Generation*](https://pcgworkshop.com/archive/teinemaa2015procedurl.pdf) | full paper, Algorithm 1, and border method inspected; linked source license not established, so no code reuse | seeded layered chunks, adjacent candidate generation, bounded center/border/corner agents, and an explicit attempt to test different approach paths | proof-of-concept validation compared screenshots from starting locations; paper describes already-created partial chunks and a prescribed smoothing order, so cache, schedule, and exhaustive order independence are not established |
| [Veloren `WorldSim` source](https://docs.veloren.net/src/veloren_world/sim/mod.rs.html) and [worldgen notes](https://book.veloren.net/internals/worldgen/worldgen.html) | generated current Rust source and project docs inspected; GPL-3.0-or-later; clean-room concepts only | finite power-of-two whole-map arrays, erosion, depression/water processing, flux and rivers, saved coarse world maps, then parallel detailed `SimChunk` generation | obtains relational hydrology by completing a finite world simulation first; map edges and out-of-bounds ocean are real boundaries, not periodic seams |
| [Génevaux et al. 2013, *Terrain Generation Using Procedural Models Based on Hydrology*](https://cs.purdue.edu/homes/bbenes/papers/Genevaux13ToG.pdf) | paper pipeline and representations inspected; concepts only | hierarchical river graphs, watershed and crest construction, and terrain reconstruction from hydrologic primitives | not an unbounded tiled generator, topology contract, or production performance proof |
| [Génevaux et al. 2015, *Terrain Modelling from Feature Primitives*](https://www.cs.purdue.edu/cgvlab/www/publications/Genevaux15CGF/) | paper representation and query structure inspected; concepts only | compactly supported skeletal features, construction hierarchy, bounding volumes, and pruned point queries | does not automatically create correct hydrology, ownership, or streamed cross-region identity |
| [Fischer, Boeckers, and Zachmann 2022, *Procedural Generation of Landscapes with Water Bodies Using Artificial Drainage Basins*](https://cgvr.cs.uni-bremen.de/papers/cgi22/CGI22.pdf) | full paper pipeline inspected; concepts only | rivers-and-lakes-first construction, basin-aware water levels, and a priority-grown surface | finite generated domain with authored constraints, not an infinite deterministic planner or periodic contract |
| [TauDEM documentation](https://hydrology.usu.edu/taudem/taudem5/help53/TauDEMToolboxOverview.htm) | public tool documentation inspected; vocabulary only | separates pit handling, flow direction, contributing area, channels, order, and watersheds | describes terrain-analysis products, not procedural streaming |
| [USGS watershed and drainage-basin overview](https://www.usgs.gov/water-science-school/science/watersheds-and-drainage-basins) and [stream order](https://www.usgs.gov/media/images/streamorder) | public scientific vocabulary inspected | common outlets, divides, nested drainage, and stream hierarchy | does not specify game dimensions, algorithms, ownership, topology, or tuning |

## Decision And Experiment Ledger

| Date | Evidence | Decision |
|---|---|---|
| 2026-07-27 | Tactical 265 source and mechanism comparison | Select a bounded hybrid only for a falsifiable prototype |
| 2026-07-27 | Tactical 267 fixed-domain maps, reconstruction, topology, and cost receipts | Bounded relational structure merits human review; production remains unchanged |
| 2026-07-27 | Tactical 268 interactive fixed-domain diagnostic | Preserve the boundary honestly; do not recenter and imply geographic stability |
| 2026-07-27 | Post-review determinism discussion | Open a separate streamed-planner feasibility workstream before any production integration |
| 2026-07-27 | Tactical 270 Phase 0 exact-hydrology and generator source review | Do not derive an allegedly exact unbounded watershed from independently solved tiles; test a bounded generative hydrography with a declared maximum scale |

Add future experiment IDs, commits, commands, corpus locations, results, and
decisions here or in the active tactical before relying on them.

## Open Questions

- Which candidate makes boundary agreement constructive rather than a
  post-process?
- Can meaningful contributing-area and stream-order facts be bounded without
  pretending to model an infinite physical watershed?
- Which facts must come from a coarser hierarchy, and which can remain
  region-local or feature-owned?
- What canonical plan extent and cell size balance quality, cost, and the
  6,144-block periodic Mclone cylinder?
- Can the plane, cylinder, and torus share one planner representation without
  hiding topology-specific sink semantics?
- What is the smallest far summary that preserves accepted topology?
- Is exact native/Wasm agreement practical with the selected numeric and
  sorting rules?
- Does relational quality remain compelling once boundaries, boundedness, and
  speed constraints are enforced?
- Is the resulting architecture easier to understand and evolve than a richer
  coordinate-pure grammar plus bounded starts?

## Related

- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`bounded-world-topology.md`](bounded-world-topology.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md)
- [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md)
- [`../tactical/258-mclone-macro-terrain-performance-baseline.md`](../tactical/258-mclone-macro-terrain-performance-baseline.md)
- [`../tactical/265-macro-landform-grammar-research.md`](../tactical/265-macro-landform-grammar-research.md)
- [`../tactical/267-hybrid-macro-landform-plan-prototype.md`](../tactical/267-hybrid-macro-landform-plan-prototype.md)
- [`../tactical/268-terrain-lab-landform-plan-diagnostic.md`](../tactical/268-terrain-lab-landform-plan-diagnostic.md)
- [`../tactical/270-deterministic-streamed-landscape-planner-research.md`](../tactical/270-deterministic-streamed-landscape-planner-research.md)
