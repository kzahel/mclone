# Tactical 45 — Overworld biome-table aliases and exactness

Close the remaining broad overworld biome-table holes after the underground-helper stack landed. This slice keeps scope narrow: fill the last layered-overworld biome keys that still fell back to carver-only settings, add the one missing badlands tree configured feature those aliases need, and correct the nearby mountain / badlands / modified-jungle builder exactness gaps revealed by the Java helpers.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../data/worldgen/biome/VanillaBiomes.java` (`mountainBiome(...)`, `modifiedJungleBiome()`, `modifiedJungleEdgeBiome()`, `baseJungleBiome(...)`, `baseBadlandsBiome(...)`, `woodedBadlandsPlateauBiome(...)`, `erodedBadlandsBiome()`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/biome/Biomes.java` (biome-key → helper wiring for `gravelly_mountains`, `modified_gravelly_mountains`, `modified_jungle`, `modified_jungle_edge`, `wooded_badlands_plateau`, `eroded_badlands`, `modified_wooded_badlands_plateau`, `modified_badlands_plateau`) | `src/worldgen/biome/biome-data.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/BiomeDefaultFeatures.java` (`addMountainTrees(...)`, `addMountainEdgeTrees(...)`, `addJungleTrees(...)`, `addJungleEdgeTrees(...)`, `addBadlandsTrees(...)`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/Features.java` (`TREES_BADLANDS`) | `src/worldgen/levelgen/feature/vegetation-features.ts` |

## What landed

- `VegetationFeatures.TREES_BADLANDS` now mirrors vanilla 1.17.1 exactly: plain `TreeFeatures.OAK` with the badlands `COUNT_EXTRA(5, 0.1, 1)` decorator chain.
- `overworld-biome-generation-settings.ts` now covers the remaining layered-overworld biome keys that previously fell back to carver-only settings:
  - `minecraft:gravelly_mountains`
  - `minecraft:modified_gravelly_mountains`
  - `minecraft:modified_jungle`
  - `minecraft:modified_jungle_edge`
  - `minecraft:wooded_badlands_plateau`
  - `minecraft:eroded_badlands`
  - `minecraft:modified_wooded_badlands_plateau`
  - `minecraft:modified_badlands_plateau`
- The nearby helper exactness fixes from the same Java methods landed in the same pass:
  - mountain-family tables now include `addDefaultFlowers(...)`
  - `wooded_mountains` now uses the `addMountainEdgeTrees(...)` branch instead of the lower-density mountain-tree path
  - badlands-family tables now include `addDefaultMushrooms(...)`
  - wooded badlands variants now add `addBadlandsTrees(...)`
  - `modified_jungle` suppresses light bamboo the same way `baseJungleBiome(..., modified=true, ...)` does in vanilla
- Focused tests now pin:
  - direct `TREES_BADLANDS` placement output
  - modified-jungle alias wiring and its no-light-bamboo difference from normal jungle
  - mountain-family flower wiring and tree-density differences for `mountains` / `wooded_mountains` / `mountain_edge` / gravelly aliases
  - badlands-family vegetational stage shape, including the wooded-tree path and default mushroom presence
  - the fact that every layered-overworld biome key in the current target now resolves to a non-empty translated feature table
- Browser validation now includes `/tmp/mclone-debug-modified-jungle.png`, inspected as a grounded modified-jungle frame with jungle trunk / vine decoration and mixed grass / gravel / sand terrain instead of fallback-empty decoration.

## Scope choice

- Landed here: the last broad biome-table alias coverage gap plus the exact helper fixes directly adjacent to those aliases in `VanillaBiomes.java`.
- Kept intentionally narrow: no new structure wiring, no bee decorators, no new underground families, no `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK` exactness yet, and no gameplay block-behavior work.
- Kept future-shaped: the broad layered-overworld key set is now covered, so the next parity slices can focus on narrow exactness or stronger decorated-stage confidence instead of missing table rows.

## Validation

- `pnpm test -- test/worldgen/levelgen/feature/vegetation-parity.test.ts test/worldgen/levelgen/feature/ore-feature.test.ts` passes.
- `pnpm typecheck` passes.
- `pnpm probe:browser -- test/browser/probes/biome-table-aliases.probe.ts` passes.
- `/tmp/mclone-debug-modified-jungle.png` was inspected.
- `pnpm test:browser` passes.
- `pnpm perf:d5` still fails in this tree during debug-page startup with `page.waitForFunction(... state.ready ...)` timing out in `test/browser/d5-traversal.test.ts`.

## Next

With the broad layered-overworld biome key set now covered, the next highest-value parity slice is a concrete decorated-stage confidence target rather than more table breadth: tactical [`46`](46-full-decorated-spawn-chunk-parity.md) should drive seed `12345`, chunk `(0, 0)` from the current `64,768 / 65,536` full-block runtime match to exact parity against the committed official-server fixture.

After that harness exists and the baseline chunk is exact, use its mismatch reports to choose the next fixture set deliberately. Likely candidates are `deep_warm_ocean` / warm-ocean exactness, a shoreline/sand boundary, a taiga/snowy slope, and a desert or badlands chunk.
