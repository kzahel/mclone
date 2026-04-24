# Tactical 42 — Ore and underground decoration foundation

Land the first underground-content slices by porting the vanilla 1.17.1 common overworld ore path and then broadening it into the active `addDefaultUndergroundVariety(...)` material-blob follow-through: `OreFeature`, the minimal `RuleTest` support it depends on, the default configured ore entries, the tuff/deepslate underground material blocks that later ore targets need, and the biome-table wiring that puts those underground features into the current overworld decoration pass.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../feature/OreFeature.java` | `src/worldgen/levelgen/feature/ore-feature.ts` |
| `reference/.../feature/configurations/OreConfiguration.java` | `src/worldgen/levelgen/feature/configurations/ore-configuration.ts` |
| `reference/.../structure/templatesystem/{RuleTest,BlockMatchTest,BlockStateMatchTest,TagMatchTest,RuleTestType}.java` | `src/world/level/levelgen/structure/templatesystem/*` |
| `reference/.../data/worldgen/Features.java` ore section | `src/worldgen/levelgen/feature/ore-features.ts` |
| `reference/.../data/worldgen/BiomeDefaultFeatures.java` `addDefaultOres(...)` | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/biome/VanillaBiomes.java` ore call sites | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/minecraft/tags/blocks/{base_stone_overworld,stone_ore_replaceables,deepslate_ore_replaceables}.json` | `src/tags/block-tags.ts` |
| `reference/.../assets/minecraft/{blockstates,models,textures}/**/*_ore*` | `src/world/level/generated-render-blocks.ts` |

## What landed

- `RuleTest`, `BlockMatchTest`, `BlockStateMatchTest`, `TagMatchTest`, and `RuleTestType` now exist in the translated structure-template path, which is the minimal vanilla rule stack ore placement needs.
- `OreConfiguration` and `OreFeature` are now direct TS ports of the vanilla common-ore path, including the target-state loop, duplicate-position suppression, and air-exposure discard helper behavior.
- `Feature.ORE` is now registered, and `ore-features.ts` mirrors the vanilla default overworld configured entries for:
  - `ORE_DIRT`
  - `ORE_GRAVEL`
  - `ORE_GRANITE`
  - `ORE_DIORITE`
  - `ORE_ANDESITE`
  - `ORE_TUFF`
  - `ORE_DEEPSLATE`
  - `ORE_COAL`
  - `ORE_IRON`
  - `ORE_GOLD`
  - `ORE_REDSTONE`
  - `ORE_DIAMOND`
  - `ORE_LAPIS`
  - `ORE_COPPER`
- `BlockTags` now exposes `BASE_STONE_OVERWORLD`, `STONE_ORE_REPLACEABLES`, and `DEEPSLATE_ORE_REPLACEABLES`, matching the vanilla target-list inputs for the common ore set.
- The generated render palette now registers tuff, deepslate, the common ore blocks, and their deepslate variants, includes the needed textures, and keeps them on the solid render layer so decorated chunks can draw exposed underground faces correctly.
- `overworld-biome-generation-settings.ts` now has local `addDefaultUndergroundVariety(...)` and `addDefaultOres(...)` helpers and wires them into the translated overworld biome tables in the same order vanilla uses: underground material blobs first, then the common ore pass at `GenerationStep.Decoration.UNDERGROUND_ORES`.
- Focused tests now cover `OreFeature.canPlaceOre(...)`, the natural-stone target predicate, deterministic ore placement, configured-feature shape + biome wiring for both common ores and underground variety, render-layer registration, and packed-snapshot round trips for the new underground block states.
- Browser validation now includes `/tmp/mclone-debug-ore-foundation.png`, `/tmp/mclone-debug-underground-variety-tuff.png`, and `/tmp/mclone-debug-underground-variety-deepslate.png`; the underground frames were inspected and show exposed tuff/deepslate material rendered in real decorated terrain.

## Scope choice

- Landed here: the common-ore overworld foundation plus the active underground variety material blobs that vanilla runs ahead of that ore pass.
- Kept intentionally narrow: no `OreVeinifier`, no disabled Caves & Cliffs cave systems, no deepslate base-stone substitution, no glow lichen, no dripstone tail from `addDefaultUndergroundVariety(...)`, no monster rooms/structures, no soft disks, no badlands extra gold, no mountain emeralds, no infested stone, and no gameplay-specific ore block behavior.
- Kept future-shaped: the target lists already include deepslate ore outputs so later underground slices can extend the active block palette without reshaping the ore configuration surface again.

## Validation

- `pnpm test -- test/worldgen/levelgen/feature/ore-feature.test.ts test/renderer/block/surface-feature-palette.test.ts test/world/packed-chunk-snapshot.test.ts` passes.
- `pnpm probe:browser -- test/browser/probes/ore-foundation.probe.ts` and `pnpm probe:browser -- test/browser/probes/underground-variety.probe.ts` pass.
- `/tmp/mclone-debug-ore-foundation.png`, `/tmp/mclone-debug-underground-variety-tuff.png`, and `/tmp/mclone-debug-underground-variety-deepslate.png` were inspected.
- `pnpm typecheck` is currently blocked in this dirty tree by unrelated errors in `test/oracle/creature-fixture.test.ts` and `test/world/client-chunk-cache.test.ts`, not by the tactical 42 ore/underground files.
- `pnpm test:browser` and `pnpm perf:d5` are currently blocked in this dirty tree by unrelated remote-browser/runtime regressions outside the tactical 42 paths (`Execution context was destroyed` during remote boot and `expected 225 loaded chunks for viewDistance=6, got 0` in the D5 harness).

## Next

The next underground slice should stay on the live vanilla overworld path:

1. Biome-specific underground extras as separate follow-ups: badlands extra gold, mountain emeralds, and infested stone.
2. Finish the remaining active default-underground tail that is still in scope for 1.17.1 overworld parity, including the glow-lichen / rare-dripstone pieces if we choose to keep broadening `BiomeDefaultFeatures`.
3. Only after those, broader underground feature forms such as `ScatteredOreFeature`, `ReplaceBlockFeature`, soft disks, and any stronger ore-stage oracle fixture work that the widened surface justifies.
