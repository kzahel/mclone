# Tactical 220: Mclone Overworld Rivers And Wetlands

Status: Human Review 1 rejected the field-revision-7 hydraulic surface on
2026-07-23. Field revision 8 replaced it with hydraulically safe flat,
sea-level lowland water and passed its objective gates, but Human Review 2
rejected the resulting world language. Rivers disappear wherever the lowland
gate ends, no screenshot contains a water-level change, and large bodies lack
the shelf, deep-basin, and seabed-relief structure visible in vanilla. The
next review requires independent ocean bathymetry plus multiple flat river
reaches connected by bounded baked drops.

Topic: `mclone-overworld-generation`

Workstream: shared native Rust terrain fields, surface and biome language,
production review maps, deterministic generation, and bounded performance.

## Result

Add the first continuous Mclone watercourse as a terrain input rather than a
river-biome decal or post-surface trench. One production sample must give
terrain, biome, surface, spawn, and review callers the same bounded facts for:

- distance to a broad river centerline;
- channel and graded-bank influence;
- channel half-width and bed depth;
- a stable local water surface;
- centerline tangent and provisional flow orientation; and
- low-gradient floodplain/wetland influence.

The first drawable result should contain legible winding rivers with water,
graded banks, open river biome, and occasional soft wetland margins. It must
work on the ordinary plane and meet exactly on the existing 6,144-block X
period. This tactical does not claim a scientific drainage simulation,
discharge accumulation, confluences, named reaches, navigable flow states,
streams, waterfalls, mill sites, ponds, caves, or structures.

## Minecraft Java 1.17.1 Reference Read

The required local reference sources were read on 2026-07-22:

- `RiverInitLayer` gives non-ocean cells stable random values;
- `RiverLayer` compares the parity-filtered center with its four neighbors and
  emits river biome `7` at boundaries;
- `RiverMixerLayer` overlays that river biome on non-ocean biome output, with
  frozen-river and mushroom-shore exceptions;
- `Layers.getDefaultLayer` zooms and smooths the independent river layer
  before mixing it with the biome layer;
- `LakeFeature` unions four to seven randomized ellipsoids in one 16x8x16
  cavity, validates its solid/liquid boundary, fills the lower half, opens the
  upper half, and performs local surface/freeze repair; and
- `SpringFeature` places a liquid source only when a configured count of the
  five adjacent positions is rock and another configured count is empty.
- the disabled `Aquifer` used by the 1.17.1 target fills default source water
  only below one global sea level and reports
  `shouldScheduleFluidUpdate() == false`; and
- `NoiseBasedChunkGenerator` schedules generated fluid only when the selected
  aquifer explicitly requests it, while springs and exposed underwater-carver
  cells schedule local ticks.

The reusable lessons are pipeline separation, deterministic domains, a
smoothed independent river signal, and local validation before liquid writes.
The deliberate divergence is geometry ownership: vanilla's layer supplies a
continuous river-shaped biome mask but does not make a profile-owned channel
elevation contract. Mclone needs one watercourse sample to shape terrain and
then drive biome, materials, vegetation exclusion, wetlands, and later
structure siting. Vanilla lakes and springs remain good later local features;
they are not substitutes for that macro fact.

This work does not port the disabled 1.17.1 aquifer or Caves & Cliffs density
paths.

Vanilla therefore does not pre-settle ordinary rivers. It avoids the problem:
the river layer is principally biome language, ordinary Overworld water is one
flat sea-level source body, and only bounded exposed liquid features request
runtime work.

## First Field Decision

Begin with an inspectable warped zero-contour corridor, not a global drainage
graph. A low-frequency periodic gradient field supplies continuous centerlines.
Independent periodic width and gentle warp fields prevent uniform canals and
axis-aligned contours. An analytic gradient-noise derivative converts field
magnitude and gradient into an approximate distance in blocks without four
extra field samples per column.

This is intentionally a first visual watercourse family:

- it is point-queryable in fixed work and has no upstream search or global
  mutable graph;
- every X scale divides 6,144, so the channel and its first derivative meet at
  the cylinder seam;
- it can cross and carve accepted terrain without changing the reference
  Overworld or other Mclone field domains; and
- it does not pretend generic zero contours have true tributary/confluence
  topology. Add network/reach identity only with a concrete caller and a
  canonical macro-tile planner.

