# Tactical 220: Mclone Overworld Rivers And Wetlands

Status: Human Review 1 open 2026-07-22. Slices 0-2 produced field revision 7
and decoration revision 4 on the plane and exact 384-chunk X cylinder. Broad
rivers, graded banks, coast transitions, local river levels, river biomes, and
sparse shallow inland wetland pools are live and inspected. Slice 3 platform
closeout remains intentionally paused until the visual/architecture decision.

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

### Slice 3: reuse and platform closeout after human acceptance

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
