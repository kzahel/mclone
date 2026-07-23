# Tactical 222: Bounded Valley Stream Structures

Status: complete 2026-07-23. The reusable procedural-start kernel, bounded
stream plan, clipped field-revision-12 realization, hydraulic proof,
profile-correct far LOD, SQLite reopen proof, generation comparison, movement
soak, and final production captures are implemented. Objective validation is
complete, and Human Review 1 accepted the result.

Topics: `mclone-overworld-generation`, `procedural-structure-starts`

Workstream: shared native Rust structure metadata and clipped terrain
modification, Mclone Overworld hydrology, production review tooling, synthetic
far LOD, persistence/reopen evidence, and bounded performance.

## Objective

Replace field revision 11's short raised tributary and artificial containment
shelf with the first bounded, multi-chunk, valley-following stream:

- an explicit headwater and an existing Y63 major river are its source and
  sink;
- the complete finite plan is monotonically non-increasing downstream;
- roughly 48-96 blocks of water occupy several calm flat reaches;
- adjacent reaches descend through one- or two-block riffles, cascades, or a
  locally justified larger fall;
- the channel and shallow valley are carved into suitable existing terrain;
- shoulders blend back into the untouched terrain without a raised retaining
  wall; and
- the generated liquid stencil is already a fixed point of the authoritative
  fluid rules.

The stream is also the first concrete consumer of a reusable,
vanilla-shaped procedural structure lifecycle:

```text
potential start chunk
  -> cheap profile-owned site predicate
  -> bounded procedural plan and pieces
  -> start bounding box
  -> references from intersected target chunks
  -> clipped per-chunk terrain intent and block realization
```

This is a bounded hydrology landmark, not a global drainage graph. It does not
claim arbitrary headwater networks, accumulated discharge, confluences,
erosion simulation, navigable flow physics, or a complete port of every
Minecraft structure status and family.

## Motivation And Rejected Shape

Revision 11 proved an important mechanical result: a Y67 source pool, short
upper reach, four-block fall, and Y63 receiver can be baked without generation
ticks and can remain unchanged after every water cell is scheduled through the
authoritative fluid simulation.

Interactive review rejected its terrain language. The roughly 31-40-block
reach is only about five blocks wide, its falling section is about two columns
thick, and its seven-block containment band raises low edge terrain above the
water plane. At the reviewed confluence, the major-river valley is lowered
toward Y63 while the tributary support band is forced above Y67. The result
reads as a grass-topped retaining wall or cul-de-sac with a tiny outlet, not as
a stream using an existing valley.

The correction is not a wider containment band. Site selection and routing
must make containment a natural-terrain fact:

- prefer excavation over fill;
- reject a route whose untouched banks do not already rise above its water;
- follow low terrain rather than cutting across ridges;
- shape a small valley and confluence notch rather than raising a plateau; and
- solve the entire finite level sequence before placing any reach.

## Minecraft Java 1.17.1 Reference Read

The local 1.17.1 reference sources and the durable
[`structures`](../structures.md) architecture were re-read on 2026-07-23:

- `StructureFeature.generate(...)` chooses a potential start chunk through
  spacing/separation placement, applies the feature predicate, creates one
  seeded `StructureStart`, and asks it to generate piece metadata;
- `StructureStart` owns pieces, one cached aggregate bounding box, start-chunk
  identity, deterministic random state, and reference count;
- `ChunkGenerator.createReferences(...)` scans structure starts through the
  fixed `[-8,+8]` reference square and records a reference only when a valid
  start bounding box intersects the target chunk;
- `StructureStart.placeInChunk(...)` invokes only pieces intersecting the
  target chunk's clipped placement box;
- noise-affecting villages, outposts, strongholds, and Nether fossils make
  structure metadata available before terrain density through `Beardifier`;
- actual structure pieces are placed during `FEATURES`, before ordinary
  configured features in the same decoration step;
- `LakeFeature` is not a true structure, but demonstrates bounded cavity
  construction followed by complete solid/liquid boundary validation; and
- `SpringFeature` is also an ordinary feature and schedules a local fluid tick
  only after its exact solid/hole neighborhood predicate succeeds.

