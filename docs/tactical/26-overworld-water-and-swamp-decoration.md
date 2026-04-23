# Tactical 26 — Overworld water and swamp decoration

Port the real overworld water-feature bridge instead of relying on terrain-only shorelines. This slice lands translated `LakeFeature` / `SpringFeature`, the minimal decorator stack they depend on, swamp surface mutation from the old numeric surface path, and the first forest/plains/swamp biome tables so natural water and lily pads come from translated worldgen instead of camera-picked terrain.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/LakeFeature.java` | `src/worldgen/levelgen/feature/lake-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/SpringFeature.java` | `src/worldgen/levelgen/feature/spring-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/SimpleRandomSelectorFeature.java` + `SimpleRandomFeatureConfiguration.java` | `src/worldgen/levelgen/feature/simple-random-selector-feature.ts`, `src/worldgen/levelgen/feature/configurations/simple-random-feature-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/CountNoiseDecorator.java` + `NoiseDependantDecoratorConfiguration.java` | `src/worldgen/levelgen/placement/count-noise-decorator.ts`, `src/worldgen/levelgen/feature/configurations/noise-dependant-decorator-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/RangeDecorator.java` + `VerticalDecorator.java` + `RangeDecoratorConfiguration.java` | `src/worldgen/levelgen/placement/range-decorator.ts`, `src/worldgen/levelgen/placement/vertical-decorator.ts`, `src/worldgen/levelgen/feature/configurations/range-decorator-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/heightproviders/{UniformHeight,BiasedToBottomHeight}.java` + `VerticalAnchor.java` | `src/worldgen/carver/carver-config.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/{BlockStateConfiguration,SpringConfiguration}.java` | `src/worldgen/levelgen/feature/configurations/block-state-configuration.ts`, `src/worldgen/levelgen/feature/configurations/spring-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/block/WaterlilyBlock.java` | `src/world/level/block/waterlily-block.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/surfacebuilders/SwampSurfaceBuilder.java` | `src/worldgen/surface/surface-builders.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`LAKE_WATER`, `SPRING_WATER`, `PATCH_WATERLILLY`, `PATCH_TALL_GRASS_2`, `FLOWER_SWAMP`, `FOREST_FLOWER_VEGETATION`, `PLAIN_VEGETATION`, `TREES_SWAMP`) | `src/worldgen/levelgen/feature/water-features.ts`, `src/worldgen/levelgen/feature/vegetation-features.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addDefaultLakes`, `addDefaultSprings`, `addForestFlowers`, `addForestGrass`, `addPlainVegetation`, `addPlainGrass`, `addSwampVegetation`, `addSwampExtraVegetation`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`baseForestBiome`, `plainsBiome`, `swampBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts`, `src/worldgen/biome/biome-data.ts` |
| `reference/.../src/net/minecraft/client/color/block/BlockColors.java` (`TALL_GRASS`, `LILY_PAD`) | `src/renderer/block/block-colors.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{blue_orchid,lily_pad,tall_grass,lilac,rose_bush,peony,lily_of_the_valley}.json` | `src/world/level/generated-render-blocks.ts`, browser smoke via existing model/atlas loaders |

## What landed

- Translated water-feature plumbing is in: `LakeFeature`, `SpringFeature`, `BlockStateConfiguration`, `SpringConfiguration`, `RangeDecorator`, `CountNoiseDecorator`, `SimpleRandomSelectorFeature`, and the small height-provider helpers those decorators need.
- The level/gen scaffolding those features depend on now exists in TS: `BaseStoneSource`, no-op block/liquid tick access on `WorldGenLevel`, `FluidState.createLegacyBlock()`, runtime biome lookup on `GeneratedRenderLevel`, and the minimal `Biome.shouldFreeze(...)` check used by the translated lake path.
- The generated render palette now includes `waterlily`, `blue_orchid`, `tall_grass`, `lilac`, `rose_bush`, `peony`, and `lily_of_the_valley`, with `WaterlilyBlock` survival rules, render-layer wiring, and `BlockColors` support for `tall_grass` and `lily_pad`.
- `VegetationFeatures` now covers the first remaining forest/plains/swamp consumers from vanilla 1.17.1 that fit the current renderer surface: `PATCH_GRASS_FOREST`, `PATCH_GRASS_PLAIN`, `PATCH_GRASS_NORMAL`, `PATCH_TALL_GRASS_2`, `PATCH_WATERLILLY`, `FLOWER_SWAMP`, `FOREST_FLOWER_VEGETATION`, `PLAIN_VEGETATION`, `TREES_SWAMP`, and the swamp mushroom aliases.
- `WaterFeatures` wires the translated `LAKE_WATER` and `SPRING_WATER` configured-feature definitions, and overworld biome generation settings now assign the first forest/plains/swamp tables while also giving the existing mountain/taiga/desert/badlands bridge a real water-feature pass.
- The old `tactical 07` swamp throw is gone. `surface-builders.ts` now ports the `SwampSurfaceBuilder` mutation so swamp columns can promote their y=62 water surface before the normal grass builder runs.
- The browser smoke moved off the old shoreline framing and now targets a real swamp region. The inspected screenshot shows the translated swamp surface with a visible lily pad rather than a harness-authored water patch.

## Scope choice

- Landed here: the water-feature path and the first biome tables that make those features matter visually in generated chunks.
- Kept narrow on purpose: only `LAKE_WATER` and `SPRING_WATER` from this family. Lava lakes, lava springs, and the rest of the range/height-provider zoo still stay out of the bridge.
- Also deferred: `PATCH_DEAD_BUSH`, `SEAGRASS_SWAMP`, `FLOWER_PLAIN_DECORATED`, `FOREST_FLOWER_VEGETATION_COMMON`, vine-decorated swamp oaks, and the broader birch/dark-forest/plains parity tables. Those need more plant providers, decorators, or block palette before they are worth landing.
- Known simplifications: the current lake port still skips the structure-start village rejection because the bridge still has no structure-start pipeline to consult. The later mycelium/ice follow-up paths are now landed through the widened surface/render path and targeted tests.

## Oracle / done-when

**Unit (Vitest):**

- `LakeFeature` carves a translated water cavity out of solid stone.
- `SpringFeature` places a translated source block when the rock/hole counts match.
- `WaterlilyBlock` survives and places on translated surface water.
- Expanded block-color/render-layer tests cover `tall_grass` and `lily_pad`.

**Browser smoke (Playwright + system Chrome):**

- Generated chunks still render through `GameRenderer -> LevelRenderer -> ChunkRenderDispatcher`.
- `solid`, `cutout`, and `translucent` all submit from translated generated content.
- The screenshot is inspected manually before continuing; this slice’s validated frame is a swamp surface with a visible lily pad.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 26 as done

## Next

Tactical 27 is now [`27-biome-decoration-parity-follow-through.md`](27-biome-decoration-parity-follow-through.md): finish the missing flower-provider, dead-bush, seagrass, birch, and flower-forest bridge first. After the later carver parity follow-through in tacticals 28-32, the next broad biome-identity slice is dark forest.
