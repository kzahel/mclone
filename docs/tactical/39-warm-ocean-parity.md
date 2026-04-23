# Tactical 39 — Warm-ocean parity

Finish the last major common ocean-family fallback by porting the warm-ocean coral path. This slice lands the coral feature family, `SeaPickleFeature`, the minimal coral / sea-pickle block and tag support they need, and the `warm_ocean` / `deep_warm_ocean` biome tables so those biomes stop falling back to the earlier reduced ocean mix.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`warmOceanBiome`, `deepWarmOceanBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{Features,BiomeDefaultFeatures}.java` | `src/worldgen/levelgen/feature/{features,vegetation-features}.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/{CoralFeature,CoralTreeFeature,CoralClawFeature,CoralMushroomFeature,SeaPickleFeature}.java` | `src/worldgen/levelgen/feature/{coral-feature,coral-tree-feature,coral-claw-feature,coral-mushroom-feature,sea-pickle-feature}.ts` |
| `reference/.../src/net/minecraft/world/level/block/{CoralBlock,BaseCoralPlantTypeBlock,BaseCoralPlantBlock,BaseCoralFanBlock,BaseCoralWallFanBlock,CoralPlantBlock,CoralFanBlock,CoralWallFanBlock,SeaPickleBlock}.java` | `src/world/level/block/{coral-block,base-coral-plant-type-block,base-coral-plant-block,base-coral-fan-block,base-coral-wall-fan-block,coral-plant-block,coral-fan-block,coral-wall-fan-block,sea-pickle-block}.ts` |
| `reference/.../src/data/minecraft/tags/blocks/{corals,wall_corals,coral_blocks,coral_plants}.json` | `src/tags/block-tags.ts` |
| `reference/.../assets/minecraft/{blockstates,models,textures}/**/*coral*`, `**/sea_pickle*` | `src/world/level/generated-render-blocks.ts` |
| `test/worldgen/levelgen/feature/vegetation-parity.test.ts`, `test/renderer/block/surface-feature-palette.test.ts`, `test/browser/warm-ocean-parity.test.ts` | same |

## What landed

- The runtime block palette now includes the live/dead coral block, plant, fan, and wall-fan families plus `sea_pickle`, with the `pickles` and `waterlogged` properties this slice needs.
- `BlockTags` now exposes the coral-family tags the translated coral features expect, and `generated-render-blocks.ts` now registers the matching blockstates, models, sprites, and cutout-layer handling.
- `CoralFeature`, `CoralTreeFeature`, `CoralClawFeature`, `CoralMushroomFeature`, and `SeaPickleFeature` are now real TS ports instead of missing registry holes.
- `Features` and `VegetationFeatures` now expose the vanilla-shaped warm-ocean configured/decorated entries: `WARM_OCEAN_VEGETATION`, `SEA_PICKLE`, `SEAGRASS_WARM`, and `SEAGRASS_DEEP_WARM`.
- `overworld-biome-generation-settings.ts` now gives `minecraft:warm_ocean` and `minecraft:deep_warm_ocean` real translated tables instead of falling back to the reduced earlier ocean path.
- Browser validation now includes a dedicated warm-ocean frame at `/tmp/mclone-debug-warm-ocean.png` rather than relying on shoreline or lukewarm-ocean shots to imply coral coverage.

## Scope choice

- Landed here: the broad warm-ocean identity slice needed for recognizable 1.17.1 overworld parity in common ocean biomes.
- Kept intentionally narrow: no exact `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK` follow-through, and no later coral death-tick/runtime-fluid behavior beyond the current generation-time placement boundary.
- Kept honest: the browser frame clearly reads as a shallow warm-ocean shelf with dense marine vegetation, while the coral / sea-pickle content is subtler than the tall seagrass massing in the current renderer. It is still a debug free-cam validation shot rather than a polished presentation render.

## Oracle / done-when

**Unit (Vitest):**

- `vegetation-parity.test.ts` proves the translated warm-ocean vegetation selector and sea-pickle path place coral / sea-pickle outputs, and that `warm_ocean` / `deep_warm_ocean` now expose the intended biome-table entries instead of kelp-heavy fallback.
- `surface-feature-palette.test.ts` proves the renderer keeps the new coral / sea-pickle blocks in the expected render layers and palette.

**Browser validation:**

- `pnpm test:browser -- test/browser/warm-ocean-parity.test.ts` passes.
- `/tmp/mclone-debug-warm-ocean.png` is manually inspected and reads as a warm-ocean shallow shelf rather than the old generic ocean mix, even though the coral / sea-pickle placements are subtler than the tall vegetation in the current renderer.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for warm-ocean vegetation and render-palette coverage
- `pnpm test:browser -- test/browser/warm-ocean-parity.test.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` stop listing warm-ocean parity as the next missing slice

## Next

The next numbered parity slice should be tactical 41, because [`40-debug-free-cam.md`](40-debug-free-cam.md) is already reserved for temporary debug tooling. That tactical should port the bamboo block / feature path, wire `bamboo_jungle` and `bamboo_jungle_hills`, and finish the still-missing jungle-family follow-through so the last obviously reduced jungle identity stops falling back.
