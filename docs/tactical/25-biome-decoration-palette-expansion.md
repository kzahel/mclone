# Tactical 25 — Biome decoration palette expansion

Expand the translated biome-decoration palette beyond the first mountain/taiga tree-and-grass bridge. This slice ports the remaining simple/double-plant and column-style vegetation consumers that the current generated-world renderer can already display, then removes the last manual smoke-harness water patch by framing a naturally generated shoreline instead of injecting translucent blocks into the level.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/block/DoublePlantBlock.java` | `src/world/level/block/double-plant-block.ts`, `src/world/level/block/state/properties/double-block-half.ts` |
| `reference/.../src/net/minecraft/world/level/block/SweetBerryBushBlock.java` | `src/world/level/block/sweet-berry-bush-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/MushroomBlock.java` | `src/world/level/block/mushroom-block.ts`, `src/tags/block-tags.ts`, `src/world/level/static-render-level.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/blockplacers/DoublePlantPlacer.java` | `src/worldgen/levelgen/feature/blockplacers/double-plant-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/blockplacers/ColumnPlacer.java` | `src/worldgen/levelgen/feature/blockplacers/column-placer.ts` |
| `reference/.../src/net/minecraft/util/valueproviders/BiasedToBottomInt.java` | `src/util/valueproviders/biased-to-bottom-int.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/ChanceDecorator.java` + `ChanceDecoratorConfiguration.java` | `src/worldgen/levelgen/placement/chance-decorator.ts`, `src/worldgen/levelgen/feature/configurations/chance-decorator-configuration.ts`, `src/worldgen/levelgen/feature/configured-feature.ts`, `src/worldgen/levelgen/placement/configured-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/Spread32Decorator.java` | `src/worldgen/levelgen/placement/spread-32-above-decorator.ts`, `src/worldgen/levelgen/placement/feature-decorators.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`PATCH_LARGE_FERN`, `PATCH_BERRY_*`, `PATCH_PUMPKIN`, `PATCH_SUGAR_CANE*`, `PATCH_CACTUS*`, `BROWN_MUSHROOM_*`, `RED_MUSHROOM_*`) | `src/worldgen/levelgen/feature/vegetation-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addFerns`, `addSparseBerryBushes`, `addDefaultMushrooms`, `addDefaultExtraVegetation`, `addBadlandExtraVegetation`, `addDesertExtraVegetation`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`mountainBiome`, `taigaBiome`, `desertBiome`, `badlandsBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/client/color/block/BlockColors.java` (`LARGE_FERN`) | `src/renderer/block/block-colors.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{large_fern,sweet_berry_bush,brown_mushroom,red_mushroom,pumpkin}.json` | `src/world/level/generated-render-blocks.ts`, browser smoke via existing model/atlas loaders |

## What landed

- Property/state support expanded with `DoubleBlockHalf`, `BlockStateProperties.DOUBLE_BLOCK_HALF`, and `BlockStateProperties.AGE_3`, plus the generated-world block classes `DoublePlantBlock`, `SweetBerryBushBlock`, and `MushroomBlock`.
- The translated placement helpers landed directly from 1.17.1: `DoublePlantPlacer`, `ColumnPlacer`, `BiasedToBottomInt`, `ChanceDecorator`, `SPREAD_32_ABOVE`, and the small `Decoratable` convenience surface (`rarity(...)`, `countRandom(...)`) they require.
- `VegetationFeatures` now includes the deferred simple/double-plant and column patches the current overworld biomes use: `PATCH_LARGE_FERN`, `PATCH_BERRY_SPARSE`, `PATCH_BERRY_DECORATED`, `PATCH_PUMPKIN`, `PATCH_SUGAR_CANE*`, `PATCH_CACTUS*`, `BROWN_MUSHROOM_*`, and `RED_MUSHROOM_*`.
- Overworld biome generation settings now wire those features into the translated mountain/taiga path, and the first desert/badlands extra-vegetation tables are in place so column features are no longer mountain/taiga-only.
- The generated render palette picked up the new block registrations, sprite requirements, and render-layer routing for `large_fern`, `sweet_berry_bush`, `brown_mushroom`, `red_mushroom`, and `pumpkin`, with `BlockColors` now honoring the `DoublePlantBlock.HALF` tint rule for large ferns.
- The browser smoke no longer injects a manual water patch. Instead it uses a naturally generated taiga shoreline so `solid`, `cutout`, and `translucent` all come from translated terrain plus biome decoration.

## Scope choice

- Landed here: the remaining vegetation consumers already needed by translated taiga/mountain/desert/badlands decoration, plus the palette and smoke-harness changes needed to render them.
- Explicitly deferred: real `LakeFeature` / `SpringFeature` placement, swamp water-surface mutation from the old terrain slice, lily pads, and the broader forest/plains/swamp biome-decoration tables.
- Also deferred: the full block-interaction behaviors on the new blocks (`use`, growth ticks, entity damage, bonemeal); this slice only ports the state/survival surface the worldgen and renderer paths need.

## Oracle / done-when

**Unit (Vitest):**

- Large fern placement writes matching lower/upper halves through the translated double-plant path.
- Berry bushes place with the translated `age=3` state and sugar cane patches place stacked columns through `ColumnPlacer`.
- Generated-world surface parity tests still pass once the expanded decoration block set is ignored.

**Browser smoke (Playwright + system Chrome):**

- The generated-world frame still renders through `GameRenderer -> LevelRenderer -> ChunkRenderDispatcher`.
- `solid`, `cutout`, and `translucent` all submit without the old manual water patch.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 25 as done

## Next

Tactical 26: overworld water and swamp decoration. The next slice should replace the shoreline-only workaround with translated `LakeFeature` / `SpringFeature`, port swamp surface mutation, add lily pads and the remaining forest/plains/swamp decoration consumers, and move the browser smoke to a real swamp frame.
