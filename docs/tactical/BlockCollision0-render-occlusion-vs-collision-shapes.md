# BlockCollision0 - Render occlusion vs collision shapes

Status: landed shared movement prerequisite.

This tactical splits block render occlusion from block collision shape semantics before deeper player or creature movement work depends on them. It is a shared environment slice: player prediction, host-authoritative player movement, mob travel, mob pathfinding, spawn placement, suffocation, and block support checks should all read the same block collision facts.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Block behavior defaults, `hasCollision`, `canOcclude`, shape accessors, cached collision shape | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockBehaviour.java` |
| Block face rendering and shape helpers | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java` |
| Block collision iteration over world blocks | `reference/minecraft-1.17.1/src/net/minecraft/world/level/CollisionGetter.java`, `CollisionSpliterator.java` |
| Shared entity `move(...)`, block/entity/world-border collision, step-up, and `onGround` | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Shared living movement branch, friction, gravity, and `Entity.move(...)` handoff | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Player-specific travel wrapper over shared living movement | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java` |
| Mob movement intent and jump decision from collision shape | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java` |
| Mob floor level and path collision checks | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/WalkNodeEvaluator.java` |
| Ground navigation stability and path following | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java`, `GroundPathNavigation.java` |
| Vanilla examples that separate occlusion from collision | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/SlabBlock.java`, `LeavesBlock.java`, `BushBlock.java`, `Blocks.java` |

## Vanilla Model

Vanilla keeps these concepts separate:

- `BlockBehaviour.Properties.hasCollision` decides whether `getCollisionShape(...)` returns the block shape or `Shapes.empty()`.
- `BlockBehaviour.Properties.canOcclude` controls render/light occlusion shape caching. `noOcclusion()` only sets `canOcclude = false`.
- `noCollission()` sets both `hasCollision = false` and `canOcclude = false`.
- The default block `getShape(...)` is a full cube. The default `getCollisionShape(...)` is that shape when `hasCollision` is true.
- `isCollisionShapeFullBlock(...)` is computed from `state.getCollisionShape(...)`, not from `state.canOcclude()`.
- `CollisionGetter.getBlockCollisions(...)` iterates collision shapes and intersects shapes against entity AABBs; it does not filter by render occlusion.
- `Entity.move(...)` is the shared collision resolution path for players and mobs. `LivingEntity.travel(...)` computes movement inputs/friction/gravity and then calls `move(...)`; `Player.travel(...)` adds player-specific swimming/flying/stat hooks around the same shared living path.
- Mob navigation/pathfinding also reads block collision facts: `WalkNodeEvaluator.getFloorLevel(...)` uses the collision shape below the node, and `MoveControl` checks the current block collision shape when deciding whether to jump.

## Fixed Gap

`mclone` previously conflated render occlusion with collision solidity:

- `src/runtime/movement/collision-world.ts` only emits collision boxes for blocks where `state.isCollisionShapeFullBlock(...)` is true.
- `src/world/level/block/state/block-behaviour.ts` currently implements `isCollisionShapeFullBlock(...)` as `state.canOcclude()`.
- Leaves are registered with `.noOcclusion()` so adjacent faces render correctly.
- Therefore leaves are non-occluding for rendering and also accidentally non-colliding for movement.

That is not vanilla-correct. Leaves should be non-occluding for rendering, but still have normal block collision because they are not registered with `noCollission()`.

## Landed Scope

Implemented the minimum vanilla-shaped block collision foundation needed by both player and creature movement:

- Added block shape accessors on `BlockBehaviour` / `BlockState`: `getShape(...)`, `getCollisionShape(...)`, `getOcclusionShape(...)`, and `getBlockSupportShape(...)`.
- Added a small runtime `VoxelShape` / shape-box representation sufficient for empty, full cube, and simple box shapes.
- Made default block collision shape full cube when `hasCollision` is true and empty when `hasCollision` is false.
- Made `isCollisionShapeFullBlock(...)` derive from collision shape fullness, not `canOcclude()`.
- Kept `canOcclude()` as render/light occlusion data.
- Updated `blockGetterCollisionWorld(...)` to emit translated collision-shape AABBs, not only full-block AABBs.
- Added focused block-state tests:
  - leaves: `canOcclude() === false`, collision shape non-empty/full cube, included in collision queries
  - plants/air/water/seagrass-style `noCollission()` blocks: collision shape empty, skipped by collision queries
  - slabs: `canOcclude() === false`, collision shape non-empty, not full cube unless double slab
- Added movement collision tests proving players cannot pass through leaves, can pass through no-collision plants, and collide with the correct slab height.

## Shared Movement Boundary

This slice should be consumed by both movement families:

| Layer | Shared? | Notes |
|---|---|---|
| Block `hasCollision`, `canOcclude`, collision shape, occlusion shape | yes | One authoritative block-state model for all systems. |
| Collision query over loaded block shapes | yes | Player prediction, host movement, mob travel, pathfinding, spawn obstruction, and suffocation should share it. |
| Entity AABB clipping, step-up, `onGround` | yes | Long-term home is a shared entity/body movement path inspired by `Entity.move(...)`. |
| Movement intent production | no | Players produce input commands; mobs produce `MoveControl`/AI intent. |
| Prediction/reconciliation | player-specific | Mobs do not need high-rate client replay. |
| Pathfinding malus and navigation | mob-specific | Uses the shared block collision facts but remains in creature movement tacticals. |

Do not patch only leaves or only cows. The fix is a shared block collision semantics slice.

## Deliberate Deferrals

- Full vanilla `Shapes` boolean operations and arbitrary voxel subdivision.
- Entity-vs-entity collision.
- World-border collision.
- Entity-aware `CollisionContext` behavior such as powder snow, sneaking edge cases, and fluid-specific contexts.
- Complete per-block collision shape parity for fences, doors, trapdoors, ladders, cactus, snow layers, lily pads, vines, rails, and every special block.
- Replacing the player movement integrator with a direct `Entity.move(...)` port.
- Replacing `SimpleGroundPathNavigation` or implementing `WalkNodeEvaluator`.

The shape API should leave room for those follow-ups without making the first slice a full physics port.

## Validation

```bash
pnpm test -- test/world/level/block test/runtime/movement
pnpm --silent typecheck
git diff --check
```

Browser validation was not required because the implementation changes collision data and unit-visible block-state semantics, not rendered pixels.

## Done When

- `noOcclusion()` no longer implies no collision.
- `noCollission()` remains the way to make a block non-colliding.
- `blockGetterCollisionWorld(...)` consumes collision shapes instead of render occlusion.
- Leaves, plants, and slabs are covered by tests that would fail under the current `canOcclude()` shortcut.
- `CreatureMovement2` and `CreatureMovement3` can rely on a shared block collision layer instead of inventing mob-only solidity rules.

## Next Step

Proceed to [`CreatureMovement1-path-node-foundation.md`](CreatureMovement1-path-node-foundation.md). After that, [`CreatureMovement2-walk-node-evaluator-and-pathfinder.md`](CreatureMovement2-walk-node-evaluator-and-pathfinder.md) can rely on this shared block collision model.
