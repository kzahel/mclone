# Tactical 54 - Deep-warm-ocean `SEAGRASS_SIMPLE` follow-through

Finish the last narrow warm-ocean table exactness gap by porting vanilla `SEAGRASS_SIMPLE`, wiring the `CARVING_MASK` decorator path it depends on, and documenting the 1.17.1 Java source fact that `deep_warm_ocean` does not naturally survive the final ocean mix even though the biome/table entry still exists.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`SEAGRASS_SIMPLE`) | `src/worldgen/levelgen/feature/vegetation-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addDefaultSeagrass`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`deepWarmOceanBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/placement/CarvingMaskDecorator.java` and `.../CarvingMaskDecoratorConfiguration.java` | `src/worldgen/levelgen/placement/carving-mask-decorator.ts`, `src/worldgen/levelgen/feature/configurations/carving-mask-decorator-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/newbiome/layer/{AddDeepOceanLayer,OceanMixerLayer}.java` | `src/worldgen/biome/layered/layers.ts`, `test/worldgen/biome/layered-layers.test.ts` |

## What landed

- The worldgen framework now carries LIQUID-carver masks forward from carver execution into generated chunks, decoration regions, and copied render/runtime chunks, so translated feature decorators can read the same mask data vanilla uses.
- `FeatureDecorators.CARVING_MASK`, `CarvingMaskDecoratorConfiguration`, and `VegetationFeatures.SEAGRASS_SIMPLE` are now direct ports of the vanilla 1.17.1 path.
- `deep_warm_ocean` now matches the vanilla table exactly by adding `SEAGRASS_DEEP_WARM` plus `BiomeDefaultFeatures.addDefaultSeagrass(...)`, which contributes the `SEAGRASS_SIMPLE` carving-mask pass.
- Focused tests now cover both sides of the slice: feature placement proves the carving-mask decorator replays stored LIQUID-carver cells, and biome/layer tests prove the final 1.17.1 ocean mix collapses would-be deep-warm output back to warm/lukewarm ocean instead of naturally yielding biome id `47`.

## Scope choice

- Landed here: the exact configured-feature and decorator plumbing needed for `deep_warm_ocean` table parity.
- Kept intentionally narrow: no coral block-state runtime behavior, no coral death-tick follow-through, and no broader ocean-table reshaping.
- Important source finding: this slice does not get a real worker-world screenshot target in Java 1.17.1 because `OceanMixerLayer` never naturally returns `deep_warm_ocean`. Even if `AddDeepOceanLayer` produces biome id `47` internally, the final mix returns warm/lukewarm output instead.

## Oracle / done-when

**Unit (Vitest):**

- `feature-placement.test.ts` proves `CARVING_MASK` replays stored LIQUID-carver cells and that `SEAGRASS_SIMPLE` only places on those masked underwater cells.
- `vegetation-parity.test.ts` proves `deep_warm_ocean` carries the translated `SEAGRASS_SIMPLE` path and that `warm_ocean` still does not.
- `layered-layers.test.ts` proves the final ocean mixer collapses would-be deep-warm output back to warm/lukewarm ocean in the live 1.17.1 layered source path.

**Browser validation:**

- No dedicated worker-world probe exists for this slice because the natural 1.17.1 Java overworld does not surface `deep_warm_ocean` after `OceanMixerLayer`.
- This is a source-verified non-visual exactness fix, not a missing rendered biome family.

## Done when

- focused `pnpm test` passes for `layered-layers.test.ts`, `feature-placement.test.ts`, and `vegetation-parity.test.ts`
- `pnpm typecheck` passes when the unrelated runtime worktree is green again
- `docs/worldgen-status.md` stops listing `deep_warm_ocean` as an open natural-world parity gap

## Next

There is no equally crisp remaining "known missing simple decoration" after this slice. The next work in the same general lane is a selector/decorator exactness audit, starting with the remaining plains / flower-forest / birch tree-table weighting and variant-consumer exactness.
