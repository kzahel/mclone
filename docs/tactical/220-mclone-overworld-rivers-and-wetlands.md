# Tactical 220: Mclone Overworld Rivers And Wetlands

Status: active 2026-07-22 after Tactical 196 completed the plane and exact
384-chunk X-periodic terrain contract. Stop the first implementation at a
multi-seed rendered human-review gate before expanding the water vocabulary.

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
axis-aligned contours. A bounded finite difference converts field magnitude
and gradient into an approximate distance in blocks.

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
- [ ] Add independent river domains/scales through both sampling topologies.
- [ ] Expose base surface, distance, influence, width, bed, water, tangent,
  grade, and wetland facts from one production sampler.
- [ ] Prove point/region identity, signed coordinates, exact periodic seam and
  slope continuity, bounded values, and deterministic controls.

Gate: a review map shows continuous, varied corridors without any block writes.

### Slice 1: terrain and language

- [ ] Carve the channel and grade banks before surface material selection.
- [ ] Fill river water to the sampled local water surface without changing
  ordinary ocean fill.
- [ ] Add river biome and riverbed/bank surface language; keep trees and land
  patches out of channel water through the ordinary biome/substrate rules.
- [ ] Make spawn continue to choose a dry safe column from final terrain facts.
- [ ] Lock adjacent chunks, seam chunks, order/partition, canonical lifts, and
  current feature dependency behavior.

Gate: packed generated chunks contain one continuous river rather than a
debug-only visualization.

### Slice 2: maps, tuning, and performance

- [ ] Extend the production review command with watercourse maps, ranges,
  percentiles, coverage, and selected river/wetland review sites.
- [ ] Compare several positive and negative seeds at lowlands, mountains,
  coasts, and the periodic seam.
- [ ] Capture fully warmed render-distance-16 top-down, landscape, and elevated
  cards from above terrain.
- [ ] Measure field-only, surface, cold decorated, and warm decorated cost
  against Tactical 196's plane baseline.
- [ ] Tune spacing, meander, width, bank grade, level, bed, and wetland response
  until remaining disagreement is subjective.

Gate: present the best multi-seed pixels and map evidence for human judgment.

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

Point sampling remains `O(live field count)` with fixed finite differences.
Chunk generation should batch a bounded halo and must not trace a channel,
walk downstream, flood fill, or acquire a process-global cache. Use the same
Tactical 196 release command and host for comparison.

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
