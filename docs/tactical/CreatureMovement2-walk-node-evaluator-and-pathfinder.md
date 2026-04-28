# CreatureMovement2 - Walk node evaluator and pathfinder

Status: landed as a unit-tested path search foundation; live generated mobs still use temporary direct steering until `CreatureMovement3`.

This tactical follows [`CreatureMovement1-path-node-foundation.md`](CreatureMovement1-path-node-foundation.md) and depends on [`BlockCollision0-render-occlusion-vs-collision-shapes.md`](BlockCollision0-render-occlusion-vs-collision-shapes.md). It ports the vanilla path search core and the ground-mob node evaluator against the host's loaded block/collision view.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Search loop, target selection, node expansion, and path reconstruction | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/PathFinder.java` |
| Base evaluator lifecycle and node helpers | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/NodeEvaluator.java` |
| Ground block classification, floor level, collision checks, malus handling | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/WalkNodeEvaluator.java` |
| Navigation search world window | `reference/minecraft-1.17.1/src/net/minecraft/world/level/PathNavigationRegion.java` |
| Path computation type and block traversal predicates | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/PathComputationType.java` |

## Vanilla Algorithm Specifics

`PathFinder` is not greedy direct steering. Vanilla uses bounded weighted A*-style graph search over `WalkNodeEvaluator` nodes:

- `BinaryHeap` is the open set, ordered by each node's `f` cost.
- `g` is accumulated path cost: previous `g` plus Euclidean neighbor distance plus the neighbor's `BlockPathTypes` malus.
- `h` is Euclidean distance to the best target, multiplied by vanilla's `1.5F` `FUDGING` factor before adding to `g`.
- `f = g + h`; an already-open node is reprioritized when a cheaper `g` is found.
- Search stops when a target is within Manhattan `accuracy`, when the open set is empty, or when `maxVisitedNodes * searchDepthMultiplier` is reached.
- If no target is reached, vanilla reconstructs the nearest partial path using each `Target`'s tracked best node, ordered by `Path.getDistToTarget()` and then node count.
- `WalkNodeEvaluator` owns the reusable ground-node semantics: start-node Y selection, four cardinal neighbors plus diagonals, one-block step-up checks, fall/open-air descent limits, collision checks for tight step movement, and `BlockPathTypes` classification for walkable, blocked, water, leaves, fences, traps, doors, rails, cactus, berry bushes, fire, lava, honey, and cocoa.

## Landed Scope

- Add a host-neutral `PathNavigationRegion` adapter over loaded chunks/block states.
- Port `NodeEvaluator` and `WalkNodeEvaluator` directly enough for common passive land mobs, using shared collision shapes rather than a mob-only solidity shortcut.
- Port `PathFinder.findPath(...)` and path reconstruction on top of the `CreatureMovement1` `Path`, `Node`, `Target`, and `BinaryHeap` primitives.
- Preserve vanilla malus defaults and `BlockPathTypes` behavior for open, blocked, water, fence, danger, damage, and walkable nodes where the required block tags/materials already exist.
- Add block `isPathfindable(...)` plumbing for land/water/air path computation, including water and slab overrides where vanilla differs from the default collision-shape rule.
- Add unit tests against small synthetic worlds for flat ground, one-block step, blocked walls, dry routing around water malus, partial path behavior, and common passive-mob block path type classification.

## Deliberate Deferrals

- Door opening/breaking, rails, fence gates, trapdoors, campfire `LIT`, and related block-state properties where those block classes/properties are not ported yet. The evaluator keeps vanilla-shaped branches so those become data follow-throughs instead of search rewrites.
- Full collision shapes for every special block beyond the minimal shared shape system from `BlockCollision0`; current coverage is enough for passive land mobs over ordinary full blocks, slabs, leaves, plants, water, cactus, berry bushes, lava/fire-like blocks by tag/location, honey by location, and cocoa.
- Mob-specific pathfinding malus overrides beyond the passive animal defaults already required by cows, pigs, sheep, and chickens.
- Live `GroundPathNavigation` path following. This slice produces vanilla-shaped paths in tests but does not replace `SimpleGroundPathNavigation`.

## Validation

```bash
pnpm test -- test/world/level/pathfinder/path-node-foundation.test.ts test/world/level/pathfinder/walk-node-pathfinder.test.ts
pnpm --silent typecheck
```

## Next Step

Proceed to [`CreatureMovement3-ground-path-navigation-integration.md`](CreatureMovement3-ground-path-navigation-integration.md).
