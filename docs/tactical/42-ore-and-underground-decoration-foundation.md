# Tactical 42 — Ore and underground decoration foundation

Land the first underground-content slice by porting the vanilla 1.17.1 common overworld ore path: `OreFeature`, the minimal `RuleTest` support it depends on, the default configured ore entries, and the biome-table wiring that puts those ores into the current overworld decoration pass.

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
  - `ORE_COAL`
  - `ORE_IRON`
  - `ORE_GOLD`
  - `ORE_REDSTONE`
  - `ORE_DIAMOND`
  - `ORE_LAPIS`
  - `ORE_COPPER`
- `BlockTags` now exposes `BASE_STONE_OVERWORLD`, `STONE_ORE_REPLACEABLES`, and `DEEPSLATE_ORE_REPLACEABLES`, matching the vanilla target-list inputs for the common ore set.
- The generated render palette now registers the common ore blocks plus their deepslate variants, includes their textures, and keeps them on the solid render layer so decorated chunks can draw exposed ore faces.
- `overworld-biome-generation-settings.ts` now has a local `addDefaultOres(...)` helper and wires the vanilla default ore set into the current overworld biome tables at `GenerationStep.Decoration.UNDERGROUND_ORES`.
- Focused tests now cover `OreFeature.canPlaceOre(...)`, deterministic ore placement, configured-feature shape + biome wiring, ore render-layer registration, and ore-state packed-snapshot round trips.
- Browser validation now includes `/tmp/mclone-debug-ore-foundation.png`, and the inspected frame shows an exposed iron ore cluster rendered in real decorated terrain.
- The browser smoke boot path now gives the remote chunk ring longer to settle before failing the smoke assertion, which was needed once the default ore decoration work started running in those startup chunks.

## Scope choice

- Landed here: the first common-ore overworld foundation and the biome-table plumbing that makes those ores appear in real chunks.
- Kept intentionally narrow: no `OreVeinifier`, no disabled Caves & Cliffs cave systems, no deepslate base-stone substitution, no monster rooms/structures, no soft disks, no badlands extra gold, no mountain emeralds, no infested stone, and no gameplay-specific ore block behavior.
- Kept future-shaped: the target lists already include deepslate ore outputs so later underground slices can extend the active block palette without reshaping the ore configuration surface again.

## Validation

- `pnpm test -- test/worldgen/levelgen/feature/ore-feature.test.ts test/renderer/block/surface-feature-palette.test.ts test/world/packed-chunk-snapshot.test.ts` passes.
- `pnpm probe:browser -- test/browser/probes/ore-foundation.probe.ts` passes.
- `/tmp/mclone-debug-ore-foundation.png` was inspected and clearly shows an exposed ore-bearing frame.
- `pnpm test:browser` passes after the smoke wait adjustment.
- `pnpm perf:d5` passes.
- `pnpm typecheck` is still blocked by a pre-existing unrelated error in `test/world/lighting/level-light-engine.test.ts` (`LevelChunk | null` vs `BlockGetter | null`), not by the tactical 42 changes.

## Next

The next underground slice should stay on the live vanilla overworld path:

1. `addDefaultUndergroundVariety(...)` foundation: dirt/gravel/granite/diorite/andesite blobs, then the active tuff/deepslate underground material follow-through only where vanilla 1.17.1 actually uses it.
2. Biome-specific underground extras as separate follow-ups: badlands extra gold, mountain emeralds, and infested stone.
3. Only after those, broader underground feature forms such as `ScatteredOreFeature`, `ReplaceBlockFeature`, and any stronger ore-stage oracle fixture work that the widened surface justifies.
