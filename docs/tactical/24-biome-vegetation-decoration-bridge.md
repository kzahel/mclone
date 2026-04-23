# Tactical 24 — Biome vegetation decoration bridge

Port the narrow 1.17.1 biome-decoration path needed so generated chunks can place their own trees and simple vegetation instead of relying on the smoke harness. Keep this slice tight: chunk-level `applyBiomeDecoration(...)`, biome generation settings, the first selector/decorator helpers, and the first real mountain/taiga vegetation tables that the pinned smoke seed actually uses.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/chunk/ChunkGenerator.java` (`applyBiomeDecoration`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/world/level/generated-render-level.ts` |
| `reference/.../src/net/minecraft/world/level/biome/BiomeGenerationSettings.java` | `src/worldgen/biome/biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` (`generate`) | `src/worldgen/biome/biome.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/GenerationStep.java` | `src/worldgen/levelgen/generation-step.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/Decoratable.java` | `src/worldgen/levelgen/feature/configured-feature.ts`, `src/worldgen/levelgen/placement/configured-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/WeightedConfiguredFeature.java` | `src/worldgen/levelgen/feature/weighted-configured-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/RandomFeatureConfiguration.java` | `src/worldgen/levelgen/feature/configurations/random-feature-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/RandomSelectorFeature.java` | `src/worldgen/levelgen/feature/random-selector-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/CountWithExtraChanceDecorator.java` | `src/worldgen/levelgen/placement/count-with-extra-chance-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/FrequencyWithExtraChanceDecoratorConfiguration.java` | `src/worldgen/levelgen/feature/configurations/frequency-with-extra-chance-decorator-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/HeightmapDoubleDecorator.java` | `src/worldgen/levelgen/placement/heightmap-spread-double-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/WaterDepthThresholdDecorator.java` | `src/worldgen/levelgen/placement/water-depth-threshold-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/WaterDepthThresholdConfiguration.java` | `src/worldgen/levelgen/feature/configurations/water-depth-threshold-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/SpruceFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/spruce-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/PineFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/pine-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/FancyTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/fancy-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/FancyFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/fancy-foliage-placer.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addMountainTrees`, `addTaigaTrees`, `addDefaultGrass`, `addTaigaGrass`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`mountainBiome`, `taigaBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`SPRUCE`, `PINE`, `FANCY_OAK`, `TAIGA_VEGETATION`, `TREES_MOUNTAIN`, `TREES_MOUNTAIN_EDGE`, `PATCH_GRASS_*`) | `src/worldgen/levelgen/feature/tree-features.ts`, `src/worldgen/levelgen/feature/vegetation-features.ts` |

## What landed

- Generated chunks now own a translated biome-decoration pass: `NoiseBasedChunkGenerator.applyBiomeDecoration(...)`, chunk-center primary-biome sampling, decoration seeding, and `GeneratedRenderLevel` chunk guards so each loaded chunk decorates once.
- `BiomeGenerationSettings`, `GenerationStep.Decoration`, and the narrowed `Biome.generate(...)` feature loop are in place, with lazy configured-feature suppliers so biome data can point at translated configured features without forcing block-registry timing issues at module load.
- The first selector/decorator helpers landed: `WeightedConfiguredFeature`, `RandomFeatureConfiguration`, `RandomSelectorFeature`, `COUNT_EXTRA`, `HEIGHTMAP_SPREAD_DOUBLE`, and `WATER_DEPTH_THRESHOLD`, plus `count(...)`, `squared()`, and `weighted(...)` helpers on the configured feature/decorator path.
- The tree palette expanded to the first real biome-driven mountain/taiga set: `SPRUCE`, `PINE`, and `FANCY_OAK`, backed by `SpruceFoliagePlacer`, `PineFoliagePlacer`, `FancyTrunkPlacer`, `FancyFoliagePlacer`, and `UniformInt`.
- The generated render palette now registers spruce log/leaves/sapling assets so those translated tree outputs can bake and render.
- The first translated overworld vegetation tables landed for the smoke seed’s actual biomes: `TAIGA_VEGETATION`, `TREES_MOUNTAIN`, `TREES_MOUNTAIN_EDGE`, `PATCH_GRASS_BADLANDS`, and `PATCH_GRASS_TAIGA_2`, wired into `mountains`, `wooded_mountains`, `mountain_edge`, `taiga`, `taiga_hills`, and `taiga_mountains`.
- `main.ts` no longer uses `smoke-feature-placement.ts` for trees and plants. The generated world now supplies those blocks through chunk decoration. A tiny manual water patch remains in the smoke harness only to keep the translucent chunk layer exercised until lake/surface-decoration worldgen is ported.
- Coverage expanded in `test/renderer/generated-render-level.test.ts`, which now verifies both terrain-surface parity under decorations and the presence of biome-driven vegetation blocks in the loaded generated chunks.

## Scope choice

- Landed here: the narrow biome-decoration bridge needed for the pinned generated-world smoke scene, focused on the mountain/taiga biomes it actually traverses and on vegetation types already present in the render palette.
- Explicitly deferred: large ferns, berry bushes, mushrooms, pumpkins, sugar cane/cactus patch worldgen, and broader biome tables outside the first mountain/taiga set.
- Also deferred: the structure loop inside `Biome.generate(...)`; this slice only ports configured-feature placement.

## Oracle / done-when

**Unit (Vitest):**

- Generated chunk-cache tests still match the committed terrain surface oracle once vegetation blocks are ignored.
- Generated chunks contain translated biome-driven vegetation after camera-centered chunk loads.
- Existing feature/tree tests stay green with the new selector/decorator support in place.

**Browser smoke (Playwright + system Chrome):**

- The frame still renders through the camera-driven generated-world path.
- The screenshot shows the expected green fog/sky clear color with generated terrain below and a darker vegetation/canopy mass rising on the left side of the frame.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 24 as done

## Next

Tactical 25: biome-decoration palette expansion. The generated world now owns the first mountain/taiga tree-and-grass pass, but it still skips large ferns, berry bushes, mushrooms, pumpkins, sugar cane/cactus patches, and broader overworld biome tables, and the smoke harness still pins a tiny manual water patch for translucent coverage. The next slice should port the missing simple/double-plant and column-style feature consumers plus the first natural translucent/surface-decoration source so the remaining harness-authored patch can disappear.