Water level comes from a smooth profile-owned hydraulic height related to the
existing broad land fields, transitions toward sea level at coasts, and is
clamped below ordinary nearby terrain. The channel lowers the base surface to
its bed; graded banks lower terrain toward the water edge. Low local grade and
low relief may widen the graded floodplain. Do not raise distant terrain or
flood arbitrary depressions merely to preserve a line.

The field revision and decoration revision must change explicitly. Preserve a
separate `base_surface_y` in the production terrain sample so review tools and
future refinements can distinguish accepted natural relief from river carving.

## Execution Plan

### Slice 0: frozen boundary and first field

- [x] Read the exact vanilla river-layer, lake, spring, and pipeline sources.
- [x] Select the bounded corridor model and record why it does not yet claim a
  drainage network.
- [x] Add independent river domains/scales through both sampling topologies.
- [x] Expose base surface, distance, influence, width, bed, water, tangent,
  grade, and wetland facts from one production sampler.
- [x] Prove point/region identity, signed coordinates, exact periodic seam and
  slope continuity, bounded values, and deterministic controls.

Gate: a review map shows continuous, varied corridors without any block writes.

Execution record 2026-07-22:

- the broad/detail/width/pool scales are `768`, `192`, `384`, and `96` blocks;
  each divides the existing 6,144-block circumference. All four use new stable
  domains and explicit plane/periodic constructors;
- the production terrain sample retains `base_surface_y` and adds channel
  distance/influence, bank influence, half-width, bed, water surface, tangent,
  locally oriented provisional flow, grade, floodplain strength, and shallow
  pool influence;
- `GradientNoise2d::sample_with_derivative` returns analytic world-block X/Z
  derivatives. Centered-difference and periodic-derivative tests lock the
  shared primitive; and
- point/region, signed-lift, field/seam-slope, chunk seam, feature partition,
  canonical lift, bounded-range, channel/wetland presence, and existing
  foundation fingerprints pass. The reference Overworld and other profiles
  do not consume any new rule.

### Slice 1: terrain and language

- [x] Carve the channel and grade banks before surface material selection.
- [x] Fill river water to the sampled local water surface without changing
  ordinary ocean fill.
- [x] Add river biome and riverbed/bank surface language; keep trees and land
  patches out of channel water through the ordinary biome/substrate rules.
- [x] Make spawn continue to choose a dry safe column from final terrain facts.
- [x] Lock adjacent chunks, seam chunks, order/partition, canonical lifts, and
  current feature dependency behavior.

Gate: packed generated chunks contain one continuous river rather than a
debug-only visualization.

Execution record 2026-07-22:

- the channel lowers accepted natural terrain to a two-to-four-block bed and
  fills source water to its local hydraulic surface. Banks blend from the
  water edge back into the original column rather than using a fixed terrace;
- channel and shallow-pool columns use river biome `7`; riverbeds use gravel,
  sparse wetland pools use clay, inland banks remain grassy, and coastal banks
  inherit sand instead of drawing green levees through beaches;
- low-grade, low-mountain, inland floodplains can widen and receive sparse
  shallow pools. A coarse-dirt marsh-bank experiment was inspected and removed
  because its parallel brown bands looked more engineered than natural; and
- spawn rejects channel and pool water. Current vegetation avoids actual
  water through ordinary biome/substrate rules. Rich wetland plants and a
  finer per-column vegetation exclusion are review-driven follow-ups, not
  silently claimed here.

### Slice 2: maps, tuning, and performance

- [x] Extend the production review command with watercourse maps, ranges,
  percentiles, coverage, and selected river/wetland review sites.
- [x] Compare several positive and negative seeds at lowlands, mountains,
  coasts, and the periodic seam.
- [x] Capture fully warmed render-distance-16 top-down, landscape, and elevated
  cards from above terrain.
- [x] Measure field-only, surface, cold decorated, and warm decorated cost
  against Tactical 196's plane baseline.
- [x] Tune spacing, meander, width, bank grade, level, bed, and wetland response
  until remaining disagreement is subjective.

Gate: present the best multi-seed pixels and map evidence for human judgment.

Execution record 2026-07-22:

- receipt schema 7 maps base versus carved terrain, channel distance and
  influence, local water level, grade, wetland/pool response, biome and surface
  language, coverage, and selected river, mountain, coast, wetland, pool, and
  periodic-seam sites from the production sampler;