The vanilla structure reference radius is eight chunks in each direction. It
is not an 8-by-8 terrain dependency and it does not grant ordinary features
permission to read or write fully generated blocks across that square.
Metadata-only outer dependencies are the precedent to preserve.

Vanilla rivers themselves are biome/terrain language rather than structure
starts. This tactical deliberately combines the structure lifecycle with a
profile-owned procedural terrain modifier. The mechanism is reusable;
Mclone owns the stream placement, route scoring, hydraulic profile, and
terrain cross-section.

## Current Native Boundary

The live native generator has no true persisted structure start/reference
runtime. Its existing shared feature plan uses:

- one chunk of backend-center write expansion;
- one further chunk of `Surface` block prerequisites;
- a mutable 5-by-5 surface region for one target; and
- deterministic replay of the surrounding 3-by-3 decoration centers.

That contract is correct for ordinary vegetation and other short-range
features. Expanding its block dependency radius to eight would incorrectly
materialize a 17-by-17 terrain region and make ordinary feature work far more
expensive.

This tactical therefore introduces a separate metadata-first procedural-start
kernel. It must be shaped so the later full native
`STRUCTURE_STARTS`/`STRUCTURE_REFERENCES` statuses and authored building
pieces can adopt it. It must not disguise far writes as ordinary decoration.

The first stream revision may deterministically reconstruct immutable accepted
plans from seed, profile revision, topology, and start chunk. Before closeout,
the chosen plan identity and all output-affecting facts must be either stored
with the generated chunk record or proven safe to reconstruct under the
current internal-mutable compatibility ledger. A future release freeze
requires persisted start/reference metadata; final block persistence alone is
not a substitute for queryable structures.

## Generic Procedural-Start Contract

The shared mechanism owns vocabulary and invariants, not Mclone rules:

- `StructurePlacement`: positive `spacing`, smaller `separation`, stable
  `salt`, and maximum reference radius no greater than eight;
- `StructureStartKey`: structure kind plus canonical start chunk;
- `StructureBoundingBox`: inclusive bounded block coordinates with chunk
  intersection and union operations;
- `StructurePiece`: stable piece kind/id and its bounding box;
- `StructureStart`: start key, aggregate box, ordered pieces, and a typed or
  opaque profile-owned payload;
- `StructureReference`: the target chunk and referenced start key; and
- clipped queries returning only pieces that intersect one target chunk.

Candidate selection, start planning, reference discovery, and clipped piece
queries must be pure and independent of request order, batching, cache
residency, thread count, and unrelated random draws. Signed coordinates and
the exact 384-chunk X cylinder need direct fixtures.

The initial kernel does not add jigsaw pools, NBT templates, processors, loot,
block entities, locate commands, mob overrides, or vanilla structure-family
registries. Structure Lab's canonical authored templates remain a later piece
payload consumer; this tactical does not move their source-of-truth boundary.

## Stream Start And Route Contract

Potential stream starts use a vanilla-shaped random-spread placement. A cheap
candidate first checks:

- proximity to a physically realized Y63 major river;
- non-ocean, non-wetland land on the selected side;
- a plausible untouched elevation range;
- enough coarse uphill relief to support at least two water levels; and
- no topology or reference-radius violation.

Only surviving candidates run the route planner. The planner samples the pure
untouched Mclone terrain field; it does not generate or inspect neighboring
chunk buffers.

The bounded first planner uses:

- a maximum 96-block route from confluence to headwater;
- a minimum 48-block accepted route;
- a fixed coarse search lattice, refined only for the chosen path;
- a hard cap on expanded nodes and candidate alternatives;
- a cost that rewards existing valleys, gradual uphill travel away from the
  sink, and low excavation;
- large penalties for ridge crossing, side-bank undercut, existing water,
  excessive curvature, or backtracking toward the sink; and
- deterministic tie-breaking from coordinates before any implementation
  collection order.

The accepted route is oriented from headwater to river. Its hydraulic profile
is derived backward from the known Y63 sink. Every flat reach has an integer
water Y; downstream levels are equal or lower; transitions drop one or two
blocks unless a locally selected fall explicitly permits more. Minimum reach
length prevents a staircase of one-block pools.

