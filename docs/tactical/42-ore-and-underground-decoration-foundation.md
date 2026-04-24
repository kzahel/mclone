# Tactical 42 - Ore and underground decoration foundation

Standing after [`41-bamboo-jungle-parity.md`](41-bamboo-jungle-parity.md) and the D5/D6 runtime scheduling decision. Surface biome identity is now broad enough that the biggest missing chunk-content signal is underground decoration, especially common overworld ores.

## Goal

Add the first vanilla-shaped 1.17.1 overworld ore path without widening the target into disabled Caves & Cliffs Part 1 systems.

The first implementation slice should make generated chunks able to contain common overworld ore veins through the same biome decoration step that already places vegetation:

- `OreFeature` placement math
- `OreConfiguration` target-state rules
- minimal `RuleTest` support for block and tag matches
- common overworld ore block registrations and render palette entries
- vanilla `Features.ORE_COAL`, `ORE_IRON`, `ORE_GOLD`, `ORE_REDSTONE`, `ORE_DIAMOND`, `ORE_LAPIS`, and `ORE_COPPER`
- `BiomeDefaultFeatures.addDefaultOres(...)` equivalent wiring into the current overworld biome tables
- focused tests and one visual/probe path that makes ore presence inspectable

## Source files

Read these before implementing the port.

