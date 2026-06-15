# Tactical 34 — Savanna parity

Finish the next broad biome-identity slice after dark forest by porting the missing acacia tree path and the savanna biome tables that depend on it. This slice adds `ForkingTrunkPlacer`, `AcaciaFoliagePlacer`, the acacia block/render palette, the savanna and shattered-savanna vegetation selectors, and the `savanna` / `savanna_plateau` / `shattered_savanna` / `shattered_savanna_plateau` biome tables so those biomes stop falling back to the reduced generic tree mix.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/ForkingTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/forking-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/AcaciaFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/acacia-foliage-placer.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`ACACIA`, `TREES_SAVANNA`, `TREES_SHATTERED_SAVANNA`, `FLOWER_WARM`, `PATCH_TALL_GRASS`, `PATCH_GRASS_SAVANNA`) | `src/worldgen/levelgen/feature/{tree-features,vegetation-features}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addSavannaTrees`, `addShatteredSavannaTrees`, `addSavannaGrass`, `addShatteredSavannaGrass`, `addSavannaExtraGrass`, `addWarmFlowers`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`baseSavannaBiome`, `savannaBiome`, `savanaPlateauBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/block/Blocks.java` (`ACACIA_LOG`, `ACACIA_LEAVES`, `ACACIA_SAPLING`) | `src/world/level/generated-render-blocks.ts`, `src/renderer/block/block-colors.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{acacia_log,acacia_leaves,acacia_sapling}.json` | `src/world/level/generated-render-blocks.ts` |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/renderer/generated-render-level.test.ts`, `test/browser/probes/savanna-parity.probe.ts` | same |

## What landed

- `ForkingTrunkPlacer` and `AcaciaFoliagePlacer` are now direct TS ports, and `TreeFeatures.ACACIA` matches the vanilla acacia tree config instead of leaving savanna on the oak fallback path.
- `VegetationFeatures` now includes the savanna-specific selector and plant follow-through the biome table expects: `TREES_SAVANNA`, `TREES_SHATTERED_SAVANNA`, `FLOWER_WARM`, `PATCH_TALL_GRASS`, and `PATCH_GRASS_SAVANNA`.
- `overworld-biome-generation-settings.ts` now has real translated tables for `minecraft:savanna`, `minecraft:savanna_plateau`, `minecraft:shattered_savanna`, and `minecraft:shattered_savanna_plateau` instead of leaving those keys on the empty-generation-settings fallback.
- The generated render palette now includes acacia logs, leaves, and saplings, and acacia leaves ride the standard biome-average foliage tint path.
- Focused tests now pin the acacia trunk/foliage behavior, the savanna selector output family, and the new biome-table wiring, while the browser frame captures a real generated savanna instead of another forest-only validation scene.

## Scope choice

- Landed here: the minimum savanna slice that needed real new tree logic and savanna-specific vegetation tables, not just another biome-key mapping.
- Kept intentionally narrow: no village/outpost structure parity, because the repo still has no structure-start pipeline for those table entries to consult.
- Also kept narrow: no jungle/cocoa/vine decorator work. That is a distinct biome-identity slice with different missing classes and should stay separate from acacia/savanna.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves the translated acacia path places a forking trunk with offset logs and the acacia foliage shape instead of silently behaving like oak.
- `vegetation-parity.test.ts` proves the savanna selector only emits the intended oak/acacia output family and that the savanna biome keys now route through translated vegetation/flower entries instead of the empty fallback.
- `generated-render-level.test.ts` stays green after the acacia palette expansion.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/savanna-parity.probe.ts` passes.
- `/tmp/mclone-debug-savanna.png` is manually inspected for a warm brown-green savanna frame with the translated acacia vegetation path present in the generated world.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for the acacia / savanna / generated-render-level suites
- `pnpm probe:browser -- test/browser/probes/savanna-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` all stop listing savanna or acacia as the next missing biome-identity slice

## Next

Tactical 35 is now landed as [`35-jungle-parity.md`](35-jungle-parity.md), and tactical 36 is now landed as [`36-snowy-giant-taiga-and-mushroom-table-coverage.md`](36-snowy-giant-taiga-and-mushroom-table-coverage.md). The next broad biome-table win after that is shoreline and transition parity: beach, river, and ocean follow-through first, with `snowy_beach`, `ice_spikes`, and bamboo-jungle still waiting behind it.