## Terrain Cross-Section

Every route sample exposes enough continuous facts for a chunk or LOD column
to derive the same nearest stream segment, downstream station, water level,
bed, and valley envelope.

The first cross-section should:

- place an approximately three- to five-block water width;
- keep the bed one or two blocks below a calm reach and deepen headwater and
  receiving pools selectively;
- cut a shallow U/V-shaped inner channel;
- blend shoulders over roughly 10-20 blocks depending on local slope;
- never raise an outer shoulder above untouched terrain;
- allow at most one block of localized natural-looking support fill, if any;
- reject the complete start when bounded bank probes expose an uncontained
  source plane; and
- widen and round the confluence rather than carrying the upper valley wall
  parallel to the major river.

Surface materials and vegetation consume the modified landform after stream
terrain intent. Decorations must not be placed and then removed by the
stream.

## Hydraulic Contract

The implementation does not pre-run fluid simulation during generation.
Instead, each accepted plan must satisfy both:

1. a generated-block closure audit proving support and horizontal containment,
   allowing unequal water tops only at explicit transition pieces; and
2. an authoritative runtime wake that schedules every source and flowing
   water block in the landmark, drains the queue, and observes zero unintended
   block mutation.

If a desired multi-level shape is not a fixed point of the current fluid
rules, change or reject the stencil. Do not suppress ticks, special-case
runtime physics by structure identity, or retain floating sources merely
because the untouched generated world initially looks correct.

## LOD Contract

This stream is large terrain language, not an ignorable tree-sized feature.
The immutable accepted plan and column query must therefore be usable by:

- authoritative full chunk generation;
- production review maps and receipts; and
- synthetic far-LOD surface sampling.

LOD may simplify the water surface and shoulder detail at coarser spacing, but
it must retain the route, reach levels, valley incision, and confluence. LOD
never becomes authoritative block or collision data.

## Performance Budget

The lifecycle makes cost bounded but does not make route search free. Record:

- candidate start checks and accepted starts per reviewed area;
- planner calls, expanded nodes, route points, pieces, and cache hits;
- per-chunk referenced-start and intersecting-piece counts;
- field-map and plan-overlay time;
- surface-only, cold decorated, and warm decorated targets per second;
- an accepted-stream-centered 3,600-frame movement soak; and
- LOD build/sample deltas where the same plan is consumed.

Hard gates:

- maximum reference radius eight chunks;
- maximum route length 96 blocks;
- maximum cross-section influence strictly inside the start bounding box;
- no unbounded search, recursive upstream trace, generated-chunk flood fill,
  or request-order-dependent cache discovery;
- more than 25 percent same-host cold-generation regression requires focused
  profiling and explanation;
- a twofold regression blocks visual acceptance; and
- cache misses may cost more than hits, but warm movement may not repeatedly
  re-plan the same accepted start.

## Slice Plan

### Slice 0: document, baseline, and rejected-shape lock

- [x] Record the accepted direction, reference precedent, dependency boundary,
  objective gates, and explicit exclusions.
- [x] Pin revision 11's reviewed start, hydraulic receipt, generation
  performance, and screenshot as the replacement baseline.

Gate: no source change until the reference read and bounded contract are
reviewable in one tactical.

Baseline record:

- revision 11 is commit `2ea92637`, reviewed at seed `-98765`, chunk
  `(183,-177)`, RD16;
- the nine-chunk hydraulic receipt counted 1,409 source blocks, 16 flowing
  blocks, four intentional drop edges, zero closure failures, and zero
  scheduled ticks after generation;
- waking every water cell through the authoritative server caused zero block
  mutations;
- same-host revision-10/revision-11 surface, cold, and warm throughput changed
  by `-7.0%`, `-1.5%`, and `-22.5%` respectively;
- the 3,600-frame RD10 movement run averaged 5.671 ms with p95 9.398 ms,
  p99 11.679 ms, 16.953 ms maximum, and zero fluid counters; and
