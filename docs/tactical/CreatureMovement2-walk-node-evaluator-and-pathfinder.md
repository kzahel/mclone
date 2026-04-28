# CreatureMovement2 - Walk node evaluator and pathfinder

Status: planned.

This tactical follows [`CreatureMovement1-path-node-foundation.md`](CreatureMovement1-path-node-foundation.md). It ports the vanilla path search core and the ground-mob node evaluator against the host's loaded block/collision view.

## Vanilla Sources To Read

| Concern | Source |
|---|---|
| Search loop, target selection, node expansion, and path reconstruction | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/PathFinder.java` |
| Base evaluator lifecycle and node helpers | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/NodeEvaluator.java` |
| Ground block classification, floor level, collision checks, malus handling | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/WalkNodeEvaluator.java` |
| Navigation search world window | `reference/minecraft-1.17.1/src/net/minecraft/world/level/PathNavigationRegion.java` |
| Path computation type and block traversal predicates | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/PathComputationType.java` |

## Scope

- Add a host-neutral `PathNavigationRegion` adapter over loaded chunks/block states.
- Port `NodeEvaluator` and `WalkNodeEvaluator` directly enough for common passive land mobs.
- Port `PathFinder.findPath(...)` and path reconstruction.
- Preserve vanilla malus defaults and `BlockPathTypes` behavior for open, blocked, water, fence, danger, damage, and walkable nodes where the required block tags/materials already exist.
- Add unit tests against small synthetic worlds for flat ground, one-block step, blocked wall, water avoidance, unreachable target, and partial path behavior.

## Deliberate Deferrals

- Door opening/breaking behavior if the required block APIs are not present yet.
- Full collision shapes for every block until the shared collision system exposes the needed voxel-shape queries.
- Mob-specific pathfinding malus overrides beyond the passive animal defaults already required by cows, pigs, sheep, and chickens.

## Validation

```bash
pnpm test -- test/world/entity/pathfinding
pnpm --silent typecheck
```

## Next Step

Proceed to [`CreatureMovement3-ground-path-navigation-integration.md`](CreatureMovement3-ground-path-navigation-integration.md).
