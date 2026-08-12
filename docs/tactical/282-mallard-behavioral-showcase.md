# Tactical 282: Mallard Behavioral Showcase

Status: **active 2026-08-12**

Topic:

- `playable-showcases`
- `habitat-driven-creature-ecology`

## Instruction Synthesis

Treat the first deployed mallard showcase as a failed interactive review. The
three ducks looked correct, but mostly remained in place while rapidly turning
left and right, so the tiny island did not expose useful flock, swimming,
shore, or life-cycle behavior. Record that evidence, repair the ordinary live
mallard simulation rather than scripting the showcase, give the family a
roomier wetland, add time-window acceptance evidence, then commit, deploy, and
verify the same public review link end to end.

## Rejected Baseline

The first `mallard-ecology` showcase passed static native/Web receipt and pixel
parity, but human interactive review rejected its behavior:

- all three mallards started at or near a valid habitat destination;
- the shallow-water search selected the current water cell before any useful
  destination, so the adults could remain stationary;
- their initial two-block spacing fell inside the flocking neutral band, so
  neither cohesion nor separation supplied motion;
- the water/shore target was recomputed every tick, allowing target and yaw
  oscillation without sustained travel;
- the shared special mallard movement path bypassed ordinary random-stroll and
  look goals whenever any water or shore target existed;
- the one-chunk island made the stalled behavior more obvious and provided too
  little room for a useful interactive observation.

The static screenshot was valid composition evidence, not behavioral
acceptance. A visually correct first frame and a matching deployed seed do not
prove that a live creature is interesting over time.

## Decisions

### Live behavior owns the correction

Mallards retain a bounded water-or-shore intent in shared authoritative
simulation. A destination is selected away from the current cell, held long
enough to produce legible travel, and replaced only after arrival, expiry, or
invalid habitat. Flock separation and cohesion steer that retained intent
instead of replacing it every tick. Body yaw follows realized horizontal
travel and is not mutated while collision or arrival leaves the duck still.

The showcase recipe contains no timers, scripted paths, AI flags, triggers, or
showcase-only gameplay branches. The repaired behavior is the same behavior
used by naturally spawned and persisted overworld mallards.

### A wetland is a review stage, not a game mode

Add one bounded authored wetland fixture to the existing base-terrain
allowlist. It supplies several chunks of shallow pond, shore, and bank so a
viewer can follow the family without immediately reaching a void edge. The
recipe continues to compile to ordinary chunks, entities, player state, and
persistence records; after hydration the integrated game has no showcase mode.

The family starts in a readable arrangement with room to move. Existing live
mechanics may be placed near an observable event, such as an attended nest
near hatching, but the recipe may only establish persistent state already
producible by ordinary gameplay. The event itself must advance through normal
server ticks.

### Time is part of acceptance

Retain the exact first-frame receipts, but add deterministic simulation and
browser observation windows. Acceptance must demonstrate displacement,
water/shore use, flock spacing, and stable orientation over time. A screenshot
remains required for composition and rendering; it is explicitly insufficient
as the sole proof of creature behavior.

## Scope

- Add retained mallard habitat intent and movement-derived orientation to the
  shared mob runtime.
- Preserve deterministic behavior across runs and ordinary save/hydration
  semantics. The short-lived runtime intent itself need not be persisted.
- Add focused multi-mallard time-window tests covering useful displacement,
  water and shore occupancy, bounded flock spacing, and no stationary yaw
  jitter.
- Add a reusable bounded authored wetland fixture and move the showcase family,
  nest, entry camera, and review patches into it.
- Stage field notes and resources so live discovery, hatching, and collection
  remain observable rather than appearing already completed on arrival.
- Extend Web showcase smoke evidence with a real-time behavior window and
  machine-readable mallard motion/life-cycle observations without introducing
  showcase-specific runtime policy.
- Capture and inspect the revised native flat/stereo and headed Web output.
- Push, deploy, open the public URL, compare the deployed receipt and pixels to
  local evidence, and record the final revision and remaining human checks.

## Guardrails

- No showcase-only entity AI, movement constants, scripted trajectories,
  teleports, tick hooks, or renderer behavior.
- No dynamic commands, conditions, timers, triggers, or goals in showcase JSON.
- Do not make every passive mob use the mallard intent contract merely to reuse
  code; keep species policy narrow while shared ownership remains server-side.
- Do not persist ephemeral path intent unless a later unload/reload acceptance
  proves that exact in-flight continuity is a product requirement.
- A wetland fixture may author bounded terrain composition, but it may not
  become the only instantiation path for any creature, item, observation, nest,
  or life-cycle fact.
- Browser diagnostics may observe ordinary replicated state. They may not
  command the ducks or alter simulation cadence to manufacture acceptance.
- Keep the public URL resettable and transient. It must create no IndexedDB
  world stores and must continue to reject conflicting launch overrides.

## Acceptance

- A fixed-seed authoritative simulation window proves that initially neutral
  spaced mallards travel through shallow water rather than remaining at their
  spawn cells.
- The same window observes both water and shore behavior and maintains useful
  flock proximity without collapse into one point.
- A tick with negligible realized horizontal displacement cannot change body
  yaw; retained travel does not exhibit rapid alternating turn jitter.
- The destination is held across ticks and excludes the mallard's current
  habitat cell when another suitable cell exists.
- Showcase compilation remains deterministic, strict, bounded, and supported
  on native and Wasm. Existing live-instantiation evidence still passes.
- The authored wetland spans multiple chunks, visibly contains shallow water
  and accessible shore, and hydrates through the ordinary world store.
- A local headed browser run observes meaningful mallard displacement during a
  bounded real-time window and reports the expected ordinary live event state.
- Revised native flat/stereo and browser screenshots are inspected at the first
  drawable milestone and after final composition.
- Focused tests, workspace/all-target checks, thin-adapter checks, Web build,
  local browser smoke, and desktop offscreen capture pass.
- The pushed revision deploys successfully; the public showcase reports the
  same recipe revision, seed, entry pose, and starting state as local evidence,
  then independently passes the behavioral observation window.

## Deferred

- General behavior trees, migration of every species to retained habitat
  intent, seasonal migration, predator/prey systems, and breeding genetics.
- A general scenario scripting language or long-running browser automation
  service.
- Final balance of mallard cadence, flock size, nest incubation, egg laying,
  feather drops, and observation timing in the procedural overworld.

## Execution Record

Pending.