- the rejected pixel is the 2026-07-23 interactive screenshot showing the
  grass-topped containment shelf and tiny outlet. Disposable final-card
  evidence was captured under `/tmp/mclone-rev11-final-card`.

### Slice 1: reusable procedural-start kernel

- [x] Add placement, start key, bounding box, piece, start, reference, and
  clipped-query types in shared worldgen ownership.
- [x] Prove random-spread placement across signed coordinates.
- [x] Prove aggregate boxes, radius-eight discovery, target references,
  clipped pieces, duplicate suppression, and reversed/partitioned request
  equivalence.
- [x] Prove periodic canonical starts and coherent lifted intersection across
  the 384-chunk seam.

Gate: a synthetic multi-chunk procedural start yields identical target
references and clipped pieces under every request partition and order.

Execution record 2026-07-23:

- `mclone-worldgen::procedural_structure` owns the first shared kernel without
  enlarging `FeatureRegion` or `ChunkGenerationPlan` block prerequisites;
- `StructurePlacement` directly translates Java 1.17.1's linear
  spacing/separation/salt selection through the existing `WorldgenRandom`,
  rejects a radius above eight, and requires periodic spacing to divide the
  cylinder circumference;
- periodic candidates carry separate canonical and coherently lifted work
  chunks, so persistent identity need not depend on which side of the seam
  requested the start;
- starts sort and validate stable piece ordinals, aggregate inclusive 3D
  boxes, emit references only inside the configured radius, and clip pieces to
  one target chunk; and
- seven focused kernel tests plus the nine existing generation-planning tests
  pass. The synthetic three-piece start produces identical references for
  ordered, reversed, and partitioned target requests.

### Slice 2: plan-only Mclone stream

- [x] Add cheap Y63 major-river candidate predicates.
- [x] Run one fixed-budget valley route search only for accepted candidates.
- [x] Derive monotonic reach levels and typed stream pieces.
- [x] Add plan caches keyed by every output fact.
- [x] Add production maps for candidates, route, reach Y, cut depth, required
  fill, rejected reason, start box, and reference footprint.

Gate: at least three seeds contain inspectable 48-96-block plans, every route
is monotonically non-increasing downstream, and no plan requires more than the
allowed fill or reference radius. No generated blocks change yet.

Execution record 2026-07-23:

- `McloneOverworldStreamPlanner` uses an eight-chunk spacing, three-chunk
  separation, stable salt, and the vanilla maximum reference radius of eight;
- only candidate chunks intersected by the existing Y63 major-river contour
  enter a deterministic two-sided beam search over a four-block lattice;
- the planner caps each search at 4,096 expanded alternatives, rewards
  valley-side relief, penalizes ridges, abrupt grade, high cuts, curvature,
  and movement back toward the river, then trims against measured route
  length rather than lattice-step count;
- accepted plans contain integer headwater-to-sink nodes, two to four
  downstream drops in the reviewed corpus, typed headwater/reach/transition/
  confluence pieces, an aggregate box, and a reusable continuous column
  query;
- the hydraulic profile is solved backward from Y63 using future minimum bank
  capacity. Accepted nodes descend or remain flat, require zero fill in the
  three-seed review, cut at most eight blocks, and reject any source plane
  lacking untouched side-bank clearance;
- `McloneOverworldStreamPlanCache` scopes positive and negative entries to one
  seed, topology, field code revision, and complete canonical/work start
  candidate. Cache tests prove both accepted and rejected hits;
- production review now emits `stream-plans`, `stream-plan-costs`, and
  `stream-candidates` maps plus candidate reasons, route nodes, reach
  metrics, boxes, and reference footprints in the JSON receipt; and
- no terrain sample, generated block, field revision, biome, surface, fluid,
  or LOD output changes in this slice.

The 4,096-by-4,096-block, step-16 review regions produced:

| Seed / center chunk | Candidates | Accepted | Planning ms | Accepted route lengths |
|---|---:|---:|---:|---|
| `12345` / `(0,0)` | 1,036 | 10 | 420.5 | 93-96 |
| `424242` / `(0,0)` | 1,038 | 2 | 196.0 | 93-95 |
| `-98765` / `(183,-177)` | 1,024 | 10 | 463.7 | 91-96 |

