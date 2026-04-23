# Tactical 36 — Snowy, giant-taiga, and mushroom table coverage

Broaden biome-table coverage for the next high-value overworld families that were still falling back to reduced settings after jungle parity. This slice ports the missing snowy-tree / giant-conifer / mushroom-field selector path, lands the `AlterGroundDecorator` podzol follow-through for mega spruce and mega pine, and wires the snowy, giant-taiga, and mushroom-field biome keys so those families stop rendering like generic taiga or carver-only terrain.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/RandomBooleanFeatureConfiguration.java` | `src/worldgen/levelgen/feature/configurations/random-boolean-feature-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/RandomBooleanSelectorFeature.java` | `src/worldgen/levelgen/feature/random-boolean-selector-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/MegaPineFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/mega-pine-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/AlterGroundDecorator.java` | `src/worldgen/levelgen/feature/treedecorators/alter-ground-decorator.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`MEGA_SPRUCE`, `MEGA_PINE`, `PATCH_GRASS_TAIGA`, `BROWN_MUSHROOM_GIANT`, `RED_MUSHROOM_GIANT`, `TREES_SNOWY`, `TREES_GIANT`, `TREES_GIANT_SPRUCE`, `MUSHROOM_FIELD_VEGETATION`) | `src/worldgen/levelgen/feature/{tree-features,vegetation-features,features}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addSnowyTrees`, `addGiantTaigaVegetation`, `addMushroomFieldVegetation`, `addTaigaGrass`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/{VanillaBiomes,Biomes}.java` (`tundraBiome`, `taigaBiome`, `giantTreeTaiga`, `mushroomFieldsBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/browser/cold-biome-parity.test.ts` | same |

## What landed

- `RandomBooleanFeatureConfiguration` and `RandomBooleanSelectorFeature` are now real TS ports, which unblocks vanilla `MUSHROOM_FIELD_VEGETATION` instead of faking mushroom fields with a fixed huge-mushroom feature.
- `MegaPineFoliagePlacer` and `AlterGroundDecorator` are now direct TS ports, and `TreeFeatures` includes the missing `MEGA_SPRUCE` and `MEGA_PINE` configs with the same podzol-ring decorator vanilla uses.
- `VegetationFeatures` now includes `PATCH_GRASS_TAIGA`, `BROWN_MUSHROOM_GIANT`, `RED_MUSHROOM_GIANT`, `TREES_SNOWY`, `TREES_GIANT`, `TREES_GIANT_SPRUCE`, and `MUSHROOM_FIELD_VEGETATION`, so the giant-taiga and mushroom-field tables can stop collapsing to the old reduced taiga mix.
- `overworld-biome-generation-settings.ts` now has translated tables for `minecraft:snowy_tundra`, `minecraft:snowy_mountains`, `minecraft:snowy_taiga`, `minecraft:snowy_taiga_hills`, `minecraft:snowy_taiga_mountains`, `minecraft:giant_tree_taiga`, `minecraft:giant_tree_taiga_hills`, `minecraft:giant_spruce_taiga`, `minecraft:giant_spruce_taiga_hills`, `minecraft:mushroom_fields`, and `minecraft:mushroom_field_shore`.
- Focused tests now pin the mega-conifer podzol decorator path, the snowy / giant-taiga / mushroom output families, the expanded biome-key wiring, and a dedicated cold-biome browser frame set.

## Scope choice

- Landed here: the high-value biome-table expansion that makes snowy, giant-taiga, and mushroom-field chunks read as their own families instead of reduced taiga or generic fallback terrain.
- Kept intentionally narrow: no `FREEZE_TOP_LAYER` feature path, no `ICE_SPIKE` / `ICE_PATCH` structures for `ice_spikes`, and no `snowy_beach` / broader beach-table follow-through. Those are separate parity slices.
- Also kept narrow: no bamboo-jungle work. Jungle core parity is already landed, but bamboo is still its own missing ecosystem and should stay visible as separate work.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves `MEGA_SPRUCE` and `MEGA_PINE` place the translated giant-trunk / mega-pine foliage path and actually paint podzol through `AlterGroundDecorator`.
- `vegetation-parity.test.ts` proves the snowy, giant-taiga, and mushroom-field selectors stay inside the intended translated output families, and that the new biome keys now route through non-empty translated feature tables instead of fallback settings.

**Browser validation:**

- `pnpm test:browser -- test/browser/cold-biome-parity.test.ts` passes.
- `/tmp/mclone-debug-snowy-taiga.png`, `/tmp/mclone-debug-giant-taiga-parity.png`, and `/tmp/mclone-debug-mushroom-fields-parity.png` are manually inspected for snowy spruce canopy, podzol-heavy giant-taiga ground cover, and visible huge mushrooms in the worker-generated world.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for the tree / vegetation parity suites
- `pnpm test:browser -- test/browser/cold-biome-parity.test.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` all stop listing snowy / giant-taiga / mushroom-field coverage as the next missing biome-table slice

## Next

Tactical 37 is now landed in [`37-shoreline-and-transition-parity.md`](./37-shoreline-and-transition-parity.md), and tactical 38 is now landed in [`38-cold-surface-parity.md`](./38-cold-surface-parity.md). The next broad follow-through after those two shoreline/cold slices is warm-ocean parity: coral, sea pickles, and the `warm_ocean` / `deep_warm_ocean` biome tables, with bamboo-jungle still visible as a separate ecosystem slice behind it.
