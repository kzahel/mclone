# Mclone Overworld Generation

Topic: `mclone-overworld-generation`

Status: the first continuous-terrain caller, three visual/distribution reviews,
the first biome/surface/decoration language, two reuse checkpoints, and full
host/persistence closeout completed 2026-07-18 as the separate
internal-mutable `mclone-overworld-v1` profile while `overworld` remains the
Minecraft Java 1.17.1 reference path. Tactical
[`188`](../tactical/188-mclone-overworld-v1-terrain-foundation.md) is complete.
Tactical [`192`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
landed its first two-field mountain geometry, a human-requested shorter
traversal-scale tune, and the slope/exposure-aware surface and vegetation
response on 2026-07-22. Human Review 1 accepted the geometry, but Human Review
2 found the resulting mountain surfaces too smooth and the dominant relief too
large-scale. A measurement-only terrain-characteristics checkpoint confirms
that finding against exact undecorated Minecraft Java 1.17.1 terrain. Field
revision 5 closed much of the scale gap but human review rejected its aligned
diagonal terrace pattern. Field revision 6 replaces it with periodic-ready
warped gradient detail; Human Review 3 accepted its more natural, less
geometric result and production browser Worker closeout passed. The shared
Flat Grass cylinder proof and
[`196`](../tactical/196-periodic-mclone-terrain-fields.md) are complete. The
entire accepted terrain, biome, surface, spawn, and vegetation pipeline now
supports either the ordinary plane or the exact 6,144-block / 384-chunk
X-periodic cylinder. The selected terrain sequence is now at rivers and
wetlands: Tactical
[`220`](../tactical/220-mclone-overworld-rivers-and-wetlands.md) has landed its
bounded broad-river field, graded channels and banks, local water level, river
biome and substrate language, sparse shallow wetland pools, production maps,
and periodic seam proof. Human Review 1 subsequently rejected its pointwise
terrain-relative water height: it permits transverse water slopes and
uncontained source faces that spill when woken. Corrective flat contained
field revision 8 now limits physical water to constant-Y63 lowland reaches.
Multi-seed closure audits, a real fluid-runtime wake test, RD16 pixels, release
cold/warm generation measurements, and an accelerated 3,600-frame movement
soak pass. Human Review 2 nevertheless rejected the world language: every
physical river still shares one level and disappears at the conservative
lowland gate, while large water bodies have smooth shallow floors without
size-aware shelf and deep-basin structure. Field revision 9 now supplies
size-aware shelf/deep-basin bathymetry. Field revision 10 then supplied local
four-block flat levels joined by bounded baked drops, but interactive review
found that its locally downhill choices can still produce a river whose level
falls and later rises. Field revision 11 replaces that rejected global
stepping with sea-level major rivers plus sparse, short, closed tributary
landmarks: an explicit source pool and Y67 upper reach descend through one
baked four-block fall into an already-valid Y63 major river. Closure,
authoritative fluid wakes, hotspot generation performance, a 3,600-frame
movement soak, maps, and final RD16 pixels pass. The compromise deliberately
does not claim a drainage network, general highland rivers, confluences, or
globally monotonic macro flow. Interactive review subsequently accepted the
mechanical fixed-point proof but rejected the grass-topped containment shelf
and tiny visible fall. Tactical
[`222`](../tactical/222-bounded-valley-stream-structures.md) now owns a
48-96-block, structure-shaped, valley-following replacement. Its reusable
start/reference kernel, plan-only Mclone consumer, and first clipped terrain
realization are live. Three reviewed seeds produce bounded 91-96-block plans
with monotonic integer reach levels, zero required fill, typed pieces,
cacheable identities, and production candidate/route/cut maps. The first
complete reviewed route cuts a grassed shallow valley, calm multi-block
reaches, a rounded headwater, three cardinal fixed-point drop stencils, and a
widened Y63 confluence without any containment fill. Exact target partition,
full-route closure, authoritative all-water wake, and RD16 production pixels
pass. Field revision 12 and decoration revision 9 remove the dead revision-11
contour/berm solver, adopt planned-stream vocabulary, update intentional raw
field fingerprints, and prove exact seam-crossing realization on the
384-chunk cylinder.
Synthetic far LOD now carries the active generation profile through startup,
scene runtime, native workers, browser doorbells, and Web Workers. Its Mclone
source reuses the same immutable bounded stream-plan cache and matches
authoritative stream columns at the reviewed route. A reviewed mid-route
chunk also survives native SQLite close/reopen as an exactly equal snapshot
loaded from storage. Deterministic plan reconstruction is accepted while the
profile remains internal-mutable; versioned queryable start/reference records
are required before release freeze. The final same-host comparison isolates
the accepted-stream cost to the new structure-shaped terrain: cold generation
is 32.0 percent below revision 11 at the hotspot but remains roughly 381
targets/s on one pinned CPU, while the no-stream control is 3.7 percent
faster. The hotspot remains below the twofold blocker in every lane. A
3,600-frame RD10 traversal records zero fluid work and bounded generation and
render queues, and fully warmed RD16 production pixels pass internal
inspection. Human Review 1 accepted the result as peaceful, “like a spring
feeding a creek.” Tactical 222 is complete.
Tactical
[`223`](../tactical/223-mclone-climate-and-bookend-biomes.md) then adds
periodic temperature and moisture, altitude cooling, and the first
cool-wet-conifer, snowy-alpine, and warm-dry-steppe recipes. Field revision 13
and decoration revision 10 first carried taiga, snowy-mountain, and
savanna-compatible
biome IDs through Mclone-owned surfaces and feature tables. Far-LOD snow,
SQLite reopen, exact periodic seams, browser WASM compilation, same-host
generation benchmarks, a mixed-climate one-minute movement soak, and fully
warmed RD16 cards pass. Human Review 1 accepted the vocabulary but found the
steppe too small. Field revision 14 and decoration revision 11 retain the old
warm-dry core and add a continuous, sparser regional shoulder. Multi-seed
component maps, alternating old/new generation measurements, and three fully
warmed RD16 cards pass; Human Review 2 is pending. The next recommended
terrain campaign remains sustained three-dimensional geology work, comparing
placed rocks, bounded formation structures, and selective regional density
modifiers before caves.
Interactive watercourse review then exposed a narrower correction before that
campaign. At a seed-`8675309` coastal outlet, the three-to-four-block major
river bed is clipped at the coastline and replaced by bathymetry's initial
two-block shelf. Lighting makes the raised underwater sill appear as a water
color boundary. The same review found major rivers and planned streams too
smoothly and symmetrically carved, with insufficient width/depth variation,
pools, riffles, substrate patches, and rocks; the binary grass-to-exposed-stone
surface rule also draws stark contour lines across valley walls. Tactical
[`225`](../tactical/225-mclone-watercourse-morphology-and-coastal-outlets.md)
owns the fixed-work outlet, morphology, mixed-surface, and sparse-local-feature
correction while preserving flat source planes and the accepted peaceful
stream family.

## Scope

This topic owns the creative and technical direction for mclone's original
Overworld terrain: macro fields, terrain shape, biome placement, surface
language, decoration, rivers, caves, geology, landmarks, and eventual
structures.

It does not own generic profile persistence, worker transport, scheduling,
lighting, or publication. Those contracts and their compatibility safety
ledger remain in
[`world-generation-profiles.md`](world-generation-profiles.md). It also does
not change the reference target recorded in
[`../reference-minecraft.md`](../reference-minecraft.md).

## Product Roles

- `overworld` remains reference-locked to Minecraft Java 1.17.1 output. It is
  the behavior baseline and a source of proven algorithms, not the creative
  profile to mutate.
- `small-island-v1` remains an internal, bounded proving ground for reusable
  noise, surfaces, placed features, dependency planning, and visual review.
- `mclone-overworld-v1` is the live internal continuous, explorable, original
  terrain profile. Its current output may still change in place under the
  compatibility safety ledger.

Small Island must not gradually become the full Overworld. It should pressure
the new profile toward useful shared mechanisms while keeping its own bounded
landform rules. The new profile must not fork scheduler or platform policy.

## Intended Experience

The first original Overworld should be recognizably Minecraft-like in its
block scale, legibility, and progression from terrain to surface to features,
while owning its exact seeds, fields, rules, and content combinations.

The long-term terrain vocabulary includes:

- continuous oceans, coasts, lowlands, uplands, mountain ranges, and valleys;
- climate- and terrain-aware biome placement;
- rivers and wetlands that participate in macro terrain before surface
  materials are applied;
- original surface and decoration combinations built from shared feature
  mechanisms;
- later caves and geological formations with independently seeded 3D rules;
- local landmarks as placed features and, when justified, persistent
  cross-chunk structures.

Minecraft terrain systems are a design and architecture reference, not a seed
parity target for this profile. Concepts such as continentalness, erosion,
ridges, temperature, and moisture are useful vocabulary, but the profile does
not port the Minecraft 1.18 density-function/spline stack or the disabled
1.17.1 Caves & Cliffs paths.

The community
[`JJThunder To The Max` reference study](jjthunder-to-the-max-reference.md)
reinforces the structured-field direction without changing that boundary. Its
strong candidates are specialized landform recipes behind macro selectors, a
direct finite-difference gradient attenuation experiment, preserved raw versus
processed relief, one river field shared by terrain and biome classification,
and relative overburden for later caves. Its 2,096-block height, registry
replacement model, and literal JSON spline tables are not implementation
targets. The study does not justify importing its literal implementation.
Mountains and periodic fields are now complete; the first bounded
river/wetland family is at human review, while caves remain deferred to their
own slice.

## Reference Vocabulary And Deliberate Divergence

Minecraft Java 1.17.1 remains the executable reference for pipeline order,
surface and feature vocabulary, deterministic chunk ownership, and the
legibility expected at block scale. Its exact terrain composition is not the
creative target for this profile.

The useful 1.17.1 lessons are separation rather than wholesale algorithm
reuse:

- biome depth and scale are smoothly blended before density sampling;
- base terrain precedes surface builders, local lakes and springs, and later
  decoration;
- river-shaped biome layers create an inexpensive, continuous visual result;
  and
- local lake and spring features validate their immediate solid/liquid
  neighborhood before placing water.

Mclone deliberately improves the ownership of landforms and water:

| Concern | Java 1.17.1 role | Mclone original direction |
|---|---|---|
| Mountains | biome depth/scale contributes geometry | terrain owns mountain regions, crests, shoulders, and valleys; biome and surface react |
| Rivers | a two-dimensional river layer mixes a low river biome into terrain | a shared watercourse contract owns channel influence, grade, width, bed, banks, and reach identity |
| Lakes | bounded local ellipsoid feature | retain small ponds as local features; add basin-aware lakes only after macro water facts exist |
| Springs | locally constrained source placement | retain as cave/cliff accents rather than treating them as a river network |
| Waterfalls | usually incidental fluid placement and flow | classify a continuous reach with an upstream supply and downstream destination |

This direction does not port `Aquifer`, `Cavifier`, `NoodleCavifier`, ore-vein
paths, the Minecraft 1.18 density-function stack, or other systems excluded by
the Java 1.17.1 target. A later profile-owned 3D density tactical remains
available if reviewed cliffs, overhangs, or geology demonstrate a concrete
need.

## Landform Composition Direction

Variation is hierarchical so that adding detail does not become independent
noise soup:

```text
continental land/ocean intent
  -> broad landform-region control
  -> connected ridge and valley organization
  -> shoulders, foothills, and local relief
  -> derived height, slope, curvature, and exposure
  -> biome, surface, vegetation, and later water response
```

Tactical 192 starts with exactly two new live semantic facts:

- `ruggedness`: a broad signed regional control selecting where strong relief
  belongs; and
- `ridges`: a normalized connected-crest signal inside those regions.

Valleys are initially the traversable low relation between the regional
control and ridge signal, not a third stored noise field. Foundation
`continentalness` and raw `relief` retain independent domains and raw values.
The derived height may change intentionally for this internal profile, while
ocean floors, coasts, and ordinary lowland controls should remain recognizable.
At most one additional field may enter Tactical 192 if the first production
maps and pixels demonstrate a specific missing degree of freedom.

The initial temperate landform family should support rounded wooded ranges,
connected rocky crests, layered grassy shoulders, broad pastoral valleys, and
occasional steeper faces. Later selectors may add sharp ranges, plateaus and
escarpments, isolated massifs, high basins, or gorge terrain. Those are
landform recipes selected by macro facts, not separate uncoordinated octave
stacks applied everywhere.

Slope and curvature are derived facts. They should be computed at the smallest
spacing needed by a real caller and should not require a neighborhood chunk
dependency. Tactical 192 promoted a four-block central-difference slope into a
profile-owned landform sample once both biome and surface selection needed the
same semantics. It remains separate from the raw terrain sample, and the
production chunk path derives it from a bounded 20-by-20 sample halo rather
than adding a chunk dependency or five independent queries per column.

## Water-System Direction

Rivers and wetlands followed Tactical 196, so every live water field is
periodic and seam-correct from its first production revision. Water is a macro
terrain input, not a biome decal or a post-surface trench.

Tactical 220's first implementation uses a warped zero-contour corridor whose
broad, detail, width, and wetland-pool scales are 768, 192, 384, and 96 blocks.
An analytic gradient-noise derivative supplies bounded centerline distance and
tangent estimates. The shared production sample now owns:

- base terrain elevation before water carving;
- channel distance, influence, half-width, bed, and local water surface;
- graded-bank and low-gradient floodplain influence;
- tangent, provisional flow orientation, and local grade; and
- wetland and sparse shallow-pool influence.

Terrain, biome, surface material, spawn, review maps, and feature exclusion
consume those facts. Riverbeds use gravel, wetland pools use clay, inland banks
remain grassy, and coastal banks inherit sand. A reviewed coarse-dirt marsh
band was rejected because it made floodplains look engineered.

Field revision 7's hydraulic realization is rejected. Sampling a smooth water
height independently at each column does not create a physically meaningful
water surface: the gradient may point across the channel, and lowering banks
cannot guarantee lateral containment. The correction begins with constant
integer-level, naturally contained lowland reaches and removes water where a
bounded cut/fill budget cannot make the initial source body a fixed point of
the runtime fluid rules.

Field revision 8 implements a useful hydraulic foundation without adding a
reach graph or chunk-time simulation. Every physical channel and shallow pool
uses the global Y63 source-water surface. Realization is limited to lowlands
whose broad surface is at most Y70 and base surface at most Y72. The existing
sea fill therefore closes every source boundary against either another Y63
source or motion-blocking natural terrain. The generator may lower terrain
into that body but never raises a levee to rescue a high reach. Highland
corridor distance, tangent, grade, and width remain useful dry planning facts;
physical higher water waits for named reaches and explicit bounded drops.

Human Review 2 confirms that this foundation is not an acceptable complete
water system. Because every implemented reach is Y63, it contains no visible
level change. Because realization is disabled above the lowland thresholds,
the corridor can vanish without a headwater, outlet, or other semantic
termination. Preserve constant-level source bodies and their closure proof,
but apply them per explicit reach rather than treating the global sea as the
only reach.

Field revision 9 makes ocean bathymetry an independent terrain family.
Negative continentalness supplies continuous ocean-interior, shelf,
shelf-break, basin, depth, and seabed-relief facts. An independent 1,536-block
basin selector and 384/96-block seabed relief preserve exact 6,144-block X
periodicity without a connected-component query. Coastal depth begins at two
blocks, the shelf contributes roughly ten more, and basin selection plus
relief may reach the configured 52-block maximum. Seed `-98765`'s reviewed
period spans depth 2 through 40 with p10/p50/p90 depths 3/9/30. The surface
remains source-flat at Y63 while the accepted RD16 deep-water card reaches a
contoured floor at Y23.

Interactive review subsequently found one missing composition invariant.
Major-river carving returns as soon as continentalness becomes ocean, while
the ocean starts at two blocks of depth. A typical three-to-four-block river
therefore meets a shallower receiving shelf. This is physical floor geometry,
not a biome-water tint or LOD boundary. Tactical 225 must continue a fading
submerged thalweg beneath the inner shelf, select the deeper of outlet and
ordinary bathymetry, and tighten the generic shelf progression after a narrow
shallow margin. It must not trace the coast or turn the receiving ocean into
an infinite river biome.

The same review makes watercourse morphology a first-class local fact.
Accepted routes remain the macro skeleton. Fixed-work meso-scale morphology
varies width, bed depth, thalweg offset, pools/riffles, bend asymmetry, and
left/right banks; micro-scale morphology varies edges, substrates, ledges,
and sparse local rocks. Every surface remains a flat integer source plane,
and the first dry containment ring remains clamped above it. Correlated
variation replaces repeated cross-sections; per-block white noise is rejected.

Field revision 10 is the first local stepped-reach realization. It projects
each column onto the analytic centerline before sampling a hydraulic terrain
profile, then quantizes that profile into four-block source planes from Y63
upward. Provisional flow comes from two fixed probes 16 blocks along the
centerline tangent. Crossing a level boundary selects a bounded two-block
source lip, narrow falling-water sheet, and deeper receiving pool. A tiny
digital-distance calculation over the existing two-block sample halo assigns
the falling sheet's top flow level. Immediate non-channel edge columns form a
containment collar; production performs no fluid simulation, arbitrary
upstream search, or new chunk dependency.

This revision provides inspectable level, local flow direction, grade,
drop-distance, upper/lower level, drop height, falling-column, and receiving
pool facts. It does **not** complete the intended reach vocabulary. The
zero-contour corridor can still close into loops, local downhill choices are
not a globally monotonic drainage proof, and there is no explicit headwater,
outlet, confluence, accumulated discharge, or named reach identity. One
integrated river-to-shelf-to-deep-basin outlet remains a review gap.

Interactive review rejected revision 10 as a global river model. Quantizing a
local hydraulic potential does not establish one persistent source-to-sink
ordering: a player can follow the corridor down one step and later encounter
an uphill step. Increasing the local sample radius cannot prove otherwise.
The implementation remains useful evidence for stable lips, falling sheets,
and receiving pools, but its levels must not be interpreted as drainage
topology.

Field revision 11 adopts the bounded compromise. The major warped-contour
river is always one Y63 source plane, including where a broad graded valley
must lower higher land toward it. Raised water exists only in a local
hydrology landmark with all four roles present:

1. a round source pool;
2. a roughly 31-40-block Y67 upper tributary;
3. one four-block rock-lip, falling-sheet, and receiver transition; and
4. the existing Y63 major river as its explicit sink.

An independent 384/128-block tributary contour provides the short natural
alignment. A bounded two-refinement local intersection solve reconstructs the
same major-river/tributary anchor from each participating column; it neither
walks the river nor consults generated chunks. A 768-block selector keeps the
landmarks sparse. The sample exposes separate major-channel,
raised-tributary, and source-pool influences so review and later structure
siting do not have to infer those roles from water blocks.

Containment is part of the authored landmark. A seven-block local support band
keeps the upper source plane below its banks. During final column realization,
a four-neighbor check converts any rare exposed high-source fringe into a
solid one-column berm. This is fixed local work over the existing two-block
sample halo, not fluid simulation, recursion, or another chunk dependency.
The major river's immediate bank is Y64 and then grades back to accepted
terrain across a width proportional to incision, avoiding both floating water
and the initial sheer-canyon artifact.

The production hydraulic-closure audit checks generated chunks with a
one-chunk halo. It rejects horizontal source faces open to non-solid cells,
unsupported source or flowing water, unequal tops on adjacent ordinary source
columns, and initial liquid ticks. It classifies unequal tops across the
bounded falling stencil as intentional drop edges. The revision-11 review
landmark at seed `-98765`, chunk `(183,-177)`, contains 1,409 source and 16
flowing blocks across its audited nine chunks, with four intentional drop
edges and zero open faces, unsupported cells, accidental sloped edges, or
initial liquid ticks. An authoritative server test wakes every water cell in
those chunks; every tick drains and zero blocks change. The periodic ordinary
river audits remain closed. The audit is review/test work and adds no chunk
dependency or fluid work to production generation.

This corridor family is fixed-work and visually continuous, but it is not yet
a drainage network. Generic zero contours may form closed loops and do not
provide tributaries, confluences, accumulated discharge, named reaches, or
true downstream identity. If human review requires true drainage before
smaller streams, coarse network construction belongs in canonical macro tiles
with an explicit finite halo and a descriptor-keyed cache; point queries
consume bounded reach facts. No column sampler may trace arbitrarily far
upstream, run an unbounded flood fill, or search until it finds an ocean.

Reach grade now selects the first bounded four-block fall and receiving pool.
It may later distinguish calm water, riffles, rapids, smaller cascades, and
gorges. A generalized waterfall must have continuous upstream and downstream
watercourse facts; the local flow estimate is not a substitute for that
network identity. Low-gradient reaches may meander or support floodplains and
wetlands. Not every valley receives a river, and small configured ponds and
springs remain useful local accents.

This shared vocabulary also gives structures stable site facts for bridges,
fords, mill races, water wheels, irrigation, ponds, and scenic settlement
placement without making structures infer hydrology from final blocks.

## Generation Cost And Quality Policy

World generation must remain a bounded, inspectable workload. The default
production path follows these rules:

- point sampling is `O(live field count)` with a fixed amount of work per
  field;
- region and chunk callers batch or cache lattice work when measurement shows
  repeated hashing to be material;
- macro planners use fixed-size canonical tiles and finite halos rather than
  request-order-dependent global searches;
- terrain, biome, surface, spawn, water, and review tools consume one
  production implementation rather than recomputing approximate variants;
- a new field records its sampling cost and its full cold-generation cost;
- caches are keyed by every output-affecting descriptor fact, including seed,
  profile revision, topology, and any future generation-quality selection;
  and
- distant LOD may simplify rendering, but it must not silently become a
  different authoritative block generator.

Bounded stream metadata follows the same rule. Candidate plans and clipped
chunk-intersection results live in capped, seed/topology-scoped caches;
rendered meander coordinates are computed once per immutable accepted plan.
Adjacent surface generation uses a shared-cache batch API. Production feature
batches and native/browser LOD reuse that contract rather than repeatedly
solving the same radius-eight metadata neighborhood. Benchmark receipts expose
both plan-cache and clipped-query hit/retention counters so a visually sparse
structure cannot hide unbounded or redundant planning cost.

The 2026-07-22 Linux Slice 0 baseline at commit `8cfa1ac4`, seed `12345`,
center `(0,0)`, radius one chunk, and three release iterations measured:

| Path | Baseline |
|---|---:|
| Mclone surface | 3,279.500 chunks/s |
| Mclone cold decorated targets | 600.997 chunks/s |
| Mclone warm decorated targets | 4,654.574 chunks/s |

The 148,225-point, 16-block-stride production review grids sampled in 12.392
ms for seed `12345` at origin and 17.457 ms for seed `-98765` around chunk
`(-96,72)`. Cross-host timings are not compatibility locks; compare before
and after on the same host. A greater than 25 percent regression in full cold
Mclone target throughput requires an explanation and focused profiling. A
twofold regression blocks a visual slice unless the tactical records an
explicit product tradeoff and optimization follow-up. Field-map time and
surface-only throughput remain diagnostic attribution gates even when full
generation stays within budget.

Tactical 192's landed terrain-language path uses a 20-by-20 sample halo for
each 16-by-16 chunk. On the same release command it measured 3,730.502 surface
chunks/s, 596.967 cold decorated targets/s, and 5,267.459 warm decorated
targets/s. Cold generation is 5.1 percent below the preceding field-revision-4
measurement and effectively equal to the Slice 0 baseline, so the shared
slope/exposure response does not consume the river slice's performance budget.
The review tool's exact five-point landform maps take about 69 ms for 148,225
points; that intentionally simple diagnostic path is not the production chunk
sampling strategy.

Tactical 196's release comparison at seed `-98765`, radius three, and three
iterations measured 1,133.096 plane versus 1,115.150 cylinder cold decorated
targets/s, and 6,079.168 versus 5,885.992 warm targets/s. The 1.6 and 3.2
percent overhead is acceptable. Surface-only throughput fell from 3,378.742
to 2,573.646 chunks/s, so field-level periodic modulo and lattice work remain
an optimization signal even though full cold generation stays well inside the
25-percent gate. Both modes made 363 dependency requests, generating all on
the cold pass and hitting all on the warm pass. The first browser Worker
request was about 1 KiB and its generated render-distance payload about 23
MiB; periodicity does not claim to solve payload size or compression.

At the same seed, radius, and iteration count, Tactical 220's reviewed plane
measured 2,911.666 surface chunks/s, 986.434 cold decorated targets/s, and
5,339.303 warm targets/s. Those are 13.8, 12.9, and 12.2 percent below the
Tactical 196 plane result and remain inside the 25-percent gate. The cylinder
measured 2,777.558, 896.640, and 4,777.684 respectively. Analytic derivatives
replaced an initial finite-difference centerline estimate whose surface-only
path was about 42 percent below the baseline.

The field-revision-8 corrective release measurement at clean commit
`2feb9189`, seed `-98765`, radius three, and three iterations measured
2,857.849 plane surface chunks/s, 904.243 cold decorated targets/s, and
5,053.663 warm targets/s. The exact cylinder measured 2,571.381, 868.850, and
4,352.229 respectively. The plane result is 15.4, 20.2, and 16.9 percent below
Tactical 196's accepted plane baseline, so all three remain inside the
25-percent investigation gate. The warm pass served all 363 dependency
requests from cache and generated none.

The clean release movement evidence uses a river-heavy, eight-chunk-radius
route around seed `-98765` chunk `(47,102)` at render distance 10. The
canonical offscreen harness advances 3,600 frames at 60 Hz and 32 blocks/s,
representing 60 seconds of motion but deliberately does not sleep to wall
clock. Across 64 unique chunk centers it measured 3.358 ms average frame work,
1.567 ms accounted frame-wall p50, 7.533 ms p95, 10.587 ms p99, and 13.108 ms
max with no over-budget frames. Worldgen and publication queues ended empty;
loaded chunks stayed between 529 and 576. Maximum and final scheduled-fluid
depth, as well as due, executed, deferred, mutation, snapshot, and event
counters, were all zero.

The revision-10 stepped-reach comparison was captured on the current Linux
workstation and therefore does not compare raw throughput with the preceding
Apple M4 records. An alternating same-host A/B built the bathymetry-only parent
and revision 10 separately, then ran two 20-iteration radius-three passes of
each at seed `-98765`. The parent averaged 2,272 surface chunks/s, 762 cold
decorated targets/s, and 4,031 warm targets/s. Revision 10 averaged 2,368,
771, and 3,836: +4.2 percent, +1.2 percent, and -4.9 percent respectively.
This is inside the 25-percent investigation gate. Hydraulic sampling returns
before its three profile probes on ocean and columns more than 48 blocks from
the corridor, which keeps the extra fixed work local.

The matching waterfall-centered movement run used chunk `(-103,185)` and the
same RD10, eight-chunk-radius, 32-block/s, 3,600-frame/60 Hz shape. Across 64
unique chunk centers, offscreen work measured 6.023 ms average, 12.691 ms p95,
14.746 ms p99, and 22.003 ms max; 10 frames exceeded the 16.667 ms budget and
none exceeded 2x. Publications ended empty, while a 17-worldgen-job and
94-render-chunk tail remained because the accelerated route was still moving.
Maximum/final scheduled-fluid depth and all due, executed, deferred, mutation,
snapshot, and fluid-event totals were zero.

The revision-11 hotspot comparison used separately built revision-10 and
revision-11 binaries on the same Linux host at seed `-98765`, chunk
`(183,-177)`, radius three, and 20 iterations. Revision 10 measured 2,331
surface chunks/s, 771 cold decorated targets/s, and 3,794 warm targets/s.
Revision 11 measured 2,168, 759, and 2,941 respectively: -7.0 percent surface,
-1.5 percent cold, and -22.5 percent warm. This deliberately branch-heavy
site remains inside the 25-percent investigation gate. Columns outside a
64-block major-river corridor return before hydraulic work; the two-step
intersection solve runs only after cheap side, distance, alignment, selector,
and height gates identify a plausible landmark.

The matching revision-11 movement run used chunk `(183,-177)` and the same
RD10, eight-chunk-radius, 32-block/s, 3,600-frame/60 Hz shape. It measured
5.671 ms average offscreen work, 9.398 ms p95, 11.679 ms p99, and 16.953 ms
max. One frame exceeded 16.667 ms and none exceeded 2x. The accelerated route
ended with a 19-job, six-publication, and 114-render-chunk tail while still
moving. Scheduled-fluid depth and due, executed, deferred, mutation, snapshot,
and event totals stayed zero.

Quality-versus-speed controls divide into two categories:

1. **Output-neutral runtime controls** may change without changing a world:
   generation distance, job budget, worker count, cache size, pre-generation,
   batching, and render/LOD quality.
2. **Output-changing generation quality** changes terrain or blocks and must
   be a stored world-creation descriptor fact. A world cannot switch between
   fast and high-quality terrain as chunks load without creating seams or
   cross-host disagreement.

A future stored choice could offer `Fast`, `Standard`, and `High` recipes, but
the initial campaign lands and tunes one canonical `Standard` path. Prefer
keeping macro river topology and essential terrain organization common while
varying detail octaves, secondary bank treatment, decoration density, or
offline authoring work. Do not add the selector until profiling demonstrates
a real product need and each mode has deterministic maps, performance
receipts, worker equivalence, persistence coverage, and an explicit migration
policy.

## Pipeline And Ownership

```text
stored mclone-overworld-v1 descriptor
  -> profile-owned seed domains and sampler session
  -> macro terrain fields
  -> terrain intent and biome choice
  -> base chunk blocks and biome payload
  -> profile-owned surface recipes
  -> later profile-owned carvers
  -> profile-owned decoration recipes on shared feature execution
  -> later landmark/structure metadata and placement
  -> shared lighting, publication, persistence, and runtime mutation
```

The generator declares outputs, backend work, and typed prerequisites through
the existing `ChunkGenerationPlan`. The scheduler continues to own readiness,
priority, admission, batching, worker capacity, lighting, persistence,
publication, and unloads. Terrain semantics must not leak back into scheduler
matches or app/platform code.

The profile implementation remains concrete. Closed profile dispatch is
sufficient; this direction does not require dynamic plugins, a worldgen node
graph, Mojang's registry/codec framework, or per-sample trait-object dispatch.

## Structured Sampling Contract

The useful structured request/response seam lives inside worldgen rather than
across another worker round trip. A pure sample should make macro intent
inspectable before it becomes blocks. The initial shape is conceptually:

```rust
struct MacroTerrainSample {
    continentalness: f32,
    ruggedness: f32,
    ridges: f32,
    temperature: f32,
    moisture: f32,
    river: f32,
    base_height: f32,
}
```

Only fields used by a landed slice belong in the live type. The conceptual
shape records the intended separation, not a requirement to add unused zeroes
or reserve an abstract schema prematurely.

Sampling must be:

- a pure function of the profile identity, signed seed, stable seed domain,
  dimension, and absolute coordinate;
- identical across native threads and browser Workers;
- independent of chunk request order, batching, neighboring cache residency,
  and random draws in unrelated subsystems;
- available in single-coordinate and bounded-region forms so generation,
  deterministic probes, and review maps use the same implementation.

The first terrain may use a 2.5D heightfield, but a single height per column
must not become the universal shared contract. Later cliffs, overhangs, caves,
and geological modifiers need a path to profile-owned 3D density or material
sampling. Small Island may remain columnar if forcing it through a density
pipeline adds no value.

The live request/response type contains `continentalness: f64`, `relief: f64`,
`ruggedness: f64`, normalized `ridges: f64`, `mountain_detail: f64`, and
derived `surface_y: i32`.
Point and bounded row-major region requests call the same production sampler
used by chunk generation. The region form is deliberately an in-process
worldgen inspection seam, not a second scheduler/Worker protocol.

Field revision 2 composes continentalness at 2,048, 1,024, and 512-block
scales and relief at 384, 128, and a low-weight 48-block scale. Review 1 added
the fine relief octave after initial cards exposed concentric contour bands;
it did not add a new public semantic field.

Field revision 3 preserved those raw fields and added broad ruggedness at
1,536 and 512 blocks plus a ridged crest source at 768 and 256 blocks. Review 1
found that the coherent result still took too long to traverse. Field revision
4 therefore keeps broad ruggedness unchanged, halves the ridge source scales
to 384 and 128 blocks, and narrows the lift response around crests. Every scale
still divides the provisional 6,144-block periodic circumference. Strong
interior crests can reach Y=160, while low ridge values within the same region
remain traversable valley floors and ordinary lowland regions retain their
character. No additional field, point-sampling cost, neighborhood search, or
generation dependency footprint is involved.

Human Review 2 found revision 4 too smooth and dominated by broad relief.
Field revision 5 preserves every existing raw domain and adds independent
32- and 8-block mountain-detail domains, weighted `0.55` and `0.45`. Their
combined value is gated by mountain-region and ridge-shoulder strength, so
oceans and ordinary lowlands retain their previous height exactly. At the same
time, the broad ridge lift changes from `4 + 16s + 54s^2` to
`4 + 12s + 38s^2`. This moves variation from broad ramps into secondary peaks,
saddles, shelves, and gullies without changing a generation dependency
footprint. Both new scales divide the provisional 6,144-block periodic
circumference.

Field revision 5 is not accepted. Its two harmonic, axis-aligned
`ValueNoise2d` bands create locally planar interpolation patches; integer
height quantization exposes those patches as repeated diagonal contour steps.
The broad roughness ratios improved while this directional defect remained
unmeasured. Revision 6 should replace these two implementations behind the
same `mountain_detail` semantic value with periodic-ready gradient noise and
independent gentle domain warps derived from existing relief/ruggedness
components. Do not add volumetric density merely to solve a two-dimensional
orientation artifact.

Field revision 6 implements that replacement behind the same semantic value.
A deterministic 16-direction `GradientNoise2d` uses quintic interpolation and
has a periodic constructor that validates a block period divisible by its
lattice scale. The 32- and 8-block domains remain independent, now weighted
`0.70` and `0.30`, and use different gentle two-axis warps derived from the
already-sampled relief-detail, relief-fine, and ruggedness-detail components.
No warp-only field, macro semantic fact, or larger chunk dependency is added.
The mountain gate, reduced broad lift, and exact ocean/ordinary-lowland
exclusion remain unchanged. Tactical 196 subsequently made every live field
periodic and proved canonical seam work without changing the ordinary path.

A separate
`McloneOverworldLandformSample` pairs one raw terrain sample with the exact
four-block slope used by production. Its exposure relation combines accepted
altitude, ridge, and mountain-region facts without adding another seed domain.
Biome selection keeps moderate sheltered uplands wooded but makes strong
valleys, transitional ridge shoulders, steep faces, and strongly exposed high
ground open. Surface selection keeps dry land grassy unless it is at least Y80
and has slope `>=0.80` or exposure `>=0.76`; those columns expose stone. The
existing open and wooded placed-feature tables then make vegetation eligibility
follow the same biome and substrate facts. Decoration revision 3 records the
intentional output change.

`pnpm native:worldgen:fields` now writes all six production-backed raw/height
maps plus derived slope, biome, and surface-recipe maps; receipts include
separate raw and landform timings, slope/exposure ranges and percentiles,
foundation/new-field fingerprints, landform counts, and selected range,
valley, edge, and lowland review sites. The fully warmed card tool writes
rendered review views from those production-selected centers. Each card eye
queries actual loaded terrain and canopy height at its own horizontal
position, requires at least 24 blocks of clearance, and records that clearance
in receipt schema 5. Review cards use render distance 16, the current shared
scene maximum, with 1,225 target chunks fully ready before capture.

## Reuse Boundary

Reuse mechanisms and data contracts; do not accidentally reuse the reference
profile's rule ownership.

| Surface | Direction |
|---|---|
| Profile descriptor, persistence, worker requests, typed prerequisites | reuse unchanged |
| Scheduler, readiness, lighting, publication, unload policy | keep profile-neutral |
| `SeedDomain`, value/Perlin/simplex noise, coordinate math | reuse and extend when a concrete field needs it |
| Chunk buffers, heightmaps, biomes, ticks, snapshots | reuse unchanged |
| Columnar 2.5D biome payload traversal | share `sample_column_biome_payload`; callers retain absolute-coordinate biome rules |
| `FeatureRegion`, configured/placed features, cross-chunk execution | reuse the executor |
| Vanilla feature and surface tables | keep reference-owned; assemble mclone tables separately |
| Vanilla `NoiseSampler` biome depth/scale composition | reference implementation, not the mclone terrain foundation |
| Density-cell interpolation and block fill mechanics | extract only if the new caller uses the same mechanism |
| Existing carver tunnel/ravine geometry | candidate later; mclone owns domains and distribution |
| Structure start/reference/piece lifecycle | reuse the model later; mclone owns candidates and content |

The strongest constraint wins when a shared primitive also affects the
reference-locked `overworld`. Any extraction from that path requires exact
oracle/output locks. New mclone rule data never enters vanilla fixtures.

The first post-terrain checkpoint extracted only the columnar biome payload
traversal, whose concrete consumers are Small Island and Mclone Overworld. It
preserves the original sampling count, order, and allocation shape.
Shared-looking biome thresholds, material writers, octave composition,
interpolation, and spawn searches remain concrete profile policy until
another real caller proves a smaller mechanism boundary. Reference Overworld
keeps its distinct three-dimensional biome source and Java-owned
surface/ordering path.

The terrain language classifies ocean, beach, open land, and wooded upland,
then selects separate gravel-floor, sand-beach, grass-soil, and exposed-stone
recipes. Its second pass makes open versus wooded land and grass versus stone
react to the shared landform sample, preserving open valleys and ridge
shoulders while keeping the lowland control wooded. Decoration uses an
independent Mclone seed domain and profile-owned oak/grass/flower tables
through the existing placed features, `FeatureRegion`, and ordered executor.
Its plan declares a 3-by-3 feature work band and 5-by-5 Surface prerequisite
band. The earlier checkpoint extracted the identical
seed/reset/input/reuse/retention lifecycle shared by Mclone and Small Island
into `SurfaceDependencyCache`. Their surface generators, feature tables,
biome assembly, target post-processing, public reports, and spawn rules remain
concrete.

The mountain checkpoint deliberately did not extract a generic slope trait,
exposure formula, surface-rule DSL, or mountain-biome classifier. Small Island
has no slope caller, and reference Overworld owns different density,
three-dimensional biome, and surface-builder semantics. The profile-owned
landform value is the smallest real two-consumer boundary; the private halo is
an optimization of its one chunk caller.

Reference Overworld deliberately retains its existing cache. It reuses a
heavyweight generator and biome source, creates liquid-carved inputs, records
phase timing, writes three-dimensional biome payloads, and has different
empty-request retention. A common wrapper with timing and lifecycle switches
would hide those differences. Surface recipes and spawn searches likewise
remain profile-owned; their similarities are vocabulary, not one rule.

The foundation profile and seed descriptor now have direct closeout evidence
through shared catalog creation/display, native descriptor sessions, the
production browser Worker, SQLite and IndexedDB reopen, concurrent independent
dimensions, transient dedicated authority, remote clients, and retained-world
mono/stereo replacement. Persistence metadata is restored before a true miss
is scheduled, and remote clients consume chunks without selecting or running
the server generator. These are shared host contracts, not new terrain
abstractions.

## Module Direction

Start the new rules together without first reorganizing every existing
worldgen file:

```text
native/crates/mclone-worldgen/src/
  noise.rs
  feature/
  levelgen/
    chunk.rs
    planning.rs
    mclone_overworld/
      mod.rs
      fields.rs
      terrain.rs
      biomes.rs
      surface.rs
      decoration.rs
      feature_batch.rs
```

`noise.rs`, `feature/`, `chunk.rs`, and `planning.rs` remain shared mechanism
owners. `mclone_overworld/` owns all original rule composition. Split Small
Island out of `levelgen/profile.rs` when a material change makes that split
useful; do not begin with a mechanical namespace migration.

If a later full Overworld landmark needs the bounded island formula, extract a
pure island landform sampler or a concrete terrain-modifier specification. Do
not invoke the complete `SmallIslandV1` profile inside another profile: a
profile owns complete missing-chunk behavior and is not a composable landmark.

## Numbered Terrain Development Sequence

Develop recognizable vertical terrain families instead of completing every
subsystem horizontally.

1. **Continuous terrain foundation**
   - Independent low-frequency land/ocean and broad-relief fields.
   - Oceans, coasts, lowlands, and rolling uplands across an unbounded world.
   - Minimal existing biome/material palette and safe spawn.
2. **Mountains and valleys**
   - Independent ridge and ruggedness/erosion composition.
   - Terrain creates relief; biome choice reacts to altitude, climate, and
     exposure rather than being the sole source of geometry.
3. **Periodic production-field contract**
   - Route every accepted Mclone terrain, biome, surface, spawn, and decoration
     caller through explicit periodic sampling and coherent work lifts.
   - This is an enabling terrain phase rather than a new visual family; it
     prevents rivers and later fields from accumulating hidden planar-only
     assumptions.
   - Completed 2026-07-22 for the plane and exact 384-chunk X cylinder.
4. **Rivers and wetlands**
   - Deterministic river influence applied before surface recipes.
   - Human Review 1 rejected field revision 7's sloped/uncontained water.
     Human Review 2 rejected revision 8's one-level language, then interactive
     review rejected revision 10's globally non-monotonic stepped corridor.
     Revision 9 bathymetry and revision 11's flat major rivers plus bounded
     source/tributary/fall/sink landmarks now pass closure, runtime, pixel,
     movement, and performance evidence.
   - Water surface, width/depth, banks, provisional flow, grade, local source
     pool, upper tributary, drop, receiving pool, and wetland facts are live.
     General highland reaches, confluences, discharge, globally monotonic
     drainage, and macro headwater/outlet identity remain deferred.
5. **Streams, cascades, and waterfall reaches**
   - The first short source-to-river tributary is live as a bounded landmark.
     Generalize it only after interactive human review, and retain explicit
     upstream and downstream destinations for every raised-water variant.
   - Classify calm, riffle, cascade, fall, and later mill-compatible reaches;
     never place an isolated falling-water decoration without a watercourse.
   - Keep sound, mist, splash particles, animation, and functional machinery
     as separate presentation or gameplay slices.
6. **Biome, surface, and decoration language**
   - Temperature/moisture/altitude combinations and original regional
     recipes.
   - Reuse configured feature implementations while owning selection,
     density, and seed domains.
   - Field revision 14 now supplies periodic temperature/moisture,
     altitude-adjusted snow, conifer, alpine, and core/shoulder steppe
     recipes. Tactical 223 is implementation-complete and awaiting Human
     Review 2 of the steppe-scale correction.
7. **Three-dimensional geology and rock formations**
   - Give boulders, scree, tors, hoodoos, sea stacks, arches, cliff shelves,
     undercuts, and overhangs sustained shape and pixel iteration.
   - Compare placed features, bounded structure-shaped volumes, and selective
     regional density modifiers before choosing one universal mechanism.
   - Use cheap macro selectors, bounded 3D influence, coarse lattice
     interpolation, and explicit far-LOD silhouettes so ordinary chunks do
     not pay dense 3D-noise cost.
8. **Caves and subsurface geology**
   - Add independently seeded 3D subtractive fields or carvers after the first
     surface-rock density experiments.
   - Reuse geometric helpers only after a concrete Mclone cave rule proves the
     shared shape; do not make cave prevalence dictate surface outcrops.
9. **Landmarks and structures**
   - Keep bounded local content in placed features when honest.
   - Add true starts, references, pieces, bounding boxes, and persistence when
     the first cross-chunk landmark requires them.

Each stage may add a new profile version after release freeze. While the
profile remains internal-mutable, fixtures are
determinism/regression evidence rather than user-save compatibility promises.

## Reuse And Refactoring Review Protocol

Every implementation tactical must reserve explicit review stages. Maximum
reuse does not mean extracting before a caller exists; it means creating time
to compare real callers before the next content layer hides the decision.

### Review A: pre-slice inventory

- State the player-visible terrain rule being added.
- Identify existing reference Overworld and Small Island mechanisms that
  appear relevant.
- Mark each candidate as reuse-as-is, reuse-after-output-locked extraction, or
  intentionally profile-owned.
- Record affected safety-ledger rows and exact non-regression evidence.

### Review B: first concrete caller

- Implement the smallest profile-owned rule through existing generic runtime
  contracts.
- Do not generalize merely to make the first implementation look abstract.
- Capture deterministic field/block facts and first drawable pixels before
  adding the next content family.

### Review C: dedicated reuse/refactor checkpoint

- Compare the working mclone code with Small Island and reference Overworld.
- Extract only behavior with two concrete consumers or a clear frozen
  mechanism boundary.
- Keep rule tables, seed domains, and compatibility fixtures profile-owned.
- Land behavior-preserving refactors separately from intentional output
  changes whenever practical.
- Re-run exact vanilla locks and Small Island seam/order fingerprints for any
  shared change.

### Review D: visual and distribution review

- Inspect multiple positive and negative seeds, more than one region center,
  and boundary coordinates.
- Review both rendered landscapes and field/distribution maps.
- Measure land fraction, height percentiles, slopes, shoreline, and biome
  proportions where they express the intended rule.
- Treat screenshots as design evidence, not the determinism oracle.

### Review E: slice closeout

- Confirm no scheduler or platform policy fork was introduced.
- Record native/browser Worker equivalence and relevant persistence evidence.
- Update this topic, the profile safety ledger, and the tactical result.
- Decide explicitly whether the next slice is content, refactoring, tooling,
  or blocked on a new shared contract.

## Review Tooling Direction

The Small Island review card proves the value of fixed, receipt-backed visual
comparisons. The original Overworld will also need:

- low-resolution regional maps for each landed macro field and derived height;
- rendered landscape views at explicit seed/center/camera coordinates;
- receipts containing profile, seed domains/revision, region, field ranges,
  terrain distributions, readiness, and camera facts;
- output under `/tmp`, with selected numeric fingerprints committed separately
  from disposable screenshots.

Review tooling must sample the same pure field implementation as generation.
Do not create a debug-only approximation of the terrain formula.

`pnpm native:worldgen:terrain-characteristics` is the scale-sensitive
counterpart to the field maps and visual cards. It converts actual generated
chunks into a masked one-block height raster, then runs the same analysis on
reference and candidate terrain. Its default 17-by-17-chunk sites compare
three exact Java 1.17.1 mountain anchors with three reviewed Mclone mountain
centers before carvers or decoration. Top-solid land at Y>=67 is the shared
mask; material and vegetation cannot affect the result.

Do not reduce this comparison to average slope. The durable characteristic
vocabulary is:

- a height-difference structure curve over 1-128-block separations;
- five-point curvature for block-scale direction changes;
- local plane-fit residual roughness over radii 2-32, which removes smooth
  grade before measuring bumps and secondary forms;
- local gradient structure-tensor coherence and diagonal alignment over the
  same radii;
- the 1-32-block log/log roughness exponent; and
- fine-detail residual-energy share between radius-4 and radius-32 fits.

The first mountain benchmark quantified Human Review 2: relative to the
vanilla group medians, Mclone has 0.408x lag-1 and 0.422x lag-4 RMS height
change, 0.158x radius-8 and 0.220x radius-16 detrended roughness, but 1.604x
lag-64 height change. Its roughness exponent is 0.919 versus 0.659. This is a
smooth broad-ramp signature, not a lack of total mountain height. The current
JSON receipt remains disposable under `/tmp`; deterministic analyzer tests
and generated-terrain fingerprints make the procedure reproducible. Expand
the site corpus before treating the values as a universal Minecraft biome
distribution.

The analyzer uses summed-area tables and takes about 14-27 ms for a
272-by-272 site in the initial release run. It is offline tooling and adds no
production generation cost. Keep future characteristic extraction in shared
Rust so the same code can later serve a Terrain Lab or automated visual-review
report without moving terrain semantics into TypeScript.

Field revision 5 uses that benchmark as a directional constraint rather than
an optimization target. Radius-8 detrended roughness moved from 0.158x to
0.711x vanilla, radius-16 from 0.220x to 0.625x, and lag-64 height change from
1.604x to 1.194x. The roughness exponent fell from 0.919 to 0.748 against the
0.659 reference median; fine-detail energy share is 0.063 versus 0.060. The
lowland control remains fingerprint-identical. This is sufficient for a new
human pixel review without forcing Mclone to copy vanilla's exact terrain
distribution.

The first field-revision-5 card review exposed a measurement blind spot:
global X/Z anisotropy is about 1.06-1.26 across the mountain sites even though
local diagonal herringbone is visually dominant. Opposing `/` and `\` patches
cancel in a global axis ratio. The analyzer now reports windowed gradient
structure-tensor coherence and diagonal alignment. It improves the vocabulary
but does not automatically detect the rejected defect: revision 5's mean
diagonal values are below the vanilla medians at radii 4 and 8. The missing
fact is closer to repeated contour straightness or lattice spectral energy.
Roughness and orientation aggregates remain supporting constraints, while
inspected pixels retain veto authority.

Field revision 6 restores a more useful visual/numerical balance without
fitting vanilla output. Its mountain medians are `0.518x` and `0.557x` vanilla
for lag-1 and lag-4 changes, `0.523x` and `0.527x` for radius-8 and radius-16
detrended roughness, `1.192x` for lag-64 change, and `0.915x` for fine-detail
energy share. The first warped-gradient weighting overshot the latter at
`1.977x`; shifting the 32/8 weights from `0.55/0.45` to `0.70/0.30` removed
uniform fine bustle while retaining varied local peaks. The exact lowland
control is unchanged. The full RD16 matrix removes the repeated chevrons, and
Human Review 3 accepted the result as more natural and less geometric.

## Acceptance Themes

- distinct profile identity without any reference Overworld output change;
- absolute-coordinate seams and deterministic negative-coordinate behavior;
- request-order, batch, partition, native-thread, and Web Worker equivalence;
- explicit seed-domain isolation between terrain, biome placement, surface,
  decoration, caves, rivers, and structures;
- useful safe spawn in the generated terrain;
- no app-local or TypeScript terrain rules;
- reusable code proven by concrete callers, not framework breadth;
- inspected visual and measured distribution evidence before each new terrain
  family.

## Next Work

Field revision 12 owns the first complete bounded valley stream: reusable
procedural starts and references, a 48-96-block monotonic plan, clipped
continuous valley terrain, fixed-point water stencils, and matching synthetic
LOD. Full-route closure, all-water authoritative wake, deterministic
partition/periodic fixtures, SQLite reopen, same-host performance comparison,
one-minute movement soak, and internally inspected production pixels pass.
Human Review 1 accepted the modest headwater, calm reaches, carved shoulders,
and confluence as a peaceful spring-fed creek. Tactical 222 is complete.
Future variation should preserve that quiet scale as one landmark family
while adding deliberately dramatic cascade, gorge, and waterfall families.
Tactical 225 now owns the objectively demonstrated river-to-shelf sill plus
the associated smooth-channel, binary-surface, and sparse-local-feature
correction. One completed review must show an outlet whose centerline floor
does not rise into the receiving ocean, coherent width/depth and bend
variation, mixed grass/dirt/gravel/stone transitions, and sparse rocks without
fluid work or loss of the accepted peaceful stream. General drainage-network
semantics remain deliberately absent. The sustained volumetric geology
campaign follows this bounded hydrology correction.

## Related

- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- [`jjthunder-to-the-max-reference.md`](jjthunder-to-the-max-reference.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../structures.md`](../structures.md)
- [`../tactical/187-generator-profile-flat-grass-and-seeded-island.md`](../tactical/187-generator-profile-flat-grass-and-seeded-island.md)
- [`../tactical/191-guarded-generation-planning-refactor.md`](../tactical/191-guarded-generation-planning-refactor.md)
- [`../tactical/192-mclone-overworld-mountains-and-valleys.md`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
- [`../tactical/196-periodic-mclone-terrain-fields.md`](../tactical/196-periodic-mclone-terrain-fields.md)
