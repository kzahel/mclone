# Tactical 43 — Biome-specific underground extras

Port the next live 1.17.1 overworld underground slice after the common-ore and underground-variety foundation: badlands extra gold, mountain emeralds, and mountain infested stone. Keep the port exact, including the fact that emeralds use `ReplaceBlockFeature` rather than `OreFeature`.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../data/worldgen/BiomeDefaultFeatures.java` (`addExtraGold(...)`, `addExtraEmeralds(...)`, `addInfestedStone(...)`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/Features.java` ore / underground section (`ORE_GOLD_EXTRA`, `ORE_EMERALD`, `ORE_INFESTED`) | `src/worldgen/levelgen/feature/ore-features.ts` |
| `reference/.../world/level/levelgen/feature/ReplaceBlockFeature.java` | `src/worldgen/levelgen/feature/replace-block-feature.ts` |
| `reference/.../world/level/levelgen/feature/configurations/ReplaceBlockConfiguration.java` | `src/worldgen/levelgen/feature/configurations/replace-block-configuration.ts` |
| `reference/.../world/level/levelgen/feature/Feature.java` (`REPLACE_SINGLE_BLOCK`) | `src/worldgen/levelgen/feature/features.ts` |
| `reference/.../data/worldgen/biome/VanillaBiomes.java` (`mountainBiome(...)`, `baseBadlandsBiome(...)`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../world/level/block/Blocks.java` (`EMERALD_ORE`, `DEEPSLATE_EMERALD_ORE`, `INFESTED_STONE`, `INFESTED_DEEPSLATE`) | `src/world/level/generated-render-blocks.ts` |

## What landed

- `ReplaceBlockConfiguration` and `ReplaceBlockFeature` now exist as direct TS ports of the vanilla single-block replacement path.
- `Features.REPLACE_SINGLE_BLOCK` is now registered, which is the exact 1.17.1 feature entry emerald generation uses.
- `ore-features.ts` now mirrors the vanilla biome-specific underground extras:
  - `ORE_GOLD_EXTRA`
  - `ORE_EMERALD`
  - `ORE_INFESTED`
- The ore target lists now include emerald ore and infested-stone variants, matching the vanilla stone/deepslate replaceable split.
- The generated render palette now registers `emerald_ore`, `deepslate_emerald_ore`, `infested_stone`, and `infested_deepslate`, keeping their default states renderable in decorated chunks.
- `overworld-biome-generation-settings.ts` now has local `addExtraGold(...)`, `addExtraEmeralds(...)`, and `addInfestedStone(...)` helpers and wires them into the translated overworld tables where vanilla uses them:
  - badlands family: extra gold at `UNDERGROUND_ORES`
  - mountains family: emeralds at `UNDERGROUND_ORES`, infested stone at `UNDERGROUND_DECORATION`
- Focused tests now cover the replace-single-block path, the configured-feature defaults, the biome-table wiring, the new render-layer registrations, and packed-snapshot round trips for the new states.
- Browser validation now includes `/tmp/mclone-debug-mountain-emerald.png`, and the inspected frame shows exposed mountain emerald ore in generated terrain.

## Scope choice

- Landed here: the live biome-specific underground extras that vanilla 1.17.1 adds on top of the common ore and underground-variety passes.
- Kept intentionally narrow: no soft disks, no glow lichen / rare-dripstone tail from `addDefaultUndergroundVariety(...)`, no disabled Caves & Cliffs cave systems, no structures, and no gameplay infestation behavior.
- Kept future-shaped: the minimal `ReplaceBlockFeature` path is now in place, so later underground slices can use the same vanilla single-block replacement surface instead of inventing a parallel mechanism.

## Validation

- `pnpm test -- test/worldgen/levelgen/feature/ore-feature.test.ts test/worldgen/levelgen/feature/replace-block-feature.test.ts test/renderer/block/surface-feature-palette.test.ts test/world/packed-chunk-snapshot.test.ts` passes.
- `pnpm probe:browser -- test/browser/probes/biome-underground-extras.probe.ts` passes.
- `/tmp/mclone-debug-mountain-emerald.png` was inspected and clearly shows exposed mountain emerald ore.
- `pnpm typecheck` passes.
- `pnpm test:browser` passes.
- `pnpm perf:d5` is currently failing in this tree during debug-page startup with `TypeError: Failed to fetch` / ready-wait timeout before traversal begins.

## Next

The next underground parity slice should move from biome-specific extras into the remaining active default-underground tail:

1. Finish the still-live `BiomeDefaultFeatures` underground follow-through: glow lichen and the rare dripstone features we still skip.
2. Port soft disks and related replace-material underground families now that the biome-specific ore/extras surface is in place.
3. Only after those, broaden oracle coverage for later underground decoration stages if the widened surface justifies it.
