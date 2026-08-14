# Tactical 296: Rabbit Player Avoidance

Status: complete 2026-08-14, including exact public desktop/phone acceptance

Topic: `wildlife-ecology-state-model`

Topic: `rabbit-burrow-ecology`

## Instruction Synthesis

Make a rabbit reliably flee an approaching player even when no burrow is
available or reachable. The immediate safety response is ordinary locomotion
away from the threat, not a mandatory trip home. Preserve selected-carrot
temptation, allow an already-started burrow entry to finish, and keep a hidden
rabbit hidden while danger remains at the mouth. Defer hearing, field of view,
crouch/sneak, scent, cover selection, and social alarm until the basic visible
reaction is trustworthy.

## Diagnosis

- The current six-block threat sensor works. A deployed ordinary-keyboard
  probe observed rabbits enter `flee` on the first gameplay sample after the
  player crossed the radius at about 5.7 and 5.9 blocks.
- The action selected after detection is wrong. Every showcase rabbit knows a
  refuge, so threat handling repeatedly replaces its route with that mouth
  even when the mouth is occupied, disturbed, or a poor escape direction.
- The primary mouth begins at capacity with its underground kit. Rabbits can
  therefore converge on one unusable point, turn or oscillate, and look
  indifferent despite being in the `flee` state. One observed rabbit travelled
  1.46 blocks of path while gaining only 0.04 blocks of net displacement.
- Threat handling also runs before the entry continuation and excludes the
  underground state. A rabbit may fail to complete entry while watched, or
  emerge while a non-tempting player remains at the mouth.
- The showcase makes this failure easy to see, but it is not a showcase-only
  problem. The correction belongs in ordinary authoritative rabbit AI.

## Reference and Deliberate Extension

Minecraft Java 1.17.1 gives rabbit avoidance a radius of eight blocks and a
speed multiplier of 2.2. `AvoidEntityGoal` asks `DefaultRandomPos` for a point
away from the threat, rejects a candidate that is closer, creates a real path,
and continues that path until navigation completes. It does not require a
home destination.

Mclone should preserve that recognizable invariant while retaining its own
burrow ecology. A burrow is a later shelter opportunity and an already-started
entry may complete, but refuge memory must not replace the immediate open-ground
escape response.

## Binding Behavior

- A non-tempting player inside eight blocks starts an urgent flee reaction.
  Only a carrot in the player's selected hotbar slot suppresses avoidance;
  carrots elsewhere in inventory do not.
- Select a walkable endpoint in the half-plane away from the nearest threat.
  The endpoint must be farther from that player than the rabbit's current
  position and must have a complete route through shared A* navigation.
- Commit to the accepted route instead of retargeting every gameplay tick.
  Continue until the rabbit is at least ten blocks from the threat or has
  finished its route; if it remains unsafe, choose another farther route.
- A blocked or invalidated route may be replanned through another bounded
  candidate. It must not fall back to an unvalidated geometric destination.
- Immediate threat detection and the visible `flee` state are not delayed by
  idle decision cadence. New escape paths consume the existing deterministic
  path-request budget ahead of idle habitat paths, with rotating fairness and
  no unbounded population-wide A* burst.
- Do not select, reserve, enter, or excavate a refuge merely because a player
  is nearby. Ordinary fatigue, active rest, and later shelter decisions retain
  their existing refuge semantics once danger has cleared.
- An `EnterBurrow` action that already crossed its threshold may finish and
  hide the animal. An underground rabbit remains hidden while a non-tempting
  player is within the continuation radius; it does not emerge into danger.

## Validation and Acceptance

Focused server tests must prove:

1. a rabbit with no refuge detects an unselected-carrot player, accepts a
   complete farther path, makes material away-from-player progress, and clears
   the reaction only outside the continuation radius;
2. a carrot elsewhere in inventory does not suppress avoidance, while the
   selected carrot retains normal temptation;
3. known available and full mouths do not become the initial escape target;
4. a blocked direct line selects or replans to a complete off-axis path rather
   than freezing or applying an incomplete route;
5. repeated ticks reuse the committed escape path instead of oscillating its
   destination;
