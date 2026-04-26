# Tactical 52 - Sunflower-plains follow-through

Finish the next clearly known-missing simple decoration slice by porting vanilla `PATCH_SUNFLOWER` and splitting `sunflower_plains` from plain plains so the biome gets its extra sunflower patch and the vanilla vegetal ordering around sugar cane, mushrooms, and pumpkins.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`PATCH_SUNFLOWER`) | `src/worldgen/levelgen/feature/vegetation-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`plainsBiome(true)`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/blockplacers/DoublePlantPlacer.java` | existing translated `DoublePlantPlacer` consumer wiring |
| `reference/.../extracted/assets/minecraft/{blockstates,models,textures}/**/*sunflower*` | `src/world/level/generated-render-blocks.ts` |
| `test/worldgen/levelgen/feature/vegetation-parity.test.ts`, `test/renderer/block/surface-feature-palette.test.ts`, `test/browser/probes/sunflower-plains-parity.probe.ts` | same |

## What landed

- `VegetationFeatures.PATCH_SUNFLOWER` now mirrors the vanilla configured feature: `RANDOM_PATCH`, `SimpleStateProvider(SUNFLOWER)`, `DoublePlantPlacer`, `tries(64)`, `noProjection`, `ADD_32`, `HEIGHTMAP_SQUARE`, and `count(10)`.
- The generated render palette now registers `minecraft:sunflower`, its extracted blockstate/models/textures, and the expected cutout render layer.
- `overworld-biome-generation-settings.ts` now splits `sunflower_plains` from `plains` instead of reusing the exact same settings object. The biome now inserts `PATCH_SUNFLOWER` and preserves the vanilla vegetal ordering split where sugar cane lands before mushrooms and pumpkins land after them.
- Validation covers the configured-feature placement, the sunflower-plains biome table shape, palette/render registration, and a browser probe. The validated frame is `/tmp/mclone-debug-sunflower-plains.png`, which shows generated sunflowers at the sunflower-plains edge.

## Scope choice

- Landed here: the minimal worldgen and render follow-through needed for sunflower-plains parity.
- Kept intentionally narrow: no new plant mechanics, no generalized double-plant feature family expansion beyond the vanilla sunflower patch, and no unrelated plains-table cleanup.
- Kept exact where it matters: the slice does not just add another patch feature somewhere in plains; it restores the sunflower-plains-specific configured feature and its ordering relative to the existing plains vegetal entries.

## Oracle / done-when

**Unit (Vitest):**

- `vegetation-parity.test.ts` proves `PATCH_SUNFLOWER` only places translated sunflower states.
- The same suite proves `sunflower_plains` now carries one extra vegetal feature, that the extra feature is the sunflower patch, and that the sugar-cane-before-mushrooms split matches vanilla.
- `surface-feature-palette.test.ts` proves the generated render palette registers `minecraft:sunflower`, its cutout render layer, and its sprite coverage.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/sunflower-plains-parity.probe.ts` passes.
- `/tmp/mclone-debug-sunflower-plains.png` is manually inspected and visibly shows generated sunflowers in the worker-generated world.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for vegetation-parity and surface-palette coverage
- `pnpm probe:browser -- test/browser/probes/sunflower-plains-parity.probe.ts` passes
- `docs/worldgen-status.md` stops naming sunflower plains as the next missing simple decoration slice

## Next

Stay in the same lane and do the remaining vine-placement consumers outside the current jungle-focused coverage.
