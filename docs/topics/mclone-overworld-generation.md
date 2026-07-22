# Mclone Overworld Generation

Topic: `mclone-overworld-generation`

Status: the first continuous-terrain caller, two visual/distribution reviews,
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
that finding against exact undecorated Minecraft Java 1.17.1 terrain; a bounded
scale-composition tune remains before host-equivalence closeout. The shared
Flat Grass cylinder proof is complete;
[`196`](../tactical/196-periodic-mclone-terrain-fields.md) is planned after the
first accepted Tactical 192 field set and before rivers, climate breadth, or
structures. The selected terrain sequence is mountains and valleys, periodic
production fields, rivers and wetlands, then coherent streams, cascades, and
waterfall reaches.

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
targets. The study does not change the current tactical order: mountains and
valleys remain next, with rivers and caves deferred to their own slices.

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

Rivers and wetlands follow Tactical 196 so their fields are periodic and
seam-correct from their first production revision. Water is a macro terrain
input, not a biome decal or a post-surface trench. The eventual shared sample
may expose these facts as each gains a real caller:

- channel distance or influence;
- stable water-surface and bed elevation;
- half-width and depth;
- downstream direction and grade;
- network and reach identity;
- headwater, tributary, main-stem, wetland, confluence, outlet, and later
  stream-order or discharge classification.

The first version may use deterministic bounded corridor planning rather than
a scientific rainfall simulation. Terrain carves from the same facts that
biomes, bank materials, wetlands, structures, and review tools query. No
column sampler may trace arbitrarily far upstream, run an unbounded flood fill,
or search until it finds an ocean. Coarse network construction belongs in
canonical macro tiles with an explicit finite halo and a descriptor-keyed
cache; point queries consume bounded reach facts.

Reach grade later distinguishes calm water, riffles, rapids, cascades, falls,
and plunge pools. A waterfall must have continuous upstream and downstream
watercourse facts. Low-gradient reaches may meander or support floodplains and
wetlands; steep constrained reaches may form gorges. Not every valley receives
a river, and small configured ponds and springs remain useful local accents.

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
`ruggedness: f64`, normalized `ridges: f64`, and derived `surface_y: i32`.
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

The raw field inventory remains revision 4. A separate
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

`pnpm native:worldgen:fields` now writes all five production-backed raw/height
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
4. **Rivers and wetlands**
   - Deterministic river influence applied before surface recipes.
   - Begin with an inspectable field; true rainfall/flow accumulation can be a
     later refinement.
   - Expose water surface, width/depth, banks, downstream direction,
     continuity, wetland, headwater, confluence, and outlet facts through the
     production sampler as each fact gains a real caller.
5. **Streams, cascades, and waterfall reaches**
   - Realize continuous watercourses against the accepted relief and river
     facts, with stable upstream and downstream destinations.
   - Classify calm, riffle, cascade, fall, and later mill-compatible reaches;
     never place an isolated falling-water decoration without a watercourse.
   - Keep sound, mist, splash particles, animation, and functional machinery
     as separate presentation or gameplay slices.
6. **Biome, surface, and decoration language**
   - Temperature/moisture/altitude combinations and original regional
     recipes.
   - Reuse configured feature implementations while owning selection,
     density, and seed domains.
7. **Caves and geology**
   - Independently seeded 3D subtractive fields or carvers.
   - Reuse geometric helpers only after the first concrete mclone cave rule
     proves the shared shape.
8. **Landmarks and structures**
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

Tune Tactical 192's mountain scale composition against the new multiscale
terrain-characteristics baseline, then repeat its production maps and
maximum-view-distance landscape matrix. The present result needs more
coherent 4-32-block structure and less dominance from broad 64-block change;
do not merely increase global height or add unmodulated noise everywhere. Once
human review accepts that bounded tune, complete native/browser Worker
equivalence and unchanged-host-contract closeout. Do not fold rivers, climate
breadth, caves, or structures into that tactical. After its field set and
terrain language are accepted, execute
[`Tactical 196`](../tactical/196-periodic-mclone-terrain-fields.md):
re-audit every live field scale, select the explicit periodic sampler and
circumference, then route terrain and decoration through canonical outputs plus
coherent seam work lifts. Do not add rivers or climate breadth before that
contract is demonstrated through the production browser Worker and persistence
paths. After Tactical 196, prefer a bounded rivers-and-wetlands tactical over
caves, structures, or broad biome expansion. Its first result should be one
inspectable production river influence with coherent water level, banks,
downstream direction, and wetland/headwater/outlet facts shared by terrain and
biome/surface selection. Follow that with a separate stream/cascade/waterfall
reach slice; waterfall placement must consume continuous watercourse and grade
facts rather than decorate arbitrary cliffs.

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
