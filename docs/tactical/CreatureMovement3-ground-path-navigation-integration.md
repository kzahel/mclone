# CreatureMovement3 - Ground path navigation integration

Status: landed as live passive-mob path following; full vanilla `LivingEntity.travel(...)` parity remains open.

This tactical follows [`CreatureMovement2-walk-node-evaluator-and-pathfinder.md`](CreatureMovement2-walk-node-evaluator-and-pathfinder.md) and assumes [`BlockCollision0-render-occlusion-vs-collision-shapes.md`](BlockCollision0-render-occlusion-vs-collision-shapes.md) has landed. It replaces the temporary direct steering internals with a vanilla-shaped `GroundPathNavigation` path-following stack.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Path create/move/tick/follow/stuck behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |
| Ground pathfinder selection, surface Y, direct-walk checks, sun trim | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/GroundPathNavigation.java` |
| Move control yaw and wanted-position handoff | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java` |
| Living entity travel branch for ground movement | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Entity movement collision and `onGround` resolution | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |

## Landed Scope

- Replace `SimpleGroundPathNavigation` with a direct `GroundPathNavigation` port for generated mobs.
- Keep `MoveControl` as the movement handoff and remove the temporary direct-waypoint target as the source of truth.
- Add path following, waypoint advancement, stuck detection, recomputation hooks, and max-distance-to-waypoint behavior.
- Introduce the minimum collision-resolved mob travel needed for passive land walking to consume wanted movement without tunneling or Y drift, reusing the same collision-shape query path as player movement.
- Keep the host authoritative: generated mobs tick on the server/host path and publish normal `entity_update` deltas.
- Extend the host `MobAiLevel` facade into the loaded `PathNavigationRegion` view that live generated mobs use for block/fluid/collision pathfinding reads.
- Preserve `GroundPathNavigation.createPath(...)` target normalization: high air targets descend to the standing surface and solid targets rise to the first open node.

## Deliberate Runtime Divergence

Vanilla `PathNavigation` constructs a fresh bounded `PathNavigationRegion` from `Level` and nearby chunks for each search. `mclone` exposes the loaded host region through `MobAiLevel` because generated mobs already tick behind the host boundary and should not reach directly into `GeneratedWorldHost` internals. Missing loaded blocks are treated as colliding for pathfinding/travel, so mobs do not route into unknown chunks.

The movement step is still a narrow passive-land approximation of `LivingEntity.travel(...)` plus `Entity.move(...)`: `GroundPathNavigation` picks waypoints, `MoveControl` computes wanted yaw/speed, and `GeneratedMobEntity` applies a bounded horizontal step only when the host collision view accepts the resulting mob AABB. This is enough to consume paths without Y drift or tunneling, but it is not the full vanilla physics stack.

## Out Of Scope

- Swimming, climbing, riding, fall flying, ladder/powder snow details, sounds, and damage.
- Natural spawning, despawn, breeding, panic, temptation, combat, or player interactions.
- Renderer-specific work.
- Exact `LivingEntity.travel(...)`, jump control, friction, fluid push, block speed factors, edge cases for doors/fences/trapdoors/rails, and sky-avoidance trimming.

## Validation

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/runtime/generated-world-host-entities.test.ts test/world/level/pathfinder/path-node-foundation.test.ts test/world/level/pathfinder/walk-node-pathfinder.test.ts
pnpm --silent typecheck
```

Browser integration is still appropriate before visual/entity-render follow-through, but this slice's behavior is covered by host/runtime tests.

## Next Step

Proceed to [`Creatures6-friendly-island-mobs-and-basic-behaviors.md`](Creatures6-friendly-island-mobs-and-basic-behaviors.md): add the remaining friendly farm animals to the island preset after their render/model coverage exists.
