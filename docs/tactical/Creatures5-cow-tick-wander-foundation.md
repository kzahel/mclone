# Creatures5 - Cow tick and wander foundation

Status: landed as a first movement foundation; full vanilla pathfinding parity remains open.

This tactical follows [`Creatures4-cow-baseline-lifecycle.md`](Creatures4-cow-baseline-lifecycle.md). It starts cow simulation on the authoritative entity path instead of keeping generated cows as stationary snapshots.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Cow goal order and attributes | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java` |
| Mob AI tick owner | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Pathfinder mob walk target value | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/PathfinderMob.java` |
| Goal selection and priority replacement | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/GoalSelector.java` |
| Goal wrapper lifecycle | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WrappedGoal.java` |
| Base goal flags/lifecycle | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/Goal.java` |
| Random stroll interval and no-action gate | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomStrollGoal.java` |
| Cow wandering target choice | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WaterAvoidingRandomStrollGoal.java` |
| Land/default random target search | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/util/LandRandomPos.java`, `DefaultRandomPos.java`, `RandomPos.java`, `GoalUtils.java` |
| Movement control yaw/speed behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java` |
| Ground navigation/pathfinding boundary | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java`, `GroundPathNavigation.java` |

## Landed Scope

- `GeneratedMobEntity` now has vanilla-shaped cow tick state: `tickCount`, random source, `GoalSelector`, `MoveControl`, ground navigation, no-action state, attributes, and cow `WaterAvoidingRandomStrollGoal` registration.
- Cow attributes include vanilla defaults needed here: max health `10.0` and movement speed `0.2`.
- `EntityRuntime` exposes lifecycle processing separately from entity ticking so chunk status changes can start/stop ticking without advancing AI during publication.
- `GeneratedWorldHost` promotes published generated chunks to `ENTITY_TICKING` for entity activity, ticks generated entities on the world tick, and publishes `entity_update` messages when position/rotation/chunk-owned facts change.
- Entity snapshots and movement updates now carry authoritative `tick` context.
- Focused tests cover entity-ticking chunk gating, forced cow `WaterAvoidingRandomStrollGoal` movement, and host-side `entity_update` publication for a moving generated cow.

## Deliberate Runtime Divergences

Two pieces are intentionally incomplete and should not be mistaken for full vanilla cow parity:

- `SimpleGroundPathNavigation` steers directly toward the chosen waypoint. Vanilla uses `PathNavigation`, `GroundPathNavigation`, `PathFinder`, `WalkNodeEvaluator`, `Path`, `Node`, and `BlockPathTypes`.
- `GeneratedWorldHost` uses current chunk-view interest as the no-action reset proxy until player tickets, surface-aware spawn placement, and natural-spawn distance ownership are wired to entity ticking.

Both divergences are narrow. The random-stroll goal, goal selector, target generation, entity section movement callback, and host protocol update path are in place so the next slice can replace navigation internals without changing entity ownership.

## Deferred

- Full `PathFinder` / `WalkNodeEvaluator` / `BlockPathTypes` parity.
- Collision-resolved `LivingEntity.travel(...)` movement, friction, step-up, jumps, fluid movement, and path stuck detection.
- Cow despawn rules, health/damage/death, sounds, drops, breeding, milking, and interactions.
- Durable entity persistence beyond the current in-memory runtime path.
- Per-session entity tracking range beyond the current published chunk-interest baseline.

## Verification

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/runtime/generated-world-host-entities.test.ts test/runtime/client-entity-interpolation-service.test.ts test/runtime/world-message-queue.test.ts
pnpm --silent typecheck
```

## Next Step

Port the real vanilla ground pathfinding stack needed by cow wandering: `Path`, `Node`, `BlockPathTypes`, `NodeEvaluator`, `WalkNodeEvaluator`, `PathFinder`, then replace `SimpleGroundPathNavigation` with a direct `GroundPathNavigation` port.
