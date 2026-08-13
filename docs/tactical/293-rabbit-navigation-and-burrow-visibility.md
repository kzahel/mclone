# Rabbit Navigation and Burrow Visibility Correction

Topic: `rabbit-burrow-ecology`

Status: complete 2026-08-13.

## Motivation

Interactive review of Tactical 292 exposed two gaps between its binding
contract and the live implementation:

- rabbit habitat intents selected walkable destinations but moved toward them
  with straight-line collision steering, bypassing the existing shared A*
  `GroundPathNavigation`; and
- `Underground` rabbits retained their client tracking pair. The server reduced
  their collision dimensions, but entity updates do not replicate dimensions,
  so the full rabbit could remain rendered at the burrow mouth.

The original showcase gate happened to lie close to the straight route between
the family and its first carrots. Its behavioral gate proved collision and an
eventual crop mutation, not an off-axis route. This tactical corrects the live
system and strengthens acceptance so the shortcut cannot pass again.

## Binding Correction

Every moving `RabbitHabitatIntent` uses the shared ground navigator. Rabbit
hop presentation remains species-owned, but its immediate steering target is
the next A* waypoint rather than the final habitat target. Rabbit navigation:

- requests an exact route to the destination cell;
- rejects bounded best-effort partial paths as unreachable;
- treats fences and closed gates through the normal walk-node evaluator;
- follows an off-axis opening without coordinate or showcase exceptions;
- invalidates nearby habitat intent after ordinary terrain edits so a newly
  opened route is reconsidered; and
- abandons or reselects a destination when the route cannot be completed.

`EnterBurrow` remains a visible one-shot transition at the real mouth.
`Underground` is the deeper compact-warren state: the same alive persistent
entity keeps ticking and saving, but is hidden from client entity tracking.
The visible tracking pair is removed exactly on the deep-state transition and
the same persistent identity is snapshotted again on `Emerge`. Hydrating a
saved underground rabbit must not flash it for one frame.

## Acceptance

Focused coverage must prove:

1. a rabbit reaches and raids a carrot only by detouring through an off-axis
   fence opening;
2. the same result occurs when the opening is made after the rabbit first sees
   the enclosed crop;
3. a nearby ordinary block edit invalidates/recomputes rabbit navigation while
   unrelated distant edits do not;
4. entering remains visible, underground tracking emits removal, emergence
   republishes the same entity and persistent IDs, and saved underground state
   hydrates hidden; and
5. the `rabbit-burrow` recipe places its gate off the direct carrot line and
   its ordinary desktop/phone behavior gate still observes protection followed
   by a real raid.

Run focused server/navigation/tracking/showcase tests, the native workspace
test gate, shared adapter checks, native capture, and local Web desktop/phone
behavioral acceptance. Any deployment receipt belongs in the execution record
after the exact pushed revision is verified.

## Non-Goals

- a rabbit-only pathfinder, scripted route, navmesh, tunnel graph, or
  player-walkable warren;
- changing the compact burrow persistence model or rabbit identity; and
- hiding the entry or emergence animation before the rabbit crosses the
  threshold.

## Execution Record

Commits `b0f7c28b` and `9a9ec120` completed the live correction and its
adversarial showcase acceptance. The shared server now:

- orders real habitat candidates, accepts only complete exact routes from
  `GroundPathNavigation`, and advances rabbit hop steering through the current
  waypoint;
- clears rabbit intent and navigation after a nearby ordinary block edit, but
  leaves distant edits alone, so opening a gate causes an autonomous replan;
- rejects partial A* results and clears a failed dig target rather than
  steering into the obstruction; and
- keeps entry and emergence visible while removing `Underground` rabbits from
  client tracking. Persistence hydrates that deep state hidden, and emergence
  republishes the same entity and persistent identities.

The recipe advanced to revision 2. The former near-line opening at
`16,65,10` is now fence and the ordinary oak gate moved off-axis to
`16,65,14`. Both the compiler test and browser harness bind those cells so the
old direct-steering implementation cannot satisfy acceptance.

Focused server tests prove the closed crop is unreachable, a later block edit
opens a complete route, the rabbit takes the off-axis detour and performs the
real raid, and deep shelter emits remove/resnapshot tracking around the same
identity. Saved-underground hydration also starts hidden. The authoritative
carrot-feeding test remains focused coverage; feeding was deliberately removed
from this route-specific browser window rather than coupling an unrelated
interaction to camera retreat.

Local desktop and phone browser runs used recipe revision 2 and seed `17507`.
Both preserved carrot states `308,308,308,308` while the boundary was closed,
opened the real gate from state `270` to `274`, then observed two real raids.
All four rabbits travelled roughly 24–30 blocks, exercised hop, emerge, dig,
and forage clips, created one additional live burrow, and left all eight
IndexedDB world-record stores empty. Inspected desktop and phone capture
digests are
`4304b9a8a4ee5f885ed66d33eb4a3a25c9a0ce0503fbb8ad909f6e3858ba6d30`
and
`8e444f5d399db07b4b8d1406c64dd84a6c2a6bf85feed2fe790077c5859a99f6`.

`pnpm native:rabbit-burrow:capture` passed flat and stereo capture; inspected
digests are
`3fe3147309f10387544b7875f2f76bf344b6fb552e40f119e7d4ecc807a24e64`
and
`32a8947fa83b9c923f1cd88bbabe4dec82f849d50f2e31e8c2bfe0054ed4e232`.
The ordinary offscreen smoke also produced credible inspected pixels with two
drawn actors.

Validation passed:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`;
- `cargo check --manifest-path native/Cargo.toml -p mclone-server`;
- `cargo test --manifest-path native/Cargo.toml`;
- `pnpm native:thin-adapters:purity`;
- `pnpm native:web:build`;
- `pnpm native:desktop-offscreen:smoke`;
- `pnpm native:web:rabbit-showcase-smoke`; and
- `pnpm native:web:rabbit-showcase-mobile-smoke`.

The first broad workspace attempt saw one transient failure in the unrelated
homestead-resident removal persistence test. A clean current-source rebuild,
five consecutive isolated repeats, and the complete workspace rerun all
passed; no rabbit change was made to suppress or bypass it. The deployment
receipt is recorded below.

Exact pushed revision `d466a763` deployed as Cloudflare Worker version
`61ee5243-c972-4f2a-8dcb-7d1a175fe0e9`. Public desktop and phone gates each
repeated revision 2's protected `308,308,308,308` state, gate transition
`270 -> 274`, two raids, roughly 24–30 blocks of travel for every rabbit, all
four required clips, the second burrow, and zero records in all eight browser
world stores. The inspected public desktop and phone capture digests are
`77e3b19214dcd8b87d4cabc1b58e7b5964a65d79f7780c1d9df191cc860fefd2`
and
`123e96492ea92f163fa3ce89b4a6dd09698c200498eb9a772e749de7b513dd83`.
The fresh non-persistent review URL is
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.
