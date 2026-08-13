# Rabbit Navigation and Burrow Visibility Correction

Topic: `rabbit-burrow-ecology`

Status: active corrective slice started 2026-08-13.

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

Pending.
