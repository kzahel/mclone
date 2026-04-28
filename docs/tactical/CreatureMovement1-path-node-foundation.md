# CreatureMovement1 - Path node foundation

Status: landed.

This tactical follows [`CreatureMovement0-grounded-mob-travel.md`](CreatureMovement0-grounded-mob-travel.md). It ports the vanilla pathfinding data model before any search algorithm or movement integration changes.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Path node type enum and malus categories | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/BlockPathTypes.java` |
| Search node coordinates, heap index, costs, type, and closed flag | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/Node.java` |
| Target node scoring wrapper | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/Target.java` |
| Path node list, next-node index, target, reach flag, and entity-position helpers | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/Path.java` |
| Binary heap used by path search | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/BinaryHeap.java` |

## Landed Scope

- Add direct TypeScript ports of `BlockPathTypes`, `Node`, `Target`, `Path`, and `BinaryHeap`.
- Preserve vanilla field names where practical so later `PathFinder` and `PathNavigation` ports read the same way as the Java source.
- Add unit tests for node hashing, clone/move behavior, heap ordering, path advancement, entity-position helpers, replacement/truncation, and `sameAs(...)`.
- Do not connect these types to live mobs yet. This slice is data-only.
- Keep vanilla stream serialization out of scope until a network/debug payload needs the exact `FriendlyByteBuf` shape.

## Out Of Scope

- `NodeEvaluator`, `WalkNodeEvaluator`, `PathFinder`, or any world reads.
- Replacing `SimpleGroundPathNavigation`.
- Entity collision, travel, step-up, or fluid behavior.

## Validation

```bash
pnpm test -- test/world/level/pathfinder/path-node-foundation.test.ts
pnpm --silent typecheck
```

## Next Step

Proceed to [`CreatureMovement2-walk-node-evaluator-and-pathfinder.md`](CreatureMovement2-walk-node-evaluator-and-pathfinder.md).
