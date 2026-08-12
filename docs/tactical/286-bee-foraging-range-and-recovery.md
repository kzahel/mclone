# Tactical 286: Bee Foraging Range and Recovery

Status: **active 2026-08-12.**

Topics:

- `habitat-driven-creature-ecology`
- `playable-showcases`

## Instruction Synthesis

Human review rejected the first deployed bee behavior. All three bees remained
close to the nest, one appeared stuck, and another repeatedly visited the
closest flower. Correct the ordinary live behavior rather than rearranging a
showcase around it, strengthen acceptance so this failure cannot pass again,
then publish the repaired transient review world.

## Rejected Baseline

The result is explained directly by the implementation:

- every departure selects the nearest flower to the bee;
- after returning home, that usually means selecting the same nearby flower;
- all colony members therefore converge on the same local forage;
- travel is one collision-clipped straight-line request with no progress
  watchdog, alternate waypoint, or target abandonment; and
- `Hover` repeatedly targets the exact home point, so ordinary between-trip
  behavior is stationary even when it is not technically stuck.

The prior 80-tick browser gate required useful movement from only one of three
bees. Initial authored states could satisfy its clip and displacement checks
without proving a distributed, repeating colony loop. That evidence is
withdrawn as a behavioral acceptance result, though it remains valid evidence
for rendering, phone input, pollination, and transient storage.

## Corrected Contract

### Distributed forage

Flower choice must be based on the colony's habitat rather than whichever
flower is closest to the bee's current position. Each colony member receives a
stable near, middle, or far foraging preference so a small colony visibly
samples different parts of a rich meadow. Selection is deterministic for a
given runtime sequence, randomized within the preferred distance band, avoids
the immediately previous flower when another eligible flower exists, and
falls back across bands when edits leave the preferred band empty.

The search consumes real loaded flower blocks. It must not add a showcase-only
destination list or hidden flower map.

### Readable flight and recovery

Outbound and return trips use elevated, laterally varied cruise waypoints so
they read as flights across a habitat rather than ground-level straight lines.
Hovering uses bounded moving waypoints around the colony instead of continually
targeting its exact center.

Movement records progress. Repeated collision clipping triggers a new
collision-checked escape waypoint above or to either side of the obstruction.
An overlong outbound trip abandons its target and returns safely; an overlong
return replaces its route with a direct elevated recovery. No bee may spin in
place as a substitute for realized motion.

### Acceptance

Deterministic server tests must prove:

- three colony members choose multiple real flowers spanning near and far
  habitat;
- completed trips do not immediately repeat one flower when alternatives
  exist;
- an obstructed direct route makes progress through recovery waypoints; and
- persistence still records the active target and colony relationship.

The browser gate observes a substantially longer window and samples each
stable entity repeatedly. Acceptance requires all three bees to accumulate
useful path length, multiple bees to reach visibly non-local colony radii,
meaningful colony spread, bounded stationary spans, flight/forage clips, and a
real pollination update. A single moving bee or an authored starting offset is
not sufficient.

## Showcase Revision

Revision 2 may start its three ordinary saved bees at legible points in one
real loop: a near flower, a far outbound trip, and a return from another side
of the meadow. This is initial save composition only. Target selection,
waypoints, collision recovery, pollination, field notes, and later trips remain
ordinary gameplay with no recipe scripts or fixture-specific AI.

## Validation and Closeout

1. Land this rejection record before the corrective implementation.
2. Add focused movement/selection tests and run the complete server suite.
3. Run workspace checks and formatting.
4. Capture and inspect native flat/stereo pixels.
5. Run and inspect local headed-WebGPU desktop and phone gates.
6. Update the ecology/showcase topics with the rejected evidence and corrected
   behavioral measurements.
7. Commit, push, deploy the exact revision, rerun both public gates, and share
   the same transient URL and new screenshots.

