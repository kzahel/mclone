# Tactical 27 — Biome-decoration parity follow-through

Finish the missing flower / dead-bush / seagrass vegetation path now that lakes, springs, and swamp surface mutation are in. This slice ports the remaining plain/forest flower providers, the first real `Feature.FLOWER` / `Feature.SEAGRASS` consumers, the dead-bush block and configured features, and the broader birch / flower-forest / swamp biome tables that can already ride on the current tree and renderer foundation.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/stateproviders/{PlainFlowerProvider,ForestFlowerProvider}.java` | `src/worldgen/levelgen/feature/stateproviders/{plain-flower-provider,forest-flower-provider}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/{AbstractFlowerFeature,DefaultFlowerFeature,SeagrassFeature}.java` | `src/worldgen/levelgen/feature/{abstract-flower-feature,default-flower-feature,seagrass-feature}.ts` |
| `reference/.../src/net/minecraft/world/level/block/{DeadBushBlock,SeagrassBlock,TallSeagrassBlock}.java` | `src/world/level/block/{dead-bush-block,seagrass-block,tall-seagrass-block}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`FLOWER_DEFAULT`, `FLOWER_FOREST`, `FLOWER_PLAIN_DECORATED`, `PATCH_DEAD_BUSH*`, `SEAGRASS_SWAMP`, `BIRCH_*`, `FOREST_FLOWER_TREES`) | `src/worldgen/levelgen/feature/vegetation-features.ts`, `src/worldgen/levelgen/feature/features.ts`, `src/worldgen/levelgen/feature/tree-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addDefaultFlowers`, `addForestFlowers`, `addPlainVegetation`, `addSwampVegetation`, `addDesertVegetation`, `addBadlandGrass`, `addPlainGrass`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`birchForestBiome`, `plainsBiome`, `baseForestBiome`, `flowerForestBiome`, `swampBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/client/color/block/BlockColors.java` (`SPRUCE_LEAVES`, `BIRCH_LEAVES`) | `src/renderer/block/block-colors.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{birch_log,birch_leaves,birch_sapling,poppy,allium,azure_bluet,red_tulip,orange_tulip,white_tulip,pink_tulip,oxeye_daisy,cornflower,dead_bush,seagrass,tall_seagrass}.json` | `src/world/level/generated-render-blocks.ts`, browser smoke via existing model/atlas loaders |

## What landed

- The translated provider / feature layer is now in place for vanilla flowers and seagrass: `PlainFlowerProvider`, `ForestFlowerProvider`, `DefaultFlowerFeature`, `SeagrassFeature`, and `ProbabilityFeatureConfiguration`.
- The supporting block layer is now present too: `DeadBushBlock`, `SeagrassBlock`, and `TallSeagrassBlock`, with the survival / waterlogging behavior the current worldgen and renderer paths actually exercise.
- `VegetationFeatures` now covers the missing 1.17.1 configured features from this family that fit the current tree/render scope: `FLOWER_DEFAULT`, `FLOWER_FOREST`, `FLOWER_PLAIN`, `FLOWER_PLAIN_DECORATED`, `FOREST_FLOWER_VEGETATION_COMMON`, `PATCH_DEAD_BUSH`, `PATCH_DEAD_BUSH_2`, `PATCH_DEAD_BUSH_BADLANDS`, `SEAGRASS_SWAMP`, `FOREST_FLOWER_TREES`, `TREES_BIRCH`, `BIRCH_TALL`, and `BIRCH_OTHER`.
- The generated render palette now includes birch tree blocks plus the missing flower / dead-bush / seagrass assets, with render-layer wiring and the vanilla spruce / birch leaf block-color rules.
- Overworld biome generation settings no longer leave birch forests and flower forests on the old reduced fallback. Forest, flower-forest, birch, plains, swamp, desert, and badlands now route through the translated configured-feature tables that are already supported by the current port.

## Scope choice

- Landed here: the missing flower-provider, dead-bush, seagrass, birch, and flower-forest parity work that only needed existing tree placers and model plumbing.
- Deferred on purpose: dark-forest parity. The real dark-forest table wants `DarkOakTrunkPlacer`, `DarkOakFoliagePlacer`, huge mushroom features, and the remaining tree-decorator path. Porting only the biome table first would have produced another partial fallback.
- Also still deferred at the time: sunflower-plains specific `PATCH_SUNFLOWER`, vine placement consumers, and bee-decorator variants. The bee-decorator follow-through later landed in [`51-bee-tree-decorator-follow-through.md`](51-bee-tree-decorator-follow-through.md), the sunflower patch follow-through later landed in [`52-sunflower-plains-follow-through.md`](52-sunflower-plains-follow-through.md), and the swamp-oak vine follow-through later landed in [`53-swamp-oak-vine-follow-through.md`](53-swamp-oak-vine-follow-through.md). The next narrow visible-exactness gap in this family is `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK`.

## Oracle / done-when

**Unit (Vitest):**

- `FLOWER_PLAIN_DECORATED` only places translated plain-flower states.
- `FLOWER_FOREST` only places translated forest-flower-provider states.
- Dead-bush and swamp seagrass features place on translated desert/swamp surfaces.
- Biome settings wire the new swamp / flower-forest / birch tables, and palette/render-layer coverage includes the new plant blocks.

**Browser smoke (Playwright + system Chrome):**

- Generated chunks still render through `GameRenderer -> LevelRenderer -> ChunkRenderDispatcher`.
- The smoke frame still submits `solid`, `cutout`, and `translucent`.
- The validated screenshot for this slice is still a swamp frame, now with visible tall seagrass columns and lily pads over the swamp shelf.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 27 as done

## Next

Tactical 28 is now [`28-carver-material-parity-and-oracle-expansion.md`](28-carver-material-parity-and-oracle-expansion.md): finish the next classic-carver parity slice by widening the carved material model, aligning `WorldCarver`'s replaceable-material set, and broadening carved-stage oracle coverage beyond spawn-plus-ocean.