The seed `-98765`, start chunk `(147,-126)` step-1 maps show the chosen
centerline following the visible low band in the untouched height field. Its
plan is intentionally still a coarse polyline; Slice 3 owns the continuous
valley cross-section and first actual pixels.

### Slice 3: clipped terrain and water realization

- [x] Replace revision 11's raised contour landmark with the accepted stream
  plan behind a new field/landmark revision.
- [x] Apply stream terrain intent before surfaces and vegetation.
- [x] Realize headwater, reach, riffle/cascade/fall, receiver, and confluence
  pieces through per-target clipping.
- [x] Preserve plane target partition and cache-independent output.
- [x] Preserve the periodic seam and update intentional output fingerprints.

Gate: exact generated blocks are independent of target request order and no
piece writes beyond its clipped target box.

First realization checkpoint 2026-07-23:

- surface dependencies and biome resampling now share one seed/topology-scoped
  positive and negative plan cache rather than repeating bounded route search
  for every column or chunk;
- each affected chunk reconstructs only metadata starts within the vanilla
  radius-eight reference square, applies the immutable continuous column
  query to its own padded terrain samples, then runs ordinary surface and
  decoration language;
- the cross-section cuts only: a three-to-five-block calm channel, deeper
  rounded source and receiving pools, and an approximately eighteen-block
  shoulder envelope blend into untouched terrain without revision 11's raised
  containment shelf;
- a small deterministic lateral offset is applied to intermediate geometry
  nodes, preserving endpoints and the accepted route envelope while avoiding
  a ruler-straight rendered centerline;
- planned banks use grass/soil rather than inheriting the broad Y63 river's
  low sand-bank rule; and
- combined, reversed, partitioned, and warm-cache target generation produce
  identical exact chunks at the reviewed accepted start.

The first naive transition used a route projection for flowing-water levels.
The authoritative wake exposed why that is insufficient: runtime water uses
cardinal shortest paths, and a diagonal lip can give one cell two source
neighbors, converting it into a new source and propagating across the lower
reach. The accepted stencil narrows only the transition to a one-cell
cardinal stair-step throat, then bakes a bounded two-dimensional Manhattan
apron with levels zero through seven. Calm reaches retain their full width.
This is a fixed discrete structure-piece stencil, not a generation-time fluid
simulation.

The reviewed seed `-98765`, start `(147,-126)`, now renders a continuous
roughly 95-block source-to-river stream with three drops, a shallow grassed
valley, a rounded headwater pool, and a widened confluence. RD16 production
top-down, landscape, and elevated evidence is under
`/tmp/mclone-stream-card-meander`. The first complete pixels are materially
more natural than the rejected shelf. This checkpoint deliberately retained
field revision 11 until the following cleanup and periodic gates passed.

Revision closeout 2026-07-23:

- field revision 12 and decoration revision 9 identify the procedural valley
  stream output;
- the disabled revision-11 tributary noise domains, selector, contour solver,
  containment branch, and vocabulary have been removed rather than retained
  as dead generation code;
- the surviving watercourse facts are named for the planned stream and its
  headwater instead of the rejected raised tributary;
- raw terrain, biome, and surface-language fingerprints were intentionally
  updated to the revision-12 no-landmark baseline; procedural stream output is
  proven separately at its accepted starts; and
- seed `12345`, periodic candidate `(3,-935)` owns a reviewed plan whose
  bounding box crosses the X seam. The exact generated stream chunk equals
  its 384-chunk lift.

### Slice 4: hydraulic, LOD, and persistence closeout

- [x] Extend closure diagnostics to every planned reach and transition.
- [x] Wake every water cell through the authoritative server and require zero
  unintended mutation and an empty queue.
- [x] Route synthetic far LOD and review sampling through the same immutable
  plan/column facts.
- [x] Record the accepted deterministic reconstruction or persist versioned
  start/reference metadata across native SQLite and browser IndexedDB reopen.

