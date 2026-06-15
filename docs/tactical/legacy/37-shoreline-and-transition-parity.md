# Tactical 37 — Shoreline and transition parity

Fill the next high-value biome-table gap after snowy / giant-taiga / mushroom coverage: beaches, rivers, and the first real ocean follow-through. This slice ports the missing seagrass/kelp variants and the `COUNT_NOISE_BIASED` placement path they depend on, wires the common shoreline and non-warm ocean biome keys into translated settings, and validates the result in browser free-cam frames instead of leaving those families on carver-only fallback.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/placement/NoiseBasedDecorator.java` | `src/worldgen/levelgen/placement/count-noise-biased-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/NoiseCountFactorDecoratorConfiguration.java` | `src/worldgen/levelgen/feature/configurations/noise-count-factor-decorator-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/KelpFeature.java` | `src/worldgen/levelgen/feature/kelp-feature.ts` |
| `reference/.../src/net/minecraft/world/level/block/{KelpBlock,KelpPlantBlock}.java` | `src/world/level/block/{kelp-block,kelp-plant-block}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`SEAGRASS_*`, `KELP_*`) | `src/worldgen/levelgen/feature/{features,vegetation-features}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addWaterTrees`, `addColdOceanExtraVegetation`, `addLukeWarmKelp`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`riverBiome`, `beachBiome`, `oceanBiome`, `coldOceanBiome`, `lukeWarmOceanBiome`, `frozenOceanBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../extracted/assets/minecraft/{blockstates,models}/kelp*.json` | `src/world/level/generated-render-blocks.ts`, browser render path |
| `test/worldgen/levelgen/feature/vegetation-parity.test.ts`, `test/renderer/block/surface-feature-palette.test.ts`, `test/browser/probes/shoreline-parity.probe.ts` | same |

## What landed

- `NoiseCountFactorDecoratorConfiguration` and `CountNoiseBiasedDecorator` are now real TS ports, which unblocks the vanilla kelp density path instead of approximating it with the older count decorators.
- `KelpFeature`, `KelpBlock`, and `KelpPlantBlock` are now real TS ports for the generation/runtime/render path, and generated render blocks register `minecraft:kelp` plus `minecraft:kelp_plant` with the expected cutout render layer.
- `VegetationFeatures` now includes `SEAGRASS_COLD`, `SEAGRASS_DEEP_COLD`, `SEAGRASS_NORMAL`, `SEAGRASS_RIVER`, `SEAGRASS_DEEP`, `SEAGRASS_WARM`, `SEAGRASS_DEEP_WARM`, `KELP_COLD`, and `KELP_WARM`.
- `overworld-biome-generation-settings.ts` now has translated settings for `minecraft:beach`, `minecraft:snowy_beach`, `minecraft:stone_shore`, `minecraft:river`, `minecraft:frozen_river`, `minecraft:ocean`, `minecraft:deep_ocean`, `minecraft:cold_ocean`, `minecraft:deep_cold_ocean`, `minecraft:lukewarm_ocean`, `minecraft:deep_lukewarm_ocean`, `minecraft:frozen_ocean`, and `minecraft:deep_frozen_ocean`.
- Focused tests now pin the new kelp/seagrass feature paths, the shoreline/ocean biome-key wiring, kelp render registration, and dedicated browser shoreline frames.

## Scope choice

- Landed here: the first real shoreline / river / ocean table coverage so those common families stop collapsing to carver-only generation settings.
- Kept intentionally narrow: no `warm_ocean` / `deep_warm_ocean` coral, `SEA_PICKLE`, or `WARM_OCEAN_VEGETATION` path yet. That is a separate marine-ecosystem slice.
- Also kept narrow: no `SEAGRASS_SIMPLE` follow-through, because the repo still lacks the `CARVING_MASK` decorator plumbing that vanilla uses for that LIQUID-carving seagrass pass.
- Also still out of scope here: no `FREEZE_TOP_LAYER`, `ICE_SPIKE`, or `ICE_PATCH` follow-through. The shoreline tables are real now, but the remaining cold-surface polish is still its own slice.

## Oracle / done-when

**Unit (Vitest):**

- `vegetation-parity.test.ts` proves the river/ocean seagrass and kelp features place translated aquatic blocks and that shoreline/ocean biome keys no longer resolve to empty fallback settings.
- `surface-feature-palette.test.ts` proves `kelp` and `kelp_plant` are registered into the generated render palette with the expected render layer.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/shoreline-parity.probe.ts` passes.
- `/tmp/mclone-debug-beach.png`, `/tmp/mclone-debug-river.png`, `/tmp/mclone-debug-snowy-beach.png`, `/tmp/mclone-debug-frozen-ocean-shoreline.png`, and `/tmp/mclone-debug-stone-shore.png` are manually inspected.
- Expectation for this slice: beach / river / frozen-ocean / stone-shore identity reads correctly under the repo’s existing green fog setup; snowy beach is still visibly limited by the deferred cold-surface follow-through (`FREEZE_TOP_LAYER`, `ICE_SPIKE`, `ICE_PATCH`).

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for vegetation parity and generated render palette coverage
- `pnpm probe:browser -- test/browser/probes/shoreline-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` stop listing generic shoreline / river / non-warm-ocean table coverage as the next missing slice

## Next

Tactical 38 is now landed as [`38-cold-surface-parity.md`](38-cold-surface-parity.md): the missing `FREEZE_TOP_LAYER`, `ICE_SPIKE`, and `ICE_PATCH` paths are translated, `ice_spikes` now has a real biome table and surface definition, and the browser suite captures a dedicated `ice_spikes` frame. The next broad biome-identity gap after that is warm-ocean parity: coral, sea pickles, and the `warm_ocean` / `deep_warm_ocean` table follow-through.
