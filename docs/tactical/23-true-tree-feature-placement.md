# Tactical 23 — True tree feature placement

Port the narrow 1.17.1 oak tree path needed to remove the remaining handwritten smoke-scene canopy. This slice is the real `TreeFeature` fanout: `TreeConfiguration`, the first trunk/foliage placer set, bounding-box/leaf post-processing, and the smallest support surface those classes expect.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/TreeFeature.java` | `src/worldgen/levelgen/feature/tree-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/TreeConfiguration.java` | `src/worldgen/levelgen/feature/configurations/tree-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/TrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/StraightTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/straight-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/FoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/BlobFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/blob-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/featuresize/FeatureSize.java` | `src/worldgen/levelgen/feature/featuresize/feature-size.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/featuresize/TwoLayersFeatureSize.java` | `src/worldgen/levelgen/feature/featuresize/two-layers-feature-size.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/TreeDecorator.java` | `src/worldgen/levelgen/feature/treedecorators/tree-decorator.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`OAK`) | `src/worldgen/levelgen/feature/tree-features.ts`, `src/worldgen/levelgen/smoke-feature-placement.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/structure/BoundingBox.java` | `src/world/level/levelgen/structure/bounding-box.ts` |
| `reference/.../src/net/minecraft/world/phys/shapes/DiscreteVoxelShape.java` | `src/world/phys/shapes/discrete-voxel-shape.ts` |
| `reference/.../src/net/minecraft/world/phys/shapes/BitSetDiscreteVoxelShape.java` | `src/world/phys/shapes/bit-set-discrete-voxel-shape.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/structure/templatesystem/StructureTemplate.java` (`updateShapeAtEdge`) | `src/world/level/levelgen/structure/templatesystem/structure-template.ts` |
| `reference/.../src/net/minecraft/core/AxisCycle.java` | `src/core/axis-cycle.ts`, `src/core/direction.ts` |

## What landed

- The translated tree core is now in place: `TreeFeature`, `TreeConfiguration`, `StraightTrunkPlacer`, `BlobFoliagePlacer`, `FeatureSize`, `TwoLayersFeatureSize`, and the empty tree-decorator base.
- The tree path’s support surface landed too: `LevelSimulatedReader`, block-tag checks for `DIRT` / `LOGS` / `LEAVES`, `BoundingBox`, discrete voxel shape traversal, and `StructureTemplate.updateShapeAtEdge(...)`.
- `Block` / `BlockState` now expose the translated `updateShape(...)` plumbing the post-placement edge update pass expects, and `StaticRenderLevel` now implements `isStateAtPosition(...)`.
- `tree-features.ts` mirrors the vanilla `Features.OAK` config from 1.17.1 using the translated providers/placers and the real `ignoreVines()` builder flag.
- The smoke harness dropped the handwritten oak canopy. Both smoke-scene trees now place through `TreeFeatures.OAK` with deterministic feature seeds, so logs, leaves, and leaf-distance propagation all come from translated feature code.
- Focused coverage landed in `test/worldgen/levelgen/feature/tree-feature.test.ts`, which exercises oak placement, translated trunk height, and leaf-distance updates.

## Scope choice

- Landed here: the narrow oak path needed to replace the manual smoke canopy and prove the translated tree feature stack can render end to end.
- Explicitly deferred: additional trunk placers, additional foliage placers, vine/beehive/cocoa decorators, and biome-driven tree placement across generated chunks.
- This slice does not yet move trees into real biome decoration. The smoke scene now uses real `TreeFeature`, but generated terrain still is not consuming translated biome vegetation steps.

## Oracle / done-when

**Unit (Vitest):**

- Oak placement succeeds on a translated flat grass surface.
- The trunk height matches the translated `StraightTrunkPlacer` random draw for a fixed seed.
- Leaves are emitted and their `DISTANCE` values are updated below the default decay value.

**Browser smoke (Playwright + system Chrome):**

- The final frame still renders through the camera-driven generated-world path.
- The screenshot visibly includes the translated oak canopy instead of the old handwritten blob.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 23 as done

## Next

Tactical 24: biome vegetation decoration bridge. The renderer can now draw real oak output from translated `TreeFeature`, but generated terrain still only gets trees and plants from the smoke harness. The next slice should port the narrow biome-decoration path needed to run translated vegetation features against generated chunks, starting with the first `RandomFeature` / selector consumers and the biome feature lists that place surface trees and simple plants in the overworld.
