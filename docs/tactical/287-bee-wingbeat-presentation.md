# Tactical 287: Continuous Bee Wingbeat Presentation

Status: **complete 2026-08-13; exact public desktop/phone wing-phase evidence
accepted.**

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

## Execution Record

- `9734d7f3` records the human-reported failure, root cause, and phase-aware
  acceptance contract before implementation.
- `8ff5a196` changes ordinary shared bee `fly` animation from distance to
  elapsed phase. Hover and forage retain their elapsed clocks, and bees remain
  outside the ground-gait distance accumulator.
- The server test begins with the legacy distance-phased `fly` state, proves
  ordinary reconciliation repairs it to elapsed, retains one outbound start
  tick across repeated snapshots, and starts a later epoch on return. The
  render-session test proves 29 replicated ticks become `1.45` seconds of
  authored `fly` phase even when gait distance is zero.
- The complete server suite passes `658` tests, the complete render-session
  suite passes `131`, workspace all-target checks and formatting pass, and the
  existing native flat/stereo captures remain structurally correct.
- Local headed desktop and phone gates retain the prior 360-tick ecology
  acceptance. Every bee additionally accumulates `14.56`-`20.89` blocks of
  rendered movement while its uninterrupted elapsed `fly` phase advances
  `198`-`281` ticks. Phone still places one authoritative hotel, and all eight
  browser persistence counts remain zero.
- Exact pushed revision
  `8ff5a196c46a874837cd64ef34817816e2cfde46` deployed as Worker version
  `660f5e52-624f-48ba-a15f-13e5113643c5`. Public desktop and phone gates
  repeat the phase-aware acceptance: rendered in-flight travel is
  `14.57`-`20.84` blocks and advancing flight phase is `199`-`280` ticks per
  bee. The inspected public desktop and phone screenshot SHA-256 digests are
  `8217cdb05c37ec3e1e9666e70505594794fc5cb2c612eb5158030401fbe738a3`
  and
  `bf9dc0155c7f097ab44c24fb36e43f7e18fe1e8a3f1bc43542e5a26e64dddf6c`.