6. already-started entry completes and an underground rabbit does not emerge
   beside a nearby non-tempting player; and
7. the 1,000-rabbit fixture retains fixed path-work bounds and fair eventual
   admission under simultaneous threat.

Advance the data-only `rabbit-burrow` showcase only if composition needs to
make the ordinary behavior easier to review. Its desktop and phone browser
gate must approach a rabbit through normal controls with carrots unselected,
observe prompt `flee`, and prove meaningful net separation rather than merely
an animation label. It must leave all browser world-record stores empty. Run
native flat/stereo, local headed WebGPU desktop/phone, and exact deployed
desktop/phone acceptance, inspect captures, and share the fresh transient URL.

## Non-Goals

- hearing, footstep loudness, crouch/sneak, vision cones, scent, wind, or
  species-specific sensory acuity;
- intelligent cover scoring, burrow-seeking during the immediate reaction,
  coordinated alarm, predator response, or combat;
- new pathfinding infrastructure, a general behavior-tree system, or lower
  fidelity movement through unloaded terrain; and
- changing carrot feeding, breeding, garden raids, refuge memory, collapse,
  or excavation beyond regressions required by this correction.

## Execution Record

The authoritative rabbit owner now treats a nearby non-tempting player as an
urgent open-ground escape rather than a request to return home. It samples a
small deterministic set of farther away-side endpoints, accepts only a
complete shared A* route, commits to that route through the ten-block
continuation radius, and retries another bounded candidate if the route is
blocked. Refuge reservations and dig targets are cleared without selecting,
reserving, or excavating a mouth for the immediate reaction.

Only the selected carrot suppresses avoidance. Entry already in progress
finishes, and a rabbit that actually observes a threat while entering or
underground retains a transient threat hold through the continuation radius.
Underground state alone does not manufacture that history; an exact local
showcase replay caught and corrected that distinction before deployment.

Urgent escape requests consume the existing 32-path-request tick budget ahead
of idle habitat work. Persistent-ID ordering with a deterministic rotating
start preserves eventual admission when early identities remain unroutable.
The 1,000-rabbit regression proves both the hard per-tick bound and fair
eventual attempts.

The ordinary data-only `rabbit-burrow` revision remains 4. Its browser gate
now chooses a visible adult, verifies that carrots are not selected, approaches
through keyboard or rendered touch-joystick input, observes the same replicated
rabbit enter `flee`, then requires at least 1.5 blocks of net separation and
1.5 blocks of subject travel. It continues through the existing refuge reuse,
hiding and same-ID return, off-axis gate, two raids, separation, three-hit
collapse, survivor, and zero-persistence gates.

Validation completed with:

- the exact 715-test `mclone-server` suite and the broad native workspace gate;
- exact native flat and synthetic-stereo revision-4 captures at seed `17507`,
  entry eye `14.5,66.62,18.5`, and target `15,65.5,9.5`;
- local desktop Web detection at 7.78 blocks, 1.57 blocks of gained separation,
  and 1.95 blocks of rabbit travel;
- local phone Web detection at 7.83 blocks, 1.62 blocks of gained separation,
  and 1.95 blocks of rabbit travel; and
- exact public desktop and phone repetitions with comparable 7.78/7.88-block
  detection, 1.57/1.62-block separation gain, and 1.95-block subject travel.

All eight IndexedDB world-record stores remained empty in every browser lane.
Exact pushed revision `1d7a37826016ce7126ee3558734c88c9acdaf680`
deployed as asset version `1d7a37826016-20260814172024` under Cloudflare
Worker version `770dcd53-927c-4055-a344-c8a0b6f2efbd`. Inspected public clean
first-frame desktop and phone digests are
`673c158aaa159472670e21aedf06a4984223dbf6b7ab4cf61b3313ff395ffab4`
and
`866cbed8a4be978c199df3aaf6abe2149e92412d2b9ea23ed165ee00718a564c`.
The phone frame is byte-identical to local. The desktop frame differs in only
842 of 1,440,000 pixels because its live rabbit pose advanced slightly; seed,
camera, terrain, habitat, HUD, and behavior receipts match. The fresh review
link is `https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.