Gate: full chunks, LOD, reload, native worker, and browser Worker agree on
start identity, route, bounding box, and water levels.

Hydraulic checkpoint 2026-07-23:

- the complete nine-by-nine source-to-sink audit reports source and flowing
  water, at least three intentional transitions, zero unsupported cells, zero
  horizontally open source faces, zero unexplained unequal-level edges, and
  zero generation-time liquid ticks; and
- the authoritative server schedules every water cell inside the accepted
  plan, executes the complete queue, observes zero block mutations, and
  finishes with an empty queue.

LOD checkpoint 2026-07-23:

- far-LOD source/build keys and portable worker inputs now carry the active
  `WorldGenerationProfile`; changing seed or profile invalidates retained
  synthetic coverage;
- native scene startup prewarm, native live rendering, mono/XR scene calls,
  the browser scene, the browser doorbell, and the Web Worker compile entry
  all preserve the profile instead of silently sampling vanilla Overworld;
- Mclone native LOD workers reuse one bounded stream-plan cache across their
  neighboring generated surface chunks;
- a focused accepted-start fixture samples every reviewed route node through
  the cached LOD source, matches the authoritative Mclone surface column,
  observes planned water, differs from vanilla surface output, and records
  plan-cache hits; and
- all 309 `mclone-app-runtime` library tests pass, while native and
  `wasm32-unknown-unknown` browser checks compile the complete transport
  boundary. Existing unrelated target-specific warnings remain.

Persistence checkpoint 2026-07-23:

- the first internal/unshipped revision uses deterministic reconstruction,
  not a new persisted structure-start record: seed, profile field revision,
  topology, structure kind, and canonical start chunk fully determine the
  immutable plan;
- exact candidate reconstruction, request-order/partition/cache independence,
  and periodic seam ownership are already pinned by the planning and
  realization fixtures;
- a native threaded SQLite fixture now generates a reviewed mid-route stream
  chunk, proves its centerline source water, closes persistence, reopens the
  world, and receives an exactly equal full `ChunkSnapshot` with
  `LoadedFromStore` residency; and
- this is intentionally scoped to the current `internal-mutable`
  `mclone-overworld-v1` ledger. Before a release freeze or queryable gameplay
  such as `/locate`, the project must add versioned start/reference metadata
  rather than relying only on deterministic replay and final chunk blocks.

Browser IndexedDB already stores the same portable chunk record and generation
descriptor codecs used by its scheduler path. A browser-specific database
reopen test is not required to prove a new structure metadata record because
this revision adds none; worker/profile equivalence and the shared record
codec remain the relevant browser gates.

### Slice 5: performance and human review

- [x] Compare revision 11 and the candidate on the same clean host at an
  accepted-stream hotspot and a no-stream control.
- [x] Run the accelerated one-minute movement soak with fluid counters.
- [x] Capture high-view-distance top-down, landscape, confluence, headwater,
  and along-stream views from fully warmed production chunks.
- [x] Inspect every first drawable milestone before adding the next visual
  layer.

Performance correction checkpoint 2026-07-23:

- the first three-iteration hotspot probe exposed the intended investigation
  gate: before follow-up optimization, seed `-98765` around `(149,-124)`
  measured 274 surface chunks/s, 468 cold decorated targets/s, and 1,822 warm
  targets/s, versus 2,055, 880, and 5,097 around the no-stream origin
  control. The source was redundant immutable metadata work, not liquid
  simulation;
- `McloneOverworldStreamPlanCache` now retains at most 4,096 candidate plans
  and 2,048 clipped chunk-intersection queries. Repeated surface/biome and
  warm target requests reuse the latter instead of rescanning the vanilla
  radius-eight reference square;
- immutable rendered route coordinates are computed once per accepted plan
  and stored as exact floating-point bit patterns. Column sampling no longer
  repeats trigonometric meander construction for every route segment and
  terrain column;
- a shared-cache surface-batch API makes the metadata dependency explicit.
  The production feature batch, native LOD worker, browser LOD tile compiler,
  and benchmark all reuse a bounded cache; the isolated single-chunk
  convenience API remains correct but is not the performance lane for
  adjacent generation;