- inspected RD16 cards include ordinary lowland river
  `/tmp/mclone-river-card-second/mclone-overworld-v1-seed-neg98765-chunk-neg183-neg30-card.png`,
  coast/floodplain
  `/tmp/mclone-wetland-card-second/mclone-overworld-v1-seed-neg98765-chunk-neg25-72-card.png`,
  mountain river
  `/tmp/mclone-mountain-river-card/mclone-overworld-v1-seed-neg98765-chunk-neg190-21-card.png`,
  sand-bank coast
  `/tmp/mclone-coastal-river-card/mclone-overworld-v1-seed-12345-chunk-141-100-card.png`,
  and the actual cylinder seam
  `/tmp/mclone-periodic-river-seam-card/mclone-overworld-v1-seed-neg98765-chunk-0-neg100-card.png`;
- the strongest broad map evidence is
  `/tmp/mclone-river-review-neg98765/mclone-overworld-v1-plane-seed--98765-chunk--96-72-watercourses.png`;
  field maps for seeds `12345` and `8675309` were also inspected; and
- at seed `-98765`, radius three, and three release iterations, the final plane
  measured 2,911.666 surface chunks/s, 986.434 cold decorated targets/s, and
  5,339.303 warm targets/s. The cylinder measured 2,777.558, 896.640, and
  4,777.684 respectively. Against Tactical 196's plane result, the decreases
  are 13.8, 12.9, and 12.2 percent. They remain below the 25-percent gate.
  Analytic derivatives replaced an initial finite-difference version whose
  isolated surface path was about 42 percent slower than the baseline.

Human Review 1 now owns the remaining questions. The strongest current result
is the mountain/valley integration and seamless broad meander. The known
architectural limitation is equally visible in the maps: zero contours may
close into loops and do not produce tributary, confluence, discharge, or true
downstream network identity. Sparse pools make the wetland fact physical, but
wetland vegetation and material character remain deliberately minimal.

Human Review 1 result, 2026-07-23: **rejected**. Interactive inspection found
two correctness failures hidden by the elevated cards:

- `water_surface_y` follows broad terrain independently at every column, so
  the water plane can slope across the channel rather than only descend along
  a real flow transition; and
- bank grading only lowers terrain. A channel source may therefore border air
  above a lower natural enclosure, appearing to float until a block mutation
  schedules it and the ordinary fluid simulation spills it outward.

These are not material/charm defects. The initial generated water is not a
fixed point of the runtime fluid rules, so revision 7 cannot proceed to
platform closeout.

### Corrective Slice 2B: flat contained reach foundation

- [x] Make ordinary generated river water one constant integer level within
  every implemented reach; begin conservatively with naturally contained
  lowland/sea-level reaches rather than inventing an unproven reach graph.
- [x] Remove physical highland/wetland water wherever the generator cannot
  establish bounded containment. Retain dry field facts only when useful.
- [x] Give each wet column a solid bottom and require every horizontal source
  boundary at its water level to meet source water or solid containment.
- [x] Use no positive bank repair in revision 8. Existing terrain may be
  lowered into the global sea-level body, but ordinary generation does not
  raise levees to rescue an invalid high reach.
- [x] Add a production hydraulic-closure diagnostic over generated regions.
  The ordinary flat-reach result must emit no initial fluid ticks and waking
  exposed source boundaries in a copy must produce no mutation.
- [x] Re-run multi-seed maps and RD16 pixels before adding rapids or falls.

Gate: no transverse/sloped ordinary water, no floating source faces, no
generated liquid ticks, and no hidden fluid mutation in the reviewed flat
reach regions.

Execution record 2026-07-23:

- field revision 8 and decoration revision 5 set every physical river and
  shallow wetland-pool surface to Y63. Channel realization is limited to
  `broad_surface_y <= 70` and `base_surface_y <= 72`; higher corridors retain
  inspectable dry facts instead of receiving invalid source water;
- the ordinary source body is the same hydrostatic body as global sea fill.
  A boundary column is therefore either source water at Y63 or
  motion-blocking terrain. No positive bank repair, whole-river search,
  simulation, generated liquid tick, or new generation dependency is used;
