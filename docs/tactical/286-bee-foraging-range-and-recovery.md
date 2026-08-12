# Tactical 286: Bee Foraging Range and Recovery

Status: **complete 2026-08-12; corrected live behavior and exact public
desktop/phone evidence accepted.**

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

## Execution Record

- `7e8e8832` recorded the human-rejected behavior and replacement contract
  before changing implementation.
- `06b0bdde` removes nearest-only selection from shared bee AI. Stable colony
  member ordering assigns near, middle, and far distance bands; selection is
  deterministic but random within real loaded flowers, and the immediately
  previous flower is excluded while an alternative exists.
- Hover now moves between bounded points around home. Long trips stage through
  elevated lateral cruise points. Six consecutive clipped movement requests
  produce a collision-checked side-and-up recovery point, and overlong trips
  return home instead of remaining indefinitely stuck.
- Focused server tests prove all three distance bands, previous-flower
  avoidance, completion over a five-block obstruction, and ordinary persistent
  trip/colony state. The complete server suite passes `657` tests; workspace
  all-target checks and formatting pass.
- Recipe revision 2 moves the initial outbound and returning bees farther into
  the existing meadow without adding any script or behavior field. Native flat
  and stereo captures draw the same four semantic actors; their inspected
  SHA-256 digests are
  `b6afc00fd8125d6ff886e437ab7a9e37a70af2a73a749099911a69dc24a95224`
  and
  `b308a1778a413bcc9c40512d8280cf4d4ee8aba6b364168f5ecb153b19c991dd`.
- Local headed desktop and phone gates sample 19 poses over 360 ticks. Bee
  travel is `20.20`, `22.83`, and `21.23` blocks; maximum colony radii are
  `7.42`, `15.39`, and `12.40`; every longest near-stationary streak is one
  sample; all three named clips appear; one pollination block update occurs;
  and every browser persistence count is zero. Phone still places one real
  hotel through slot nine and `USE`.
- Inspected local Web desktop and phone screenshots have SHA-256
  `c881c5e71b9d54cb0469ceb556362bc3fb69618312973382c6971104562e7515`
  and
  `86f8b654142acbea0cd1de5b10cde1fca3bcc3494d7403a73d08527390d22468`.
- Exact pushed behavioral revision
  `fe7eb06d43046012297b225b36fee258e79f675e` deployed as Worker version
  `6a212f1b-3aa0-4646-a248-8931ab24e232`. Its public desktop and phone gates
  reproduce the local 360-tick measurements exactly, including all-bee
  travel, the three distinct colony radii, one-sample maximum stationary
  streaks, all three animation clips, one pollination update, and zero
  browser-world records. The phone path also places one authoritative hotel.
- The inspected public desktop and phone screenshots have SHA-256
  `5480fee7aef944d351a0193112ac778d53e58f53fe441a1b405c5f7402d78578`
  and
  `bbbf89ca53f1f52052da570a634412fce308c1c7b14dea9631ba0a0cd78b8029`.
  They show three bees occupying near, middle, and far parts of the meadow
  rather than clustering at the hotel.
