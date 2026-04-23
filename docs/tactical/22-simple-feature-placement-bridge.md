# Tactical 22 — Simple feature placement bridge

Port the narrow 1.17.1 worldgen feature path needed to stop hand-placing smoke-scene plants. In 1.17.1 this bridge is `Feature` + `ConfiguredFeature` plus `DecoratedFeature` / decorators, not the later `PlacedFeature` API. Keep the scope tight: simple vegetation and column-style plants only. True `TreeFeature` stays deferred because it fans out into trunk placers, foliage placers, bounding boxes, and leaf post-processing.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/Feature.java` | `src/worldgen/levelgen/feature/feature.ts`, `src/worldgen/levelgen/feature/features.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/ConfiguredFeature.java` | `src/worldgen/levelgen/feature/configured-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/FeaturePlaceContext.java` | `src/worldgen/levelgen/feature/feature-place-context.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/DecoratedFeature.java` | `src/worldgen/levelgen/feature/decorated-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/SimpleBlockFeature.java` | `src/worldgen/levelgen/feature/simple-block-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/RandomPatchFeature.java` | `src/worldgen/levelgen/feature/random-patch-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/SimpleBlockConfiguration.java` | `src/worldgen/levelgen/feature/configurations/simple-block-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/RandomPatchConfiguration.java` | `src/worldgen/levelgen/feature/configurations/random-patch-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/DecoratedFeatureConfiguration.java` | `src/worldgen/levelgen/feature/configurations/decorated-feature-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/ConfiguredDecorator.java` | `src/worldgen/levelgen/placement/configured-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/FeatureDecorator.java` | `src/worldgen/levelgen/placement/feature-decorator.ts`, `src/worldgen/levelgen/placement/feature-decorators.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/DecoratedDecorator.java` | `src/worldgen/levelgen/placement/decorated-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/NopePlacementDecorator.java` | `src/worldgen/levelgen/placement/nope-placement-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/CountDecorator.java` + `RepeatingDecorator.java` | `src/worldgen/levelgen/placement/count-decorator.ts`, `src/worldgen/levelgen/placement/repeating-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/SquareDecorator.java` | `src/worldgen/levelgen/placement/square-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/HeightmapDecorator.java` | `src/worldgen/levelgen/placement/heightmap-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/DecorationContext.java` | `src/worldgen/levelgen/placement/decoration-context.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/Heightmap.java` (`Types` subset) | `src/worldgen/levelgen/heightmap.ts` |
| `reference/.../src/net/minecraft/world/level/block/BushBlock.java` | `src/world/level/block/bush-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/CactusBlock.java` | `src/world/level/block/cactus-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/SugarCaneBlock.java` | `src/world/level/block/sugar-cane-block.ts` |
| smoke-scene harness plumbing | `src/worldgen/levelgen/smoke-feature-placement.ts`, `src/renderer/main.ts` |

## What landed

- The minimal 1.17.1 feature core is now ported under `src/worldgen/levelgen/feature`: `Feature`, `ConfiguredFeature`, `FeaturePlaceContext`, `SimpleBlockFeature`, `RandomPatchFeature`, and `DecoratedFeature`.
- The matching decorator path is in place under `src/worldgen/levelgen/placement`: `ConfiguredDecorator`, `DecorationContext`, and the `nope`, `count`, `square`, `heightmap`, and `decorated` decorators needed for simple surface vegetation.
- The translated support types landed too: `BlockStateProvider` + `SimpleStateProvider`, `BlockPlacer` + `SimpleBlockPlacer`, `CountConfiguration`, `HeightmapConfiguration`, `NoneDecoratorConfiguration`, `DecoratedDecoratorConfiguration`, and a minimal `Heightmap.Types` port.
- `StaticRenderLevel` now exposes the worldgen-facing mutator and heightmap hooks the feature path needs, and `BlockState`/`Block` grew the `canSurvive(...)` path used by feature placement.
- `BushBlock`, `CactusBlock`, and `SugarCaneBlock` now enforce the translated survival rules this slice needs for placement-time validation.
- The smoke harness moved its plant/simple-feature placement into `smoke-feature-placement.ts`: grass, fern, dandelion, sapling, cactus, and sugar cane now come through translated feature logic and deterministic feature/decorator seeds instead of direct `setBlock(...)` calls.

## Scope choice

- Landed here: the narrow simple-feature bridge used by surface vegetation and simple vertical plants, plus the smoke-scene swap from hand-placed plants to deterministic translated feature placement.
- Explicitly deferred: `TreeFeature`, `TreeConfiguration`, trunk placers, foliage placers, decorators tied to full biome-decoration steps, and the rest of the broader feature registry surface.
- The smoke canopy remains manual for now. That is deliberate: the real oak path is much larger than the simple-feature bridge and deserves its own slice.

## Oracle / done-when

**Unit (Vitest):**

- `count + square + heightmap` decorator composition returns surface-projected positions in the expected 16x16 region.
- `RandomPatchFeature` places vegetation onto a translated grass surface through `ConfiguredFeature`.
- `SimpleBlockFeature` respects translated sugar-cane survival, including stacked placement.

**Browser smoke (Playwright + system Chrome):**

- The frame still renders generated terrain through the camera-driven chunk path.
- At least one feature-driven surface plant is visibly present in the inspected screenshot.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 22 as done

## Next

Tactical 23: true tree feature placement. Port the narrowest real oak path needed for smoke-scene parity: `TreeFeature`, `TreeConfiguration`, the first trunk placer and foliage placer set, plus the post-placement leaf-distance/update work so the harness can drop the remaining manual canopy and generated scenes can start consuming translated tree output end to end.