- the benchmark receipt now exposes planner requests/hits, clipped-query
  requests/hits and retention, plus accepted/rejected plan counts; and
- exact batch-versus-isolated blocks, raw field fingerprints,
  partition/order/cache independence, all 306 worldgen tests, all 309 app
  runtime tests, and the browser WASM check pass. Final same-host numbers and
  the movement soak follow from a committed clean build.

Final performance and review checkpoint 2026-07-23:

- revision 11 at `2ea92637` and the field-revision-12 candidate at
  `9bbb240d` were built into separate release targets, alternated twice on the
  same host, and pinned to CPU 15. Every sample generated a radius-three
  region for 20 iterations;
- at seed `-98765`, chunk `(149,-124)`, revision 11 averaged 1,741 surface
  chunks/s, 561 cold decorated targets/s, and 2,623 warm targets/s. Revision
  12 averaged 1,065, 381, and 1,945 respectively: `-38.8%`, `-32.0%`, and
  `-25.9%`;
- at the no-stream origin control, revision 12 versus revision 11 changed by
  `-4.3%`, `+3.7%`, and `+20.9%`. This isolates the material cost to planned
  stream terrain rather than a broad field regression;
- within revision 12, the accepted-stream hotspot is 1.69x, 1.62x, and 1.91x
  slower than its control. The greater-than-25-percent investigation gate was
  therefore justified and completed by the cache/geometry correction above;
  no lane reaches the twofold visual-acceptance blocker. Absolute cold
  throughput remains about 381 targets/s on one pinned CPU;
- the committed clean 3,600-frame RD10 movement run used a radius-eight path
  at 32 blocks/s and 60 Hz. It averaged 8.303 ms, with p95 15.978 ms, p99
  24.703 ms, 43.563 ms maximum, 157 over-budget frames, six over 2x budget,
  and none over 4x;
- the movement run scheduled, found due, executed, deferred, and mutated zero
  fluid work. Generation jobs, publications, render chunks, and render
  compiles remained bounded at maxima of 1, 94, 129, and 9 respectively;
  the server update queue remained zero. Active movement still had ordinary
  interest/render work at the final frame, but none of the queues grew
  without bound; and
- the final clean receipt at seed `-98765`, chunk `(149,-124)`, warmed all
  1,225 expected RD16 chunks and finished with zero target render work
  pending. Its top-down and elevated panels expose the complete headwater,
  four calm reaches, three transitions, downstream view, and confluence;
  the landscape panel verifies the valley at traversable scale. The inspected
  card and full-size source PNGs are under
  `/tmp/mclone-stream-final-card-clean`.

The clean A/B reports are under `/tmp/mclone-stream-{current,rev11}-*.json`;
the movement receipt is `/tmp/mclone-stream-movement-soak.json`. These are
disposable host evidence and are intentionally not repository artifacts.

Human review asks:

- Does the stream visibly occupy several calm reaches rather than only a tiny
  waterfall mouth?
- Does it follow and enhance an existing valley?
- Are one- and two-block transitions legible without looking like stairs?
- Do shoulders read as carved terrain rather than a levee or retaining wall?
- Does the confluence widen and blend naturally into the major river?
- Is the result worth its generation and LOD cost?

Human Review 1 accepted the complete RD16 result on 2026-07-23. The intended
visual language landed: it reads as “peaceful,” specifically “like a spring
feeding a creek.” Preserve that modest headwater, calm scale, and gentle
valley relationship in later variation work rather than turning every
instance into a dramatic waterfall landmark.

## Stop Conditions

Pause for direction before expanding scope if:

- the current scheduler cannot represent metadata-only radius-eight
  prerequisites without materializing terrain;
- persistence requires an incompatible chunk-record or world-version change;
- exact periodic seam ownership requires a global mutable registry;
- current fluid rules cannot express a stable sequence of bounded one- and
  two-block transitions;
- accepted routes are too rare without weakening natural-bank requirements;
  or
- the first complete route and valley pixels present a subjective choice
  between materially different visual languages.

Otherwise continue through objective gates and stop at the first complete
human visual review point.
