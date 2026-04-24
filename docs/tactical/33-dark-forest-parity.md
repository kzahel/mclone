# Tactical 33 — Dark-forest parity

Finish the first true dark-forest biome-identity slice now that the current carver and surface-material matrix is broad enough to stop blocking it. This slice ports the missing dark-oak tree path, the first huge-mushroom feature path, the dark-oak placement decorator, and the dark-forest / dark-forest-hills biome tables so those biomes stop falling back to the reduced generic forest mix.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/DarkOakTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/dark-oak-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/DarkOakFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/dark-oak-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/featuresize/ThreeLayersFeatureSize.java` | `src/worldgen/levelgen/feature/featuresize/three-layers-feature-size.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/{AbstractHugeMushroomFeature,HugeBrownMushroomFeature,HugeRedMushroomFeature}.java` | `src/worldgen/levelgen/feature/{abstract-huge-mushroom-feature,huge-brown-mushroom-feature,huge-red-mushroom-feature}.ts` |
| `reference/.../src/net/minecraft/world/level/block/HugeMushroomBlock.java` | `src/world/level/block/huge-mushroom-block.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/DarkOakTreePlacementDecorator.java` | `src/worldgen/levelgen/placement/dark-oak-tree-placement-decorator.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{Features,BiomeDefaultFeatures}.java` | `src/worldgen/levelgen/feature/{features,tree-features,vegetation-features}.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`darkForestBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{dark_oak_log,dark_oak_leaves,dark_oak_sapling,brown_mushroom_block,red_mushroom_block,mushroom_stem}.json` | `src/world/level/generated-render-blocks.ts`, `src/renderer/block/block-colors.ts` |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/browser/probes/dark-forest-parity.probe.ts` | same |

## What landed

- `DarkOakTrunkPlacer`, `DarkOakFoliagePlacer`, and `ThreeLayersFeatureSize` are now direct TS ports, and `TreeFeatures.DARK_OAK` matches the vanilla dark-oak tree config instead of falling back to the oak/birch mix.
- The first huge-mushroom path is live: `HugeMushroomBlock`, `HugeBrownMushroomFeature`, `HugeRedMushroomFeature`, and `HugeMushroomFeatureConfiguration` now place vanilla-shaped caps and stems with the directional mushroom-block state toggles the renderer expects.
- `DarkOakTreePlacementDecorator` is ported and registered, so dark-forest vegetation uses the same one-position-per-4x4-cell distribution pattern as vanilla instead of generic square/count placement.
- `VegetationFeatures` now has dark-forest random selectors that mix dark oak, birch, fancy oak, and huge mushrooms with the same weight split vanilla uses for dark forest versus dark-forest hills.
- `overworld-biome-generation-settings.ts` now has real translated tables for `minecraft:dark_forest` and `minecraft:dark_forest_hills` instead of leaving those keys on the empty-generation-settings fallback.
- The generated render palette now includes dark-oak logs/leaves/saplings plus brown/red mushroom blocks and mushroom stems, and dark-oak leaves use the same foliage tint path as oak leaves.

## Scope choice

- Landed here: the minimum dark-forest identity slice that needed real new tree and feature ports, not just another biome-table entry.
- Kept intentionally narrow: no vine-decorator consumers, no bee decorators, and no broader jungle/acacia ecosystem work. Those are separate parity slices with different missing classes.
- Also kept narrow: huge mushrooms only for the overworld dark-forest path. This is the first translated huge-mushroom feature family, not the full mushroom-field biome table.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves the translated dark-oak path places the 2x2 trunk and dark-oak foliage shape instead of silently using an older oak-like fallback.
- `vegetation-parity.test.ts` proves the huge-brown and huge-red mushroom features place translated mushroom blocks/stems, and that the dark-forest selector only emits the intended dark-forest output families.
- biome-settings wiring tests prove `dark_forest` and `dark_forest_hills` now route through translated `RANDOM_SELECTOR` vegetation entries instead of the empty fallback.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/dark-forest-parity.probe.ts` passes.
- `/tmp/mclone-debug-dark-forest.png` is manually inspected for a dark canopy with the translated dark-oak / huge-mushroom block set present in the generated frame.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for the dark-oak / huge-mushroom / generated-render-level suites
- `pnpm probe:browser -- test/browser/probes/dark-forest-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` all stop describing dark forest as upcoming work

## Next

That next biome-identity follow-through is now [`34-savanna-parity.md`](34-savanna-parity.md): savanna landed through the translated acacia tree path and the savanna / shattered-savanna biome tables. The next slice after that should move to jungle parity.