- `analyze_mclone_overworld_hydraulic_closure` generates a one-chunk halo and
  counts open horizontal source faces, unsupported sources, unequal adjacent
  water tops, and scheduled liquid ticks. Dense river, coastal, wetland-pool,
  and exact periodic-seam radius-one receipts each report `closed=true` with
  zero failures. The inspected nine-chunk regions contain 2,059, 3,145,
  2,927, and 2,409 source blocks respectively;
- an authoritative `mclone-server` test schedules every source in a dense
  generated river chunk, executes the fluid runtime, and requires zero block
  mutation and an empty final queue; and
- broad 6,144-by-6,144-block maps for seeds `12345`, `-98765`, and `8675309`
  were inspected. RD16 cards cover dense river chunk `(47,102)`, coast
  `(13,-4)`, wetland-pool `(-28,-119)`, dry mountain control `(-192,-24)`,
  and exact cylinder seam `(0,-190)` for seed `-98765`. Water is level and
  visibly supported in all four wet cards; the highland control remains dry.

### Corrective Slice 2C: performance and sustained movement

- [x] Re-run the Tactical 196/220 release field, surface, cold-decoration, and
  warm-decoration comparison on the same host and parameters.
- [x] Run the canonical native movement-frame route for 3,600 frames at
  60 Hz, representing 60 seconds of movement with live fluid simulation.
  Record that this harness advances target-Hz movement without wall-clock
  sleeping, so it is an accelerated movement soak rather than a paced
  end-user session.
- [x] Record p50/p95/p99/max frame work, generation/publication/mesh queues,
  loaded chunks, fluid tick time, executed/deferred ticks, mutations, and
  final/max scheduled-fluid depth.
- [x] Require zero generated-river fluid work throughout an untouched walking
  soak. Any nonzero work must be attributed to another known generated feature
  or blocks the reach result.

Gate: warm generation remains reasonably close to the accepted baseline, work
queues drain/plateau under movement, and passive rivers cause no fluid-tick
tail.

Execution record 2026-07-23:

- the seed `-98765`, radius-three, three-iteration release benchmark at clean
  commit `2feb9189` measured:

  | Topology | Mclone surface | Cold decorated | Warm decorated |
  |---|---:|---:|---:|
  | plane | 2,857.849 chunks/s | 904.243 targets/s | 5,053.663 targets/s |
  | cylinder-x:384 | 2,571.381 chunks/s | 868.850 targets/s | 4,352.229 targets/s |

  The plane result is 15.4, 20.2, and 16.9 percent below Tactical 196's
  accepted plane baseline, respectively. All remain inside the 25-percent
  investigation gate. The warm pass served all 363 dependency requests from
  cache and generated none;
- the clean release movement probe used seed `-98765`, river chunk `(47,102)`,
  render distance 10, an eight-chunk-radius route, 32 blocks/s, derived
  7-worker/20-pending render capacity, and 3,600 frames at 60 Hz. The route
  visited 64 unique chunk centers and repeated the same river-heavy circuit,
  exercising cold then warm streaming;
- the accelerated 60-second route measured 3.358 ms average offscreen frame
  work, 1.567 ms accounted frame-wall p50, 7.533 ms p95, 10.587 ms p99, and
  13.108 ms max. No frame exceeded the 16.667 ms budget and frame accounting
  reported no conservation violation;
- loaded chunks remained between 529 and 576. Worldgen jobs peaked at one and
  ended at zero; publications peaked at 14 and ended at zero; render compile
  jobs peaked at five and ended at zero. Pending render chunks peaked at 129
  and ended at 91 while the camera was still moving; and
- scheduled fluid depth was zero at maximum and final state. Due, executed,
  deferred, mutated, snapshot, and fluid-event totals were all zero. The
  largest bookkeeping-only fluid lane sample was 0.023 ms.

### Later bounded waterfall templates

Do not run a whole-river fluid simulation during chunk generation. A later
slice may settle a finite catalogue of width/drop/run/receiving-pool templates
in build or test tooling, bake their source/flowing block stencils, and stamp
only at sites satisfying bounded wall, lip, drop, and receiving-pool
preconditions. Production generation stays fixed-work. Any optional
output-changing offline quality mode must be persisted in the world descriptor
and may not vary by chunk.

### Corrective Slice 2D: ocean bathymetry

- [ ] Add explicit shore/shelf, shelf-break, basin-interior, water-depth, and
  seabed-relief facts to the production sample.
