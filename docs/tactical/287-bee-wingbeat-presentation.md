# Tactical 287: Continuous Bee Wingbeat Presentation

Status: **planned 2026-08-13; implementation pending.**

Topics:

- `habitat-driven-creature-ecology`
- `playable-showcases`

## Instruction Synthesis

Human review accepted the wider bee foraging loop but found that wing flapping
stops while a bee flies toward a flower. Correct the ordinary shared
presentation behavior, prove the animated pose advances during realized
outbound flight, then redeploy the same transient review world.

## Root Cause

Authoritative bee state selects the authored `fly` clip for both outbound and
return travel, but marks it as distance-phased. The shared client intentionally
does not place bees in its ground-locomotion horizontal-distance accumulator,
so their render distance remains zero and the renderer continually samples the
first `fly` pose. The existing showcase gate checks only the selected clip and
entity displacement; it can therefore accept a moving bee with frozen wings.

## Corrected Contract

- `fly` is an elapsed-time loop. Wings keep beating during horizontal ascent,
  descent, collision recovery, and any brief in-air pause.
- `hover` and `forage` remain elapsed-time loops. No bee-specific renderer
  clock or app-local animation exception is added.
- Entering outbound or return flight starts a new authoritative elapsed clip
  epoch. Repeated snapshots of that same state retain its original start tick;
  hydration repairs the former distance-phased state through the ordinary
  animation-state reconciliation.
- Do not add bees to the ground-locomotion distance accumulator. Distance is a
  useful gait phase for feet contacting terrain; it is the wrong clock for an
  insect that must remain airborne even when instantaneous translation is
  small or vertical.

## Acceptance

1. A focused authoritative test proves both outbound and return behavior
   select elapsed `fly`, retain their start tick across repeated ticks, and
   advance the replicated animation clock.
2. A shared render-session test proves an elapsed flying-bee presentation
   becomes a `fly` actor animation with the expected elapsed seconds.
3. The bee browser gate samples rendered presentation state by stable entity
   ID and requires a bee to translate while its uninterrupted `fly` phase also
   advances. Merely observing the clip name is insufficient.
4. Existing all-bee range, recovery, pollination, phone-use, and zero-browser-
   persistence checks remain mandatory.
5. Inspect native flat/stereo and headed Web desktop/phone output, then deploy
   the exact pushed revision and repeat the public gates.