| Java / asset source | TS target |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/OreFeature.java` | `src/worldgen/levelgen/feature/ore-feature.ts` |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/configurations/OreConfiguration.java` | `src/worldgen/levelgen/feature/configurations/ore-configuration.ts` |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/{RuleTest,BlockMatchTest,BlockStateMatchTest,TagMatchTest,RuleTestType}.java` | `src/world/level/levelgen/structure/templatesystem/*` |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/Feature.java` | `src/worldgen/levelgen/feature/features.ts` |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java` ore section | `src/worldgen/levelgen/feature/ore-features.ts` or local equivalent |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` `addDefaultOres(...)` | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` ore call sites | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/minecraft-1.17.1/src/data/minecraft/tags/blocks/{base_stone_overworld,stone_ore_replaceables,deepslate_ore_replaceables,*_ores}.json` | `src/tags/block-tags.ts` |
| `reference/minecraft-1.17.1/src/assets/minecraft/{blockstates,models,textures}/**/*_ore*` | `src/world/level/generated-render-blocks.ts` and renderer palette tests |

Also review the existing local analogs before editing:

- `src/worldgen/levelgen/feature/configured-feature.ts`
- `src/worldgen/levelgen/placement/range-decorator.ts`
- `src/worldgen/carver/carver-config.ts`
- `src/worldgen/biome/overworld-biome-generation-settings.ts`
- `src/worldgen/chunk/chunk-block-buffer.ts`
- `src/world/level/chunk-snapshot.ts`
- `src/world/packed-chunk-snapshot.ts`

## Scope

### In scope for the first slice

- Direct TS port of `OreFeature` and `OreConfiguration`.
- Minimal `RuleTest` hierarchy needed by ore targets:
  - `BlockMatchTest`
  - `BlockStateMatchTest`
  - `TagMatchTest`
- Block tags needed by the common ore target lists:
  - `BASE_STONE_OVERWORLD`
  - `STONE_ORE_REPLACEABLES`
  - `DEEPSLATE_ORE_REPLACEABLES`
- Common overworld ore blocks and chunk ids:
  - `coal_ore`
  - `iron_ore`
  - `gold_ore`
  - `redstone_ore`
  - `diamond_ore`
  - `lapis_ore`
  - `copper_ore`
- Render palette entries for those blocks so exposed ore veins draw in the browser.
- Height-provider/decorator helpers needed by the vanilla configured ore entries, especially uniform and triangle ranges.
- `GenerationStep.Decoration.UNDERGROUND_ORES` wiring for the vanilla default ore set in current overworld biome tables.

### Explicit non-goals

- No `OreVeinifier`; it is disabled for the 1.17.1 vanilla overworld target in `NoiseGeneratorSettings.overworld(...)`.
- No `Aquifer`, `Cavifier`, `NoodleCavifier`, `NoiseUtils`, or other disabled Caves & Cliffs Part 1 cave systems.
- No deepslate base-stone substitution path in `DepthBasedReplacingBaseStoneSource`.
- No structures, monster rooms, mineshafts, strongholds, or jigsaw systems.
- No full `addDefaultUndergroundVariety(...)` pass yet unless the implementation proves it is needed to make the default ore set coherent.
- No soft disks, dripstone clusters, glow lichen, emerald mountain extras, badlands extra gold, infested stone, `ScatteredOreFeature`, or `ReplaceBlockFeature` in the first slice.
- No item drops, tool requirements, redstone lighting behavior, XP behavior, or block-entity/runtime gameplay semantics.

## Divergence review

Vanilla `OreFeature` mutates sections through `BulkSectionAccess` while preserving exact placement math, target-rule tests, duplicate-position filtering, and air-exposure discard behavior.

The browser/TypeScript port should keep the placement math and rule decisions direct. The expected narrow divergence is storage access: use the existing `WorldGenLevel` / generated chunk APIs instead of reproducing JVM `BulkSectionAccess` as a separate abstraction. If that access pattern becomes a measured D5 regression, split or schedule it later; do not change the ore algorithm first.

The vanilla configured ore lists include deepslate target states. Keep those target lists shaped for future parity, but do not enable deepslate base-stone replacement or `OreVeinifier` as part of this slice.

## Implementation sequence

1. Add the ore block/tag foundation.

   Register common ore blocks, add the needed block tags, widen `ChunkBlockId` / packed snapshot paths, and add render palette coverage. Keep block behavior minimal; generation only needs replaceability and rendering.

2. Port rule tests and ore configuration.

   Add `RuleTest` plus the minimal block/tag/state match tests. Port `OreConfiguration` with `targetStates`, `size`, and `discardChanceOnAirExposure` field names intact.

3. Port `OreFeature`.

   Translate the Java logic directly, including the vein endpoint calculation, ellipsoid pass, overlap suppression, target-state loop, and `canPlaceOre(...)` / air-exposure helpers. Use existing TS math helpers only where they preserve Java behavior.

4. Add configured common ore features.

   Add a focused ore feature module for the default overworld set. Mirror the vanilla `Features.java` names and decorator chain shape for:

   - `ORE_COAL`
   - `ORE_IRON`
   - `ORE_GOLD`
   - `ORE_REDSTONE`
   - `ORE_DIAMOND`
   - `ORE_LAPIS`
   - `ORE_COPPER`

5. Add any missing height/decorator helpers.

   Existing `RANGE`, `SQUARE`, and `COUNT` decorators cover part of the chain. Add the missing triangle/trapezoid height provider or configured-feature convenience helpers only as needed by the ore entries.

6. Wire default ores into biome tables.

   Add an `addDefaultOres(...)` local helper and call it for the current overworld biome settings that vanilla routes through `BiomeDefaultFeatures.addDefaultOres(...)`. Keep biome-specific extras for a later slice.

7. Add validation.

   Cover the feature in isolation, the configured feature list, packed snapshot round trips, and at least one browser probe that frames exposed underground stone where ores can be seen.

8. Update docs.

   Update this doc, [`../worldgen-status.md`](../worldgen-status.md), and this README row with what actually landed and what remains deferred.

## Oracle strategy

Use unit tests first because ore placement has a compact, deterministic algorithm:

- `OreFeature.canPlaceOre(...)` respects tag targets and air-exposure discard chance.
- A fixed seed/origin in an all-stone test level produces stable ore placement.
- Target lists replace stone/granite/diorite/andesite and leave unrelated blocks alone.
- Configured default ore entries expose the expected generation step and decorator shape.

For integration confidence, add a narrow ore-stage oracle if practical. The ideal fixture is a Java-generated chunk after terrain, surface, carvers, and default ores, with other unported underground decorations excluded. If the current oracle harness cannot isolate that stage cleanly, document the gap and use focused unit coverage plus browser visual validation for the first slice.

## Browser validation

The visual probe should save screenshots under `/tmp`, for example:

- `/tmp/mclone-debug-ore-cave.png`

Prefer a seed/position with exposed cave or ravine walls so ore blocks are visible without adding x-ray or debug-only rendering. If no natural exposure is reliable enough, use a targeted browser probe that places the camera inside a carved opening and asserts that the rendered frame settles with nonzero ore block presence in the loaded chunks.

## Done when

- `OreFeature` and `OreConfiguration` are direct ports of the Java source.
- Common overworld ore blocks survive generation, packed snapshots, storage/protocol boundaries, render-world ingest, and browser rendering.
- Current overworld biome tables include the default ore set in `UNDERGROUND_ORES`.
- Focused ore feature/configuration tests pass.
- Snapshot/codec tests cover the new ore ids.
- A browser probe screenshot under `/tmp` has been inspected and shows ore-bearing generated terrain, or the doc records why the first slice could not make a visual ore frame reliable.
- `pnpm typecheck`, focused `pnpm test`, relevant browser validation, and `pnpm perf:d5` pass.
- Docs are updated with what landed and the next underground follow-up.

## Next

After the foundation lands, broaden underground content in this order:

1. `addDefaultUndergroundVariety(...)`: dirt/gravel/stone blobs, then tuff/deepslate blocks only within the active vanilla feature path.
2. Soft disks and biome-specific underground extras: badlands extra gold, mountain emerald/infested stone.
3. Remaining feature forms needed by underground parity, such as `ScatteredOreFeature` and `ReplaceBlockFeature`.
4. A stronger ore-stage oracle matrix once the implementation surface is broad enough to justify more fixtures.