- [ ] Keep the ocean surface hydrostatic at Y63 while making broad water
  interiors materially deeper and more locally varied than coastal shelves.
- [ ] Use the exact periodic production fields and fixed-work point sampling;
  do not add a connected-component search to every column.
- [ ] Add water-depth distributions, maps, representative coast-to-basin
  transects, and RD16 deep-water cards.
- [ ] Re-run surface, cold, and warm generation performance.

Gate: large bodies visibly progress from shallow shore through a shelf break
into a deep, irregular basin without changing the flat water surface or
creating fluid ticks.

Minecraft 1.17.1 supplies the architectural reference. `AddDeepOceanLayer`
promotes a shallow-ocean cell only when all four cardinal neighbors are also
ocean. The resulting deep-ocean biome uses depth `-1.8` rather than `-1.0`;
`NoiseSampler` blends depth/scale over a five-by-five biome neighborhood and
then combines that result with three-dimensional blended density noise. Mclone
should preserve the size-aware shelf/interior lesson without porting that
profile's biome-layer or density pipeline.

### Corrective Slice 2E: explicit flat reaches and bounded drops

- [ ] Replace the single global river level with an inspectable reach
  vocabulary: reach level, upstream/downstream relation, headwater, outlet,
  drop height, transition kind, and receiving pool.
- [ ] Keep every ordinary reach surface constant at one integer Y and
  monotonic downstream.
- [ ] End water only at an explicit headwater/source or outlet. Never let a
  height threshold silently erase an otherwise visible corridor.
- [ ] Connect adjacent levels with bounded baked flowing-water stencils whose
  wall, lip, fall, and receiving-pool preconditions are locally checkable.
- [ ] Extend hydraulic review to distinguish quiescent source reaches from
  intentional stable flowing transitions, including a real fluid-runtime wake
  test.
- [ ] Capture at least one RD16 card where two flat levels and their transition
  are visible together, plus a river-to-shelf-to-deep-basin outlet.

Gate: pixels visibly demonstrate level change without a sloped source sheet,
arbitrary disappearance, floating water, or an unbounded generation-time
simulation.

### Slice 3: reuse and platform closeout after corrective human acceptance

- [ ] Compare the working channel writer with vanilla lake/spring liquid and
  local validation mechanisms; extract only a behavior-preserving primitive
  with a second real caller.
- [ ] Run full worldgen/server/workspace locks, native pixel, production browser
  Worker and IndexedDB reopen, Android, and available XR gates.
- [ ] Record payload, cache, storage, and platform evidence and update support
  matrices.
- [ ] Decide whether the accepted next slice is watercourse-network semantics,
  streams/cascades/waterfalls, local ponds, or another terrain correction.

Gate: close only after human acceptance and shared-host evidence.

## Performance Budget

Point sampling remains `O(live field count)` with an analytic gradient
derivative and a fixed number of bounded grade probes. Chunk generation should
batch a bounded halo and must not trace a channel, walk downstream, flood fill,
or acquire a process-global cache. Use the same Tactical 196 release command
and host for comparison.

- more than 25 percent cold decorated-target regression requires focused
  attribution and an explicit tradeoff;
- twofold regression blocks visual acceptance;
- surface-only and field-map cost identify repeated noise work even if feature
  execution hides it in the full target; and
- a later output-changing quality selector remains a persisted descriptor, not
  a per-chunk performance switch.

## Human Review Questions

- Do rivers read as connected waterways instead of trenches, blue contour
  lines, or canals?
- Are spacing, width, turns, banks, and floodplains believable at walking and
  elevated scales?
- Do channels sit naturally in lowlands and mountain valleys without cutting
  implausible level slots across every ridge?
- Are coast transitions and wetland widening charming enough to develop, or
  should the first field be replaced by a macro graph before adding streams?
- Does the result preserve the accepted terrain's varied, non-geometric feel?

## Stop Conditions

Stop and split follow-up work if the first visual result requires a global
drainage solver, arbitrary upstream search, fluid simulation changes, true
confluence/discharge identity, waterfall realization, a generic field graph,
reference Overworld changes, or app/platform terrain policy.

## Related

- [`196-periodic-mclone-terrain-fields.md`](196-periodic-mclone-terrain-fields.md)
- [`192-mclone-overworld-mountains-and-valleys.md`](192-mclone-overworld-mountains-and-valleys.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
