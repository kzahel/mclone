# CreatureMovement3 - Ground path navigation integration

Status: planned.

This tactical follows [`CreatureMovement2-walk-node-evaluator-and-pathfinder.md`](CreatureMovement2-walk-node-evaluator-and-pathfinder.md). It replaces the temporary direct steering internals with a vanilla-shaped `GroundPathNavigation` path-following stack.

## Vanilla Sources To Read

| Concern | Source |
|---|---|
| Path create/move/tick/follow/stuck behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |
| Ground pathfinder selection, surface Y, direct-walk checks, sun trim | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/GroundPathNavigation.java` |
| Move control yaw and wanted-position handoff | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java` |
| Living entity travel branch for ground movement | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Entity movement collision and `onGround` resolution | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |

## Scope

- Replace `SimpleGroundPathNavigation` with a direct `GroundPathNavigation` port for generated mobs.
- Keep `MoveControl` as the movement handoff and remove the temporary direct-waypoint target as the source of truth.
- Add path following, waypoint advancement, stuck detection, recomputation hooks, and max-distance-to-waypoint behavior.
- Introduce the minimum collision-resolved mob travel needed for passive land walking to consume wanted movement without tunneling or Y drift.
- Keep the host authoritative: generated mobs tick on the server/host path and publish normal `entity_update` deltas.

## Out Of Scope

- Swimming, climbing, riding, fall flying, ladder/powder snow details, sounds, and damage.
- Natural spawning, despawn, breeding, panic, temptation, combat, or player interactions.
- Renderer-specific work.

## Validation

```bash
pnpm test -- test/world/entity test/runtime/generated-world-host-entities.test.ts
pnpm --silent typecheck
pnpm test:browser:integration
```

Browser integration should be used once live path following changes rendered entity movement.

## Next Step

Proceed to [`Creatures6-friendly-island-mobs-and-basic-behaviors.md`](Creatures6-friendly-island-mobs-and-basic-behaviors.md) once the shared ground navigation path is available.
