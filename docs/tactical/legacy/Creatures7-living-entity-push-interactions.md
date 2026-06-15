# Creatures7 - Living entity push interactions

Status: first generated-mob/local-player push baseline landed.

This tactical records the vanilla entity-overlap behavior behind the observed "animals pass through each other" issue. Normal passive mobs and players should not become hard block-like path obstacles. They should separate through the living-entity push pass when their bounding boxes overlap.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Entity movement collision and `Entity.push(...)` formula | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Living entity push pass | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Pushability filters and team-collision rules | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntitySelector.java` |
| Entity collision shapes in movement/no-collision queries | `reference/minecraft-1.17.1/src/net/minecraft/world/level/EntityGetter.java`, `CollisionGetter.java` |
| Pathfinding-region entity-collision behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/level/PathNavigationRegion.java` |

## Vanilla Model

- `Entity.move(...)` clips movement against block collisions plus hard entity collision shapes from `Level.getEntityCollisions(...)`.
- Ordinary `LivingEntity` instances set `blocksBuilding = true` and are `isPushable()` while alive, not spectator, and not on climbable blocks.
- Base `Entity.canBeCollidedWith()` returns `false`; ordinary living mobs are pushable but are not hard solid blockers for movement/pathfinding.
- `LivingEntity.aiStep()` runs `pushEntities()` after travel. That query finds overlapping pushable entities through `EntitySelector.pushableBy(this)`.
- `Entity.push(other)` applies the vanilla horizontal impulse formula to both entities unless one is a vehicle or either entity has `noPhysics`.
- `PathNavigationRegion.getEntityCollisions(...)` returns an empty stream in vanilla, so path search does not treat ordinary mobs as obstacles.

## Landed Scope

- Added a reusable `collectLivingEntityPushDeltas(...)` helper that ports the vanilla `Entity.push(...)` horizontal impulse formula.
- Kept generated-mob pathfinding and `noCollision(...)` block-focused, matching vanilla `PathNavigationRegion` behavior for entity collisions.
- Integrated an authoritative host push pass after generated entity ticks. Ticking generated mobs initiate pushes; tracked pushable mobs and the local player can receive them.
- Added local-player push displacement through the authoritative player state and movement-body snapshot, with block-collision rejection.
- Applied generated-mob push displacement through the existing stable-standing-Y and block-collision checks so animals separate without drifting vertically or entering blocks.
- Published normal `entity_update` and `player_state` messages when push displacement changes authoritative state.
- Added coverage for vanilla push math, non-ticking source behavior, no-physics/non-pushable skips, player push state updates, mob/mob host separation, and mob/player host separation.

## Runtime Divergence

Vanilla applies `Entity.push(...)` to `deltaMovement`; the displacement is then consumed by the normal `travel(...)` / `move(...)` pipeline on a later movement step. `mclone` does not yet have full `LivingEntity.travel(...)`, `Entity.move(...)`, or player `deltaMovement` parity. This slice applies the vanilla push deltas as small collision-checked horizontal displacement inside the authoritative host tick.

The divergence is intentionally narrow:

- push direction and magnitude use the vanilla formula
- entity pathfinding still does not treat ordinary mobs as hard blockers
- block collision still gates the final displaced position
- the application point can move from direct displacement to `deltaMovement` when the full travel pipeline lands

## Deferred Scope

- Full `deltaMovement`, `hasImpulse`, `hurtMarked`, and `Entity.move(...)` parity.
- Hard entity collision shapes for boats, minecarts, shulkers, end crystals, and other non-ordinary cases where `canBeCollidedWith()` is true.
- Team collision rules from `EntitySelector.pushableBy(...)`.
- Spectator/creative special cases, climbing checks, passengers/vehicles, entity cramming damage, auto-spin attack, and water/lava special movement.
- Remote multi-player push integration for dedicated/HTTP sessions after the remote host owns the same player-entity abstraction.

## Validation

```bash
pnpm test -- test/world/entity/entity-push.test.ts test/runtime/player-loop.test.ts test/runtime/generated-world-host-entities.test.ts
pnpm --silent typecheck
```

## Next Step

Shared passive look/idle goals are now tracked in `Creatures8`. The next movement-parity follow-up should be full `deltaMovement`/`LivingEntity.travel(...)` integration so push impulses, gravity, friction, and block movement all share the same application path.
