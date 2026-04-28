# CreatureMovement0 - Grounded mob travel

Status: landed as a reusable movement foundation; full vanilla `LivingEntity.travel(...)` and `GroundPathNavigation` parity remain open.

This tactical follows [`Creatures5-cow-tick-wander-foundation.md`](Creatures5-cow-tick-wander-foundation.md). It fixes the temporary direct steering layer so generated mobs do not interpolate their Y position toward a random target waypoint. The stack remains generic for all generated mobs, not cow-specific.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Entity move collision, vertical collision, and `onGround` ownership | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Ground branch of living travel, friction, gravity, and animation update boundary | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Navigation target creation, path following, waypoint handoff to move control | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |
| Ground surface Y selection and path creation around air/solid targets | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/GroundPathNavigation.java` |

## Landed Scope

- `MobAiLevel` now exposes `findStableStandingY(x, z, nearY)` as a generic host-world query for generated mob movement.
- `GeneratedWorldHost` answers that query by scanning loaded blocks near the mob's current foot Y for a stable standing destination: open foot/head blocks with solid support below.
- `GeneratedMobEntity.applyControlledTravel()` now moves directly steered mobs only in X/Z and lets the host supply the stable standing Y for the destination column.
- Temporary direct steering stops if the loaded world cannot provide a nearby stable standing Y instead of drifting through floors, ceilings, or holes.
- Focused cow AI coverage asserts that a high waypoint target does not pull the mob off the ground.

## Deliberate Runtime Divergence

Vanilla does not have a standalone `findStableStandingY(...)` API. The real game resolves mob Y through `PathNavigation`, `WalkNodeEvaluator`, `MoveControl`, `LivingEntity.travel(...)`, `Entity.move(...)`, collision shapes, step-up, gravity, fluids, and `onGround` detection.

The browser host still lacks that full stack. This slice introduces a narrow temporary query so the already-landed generated-mob AI path remains usable and all mobs share one ground-following behavior until the real pathfinding and travel ports land. This should be deleted when `GroundPathNavigation` plus collision-resolved `LivingEntity.travel(...)` own mob movement.

## Deferred

- Full `NodeEvaluator`, `WalkNodeEvaluator`, `PathFinder`, and live `PathNavigation` integration. `CreatureMovement1` has landed the data-only `Path`, `Node`, `Target`, `BlockPathTypes`, and `BinaryHeap` foundation.
- Collision-shape based `Entity.move(...)`, step-up, edge handling, gravity, block friction, stuck detection, and fluids.
- Door/fence/malus handling and per-mob pathfinding penalties.
- Real terrain traversal over drops, climbs, water, fences, leaves, and partial block collision shapes.

## Verification

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/runtime/generated-world-host-entities.test.ts test/worldgen/levelgen/demo-world-generators.test.ts
pnpm --silent typecheck
```

## Next Step

`CreatureMovement1` and `CreatureMovement2` have landed the data-only path primitives plus unit-tested weighted A*-style path search over the shared collision model. Proceed to [`CreatureMovement3-ground-path-navigation-integration.md`](CreatureMovement3-ground-path-navigation-integration.md) to replace temporary direct steering with live `GroundPathNavigation`.
