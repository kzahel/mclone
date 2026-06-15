# Tactical 35 — Jungle parity

Finish the next broad biome-identity slice after savanna by porting the missing jungle tree/decorator ecosystem and the first jungle biome tables that depend on it. This slice adds the translated jungle tree configs (`JUNGLE_TREE`, `MEGA_JUNGLE_TREE`, `JUNGLE_BUSH`), the first vine and cocoa decorators, the minimal standalone `VinesFeature`, the jungle block/render palette, and the `jungle` / `jungle_hills` / `jungle_edge` biome tables so those biomes stop falling back to the reduced generic tree mix.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/GiantTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/giant-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/trunkplacers/MegaJungleTrunkPlacer.java` | `src/worldgen/levelgen/feature/trunkplacers/mega-jungle-trunk-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/BushFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/bush-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/foliageplacers/MegaJungleFoliagePlacer.java` | `src/worldgen/levelgen/feature/foliageplacers/mega-jungle-foliage-placer.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/{TrunkVineDecorator,LeaveVineDecorator,CocoaDecorator}.java` | `src/worldgen/levelgen/feature/treedecorators/{trunk-vine-decorator,leave-vine-decorator,cocoa-decorator}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/VinesFeature.java` | `src/worldgen/levelgen/feature/vines-feature.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`JUNGLE_TREE`, `JUNGLE_TREE_NO_VINE`, `MEGA_JUNGLE_TREE`, `JUNGLE_BUSH`, `PATCH_GRASS_JUNGLE`, `PATCH_MELON`, `TREES_JUNGLE_EDGE`, `TREES_JUNGLE`, `VINES`) | `src/worldgen/levelgen/feature/{tree-features,vegetation-features,features}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addJungleTrees`, `addJungleEdgeTrees`, `addJungleGrass`, `addJungleExtraVegetation`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`baseJungleBiome`, `jungleBiome`, `jungleHillsBiome`, `jungleEdgeBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/block/{VineBlock,CocoaBlock}.java` | `src/world/level/block/{vine-block,cocoa-block}.ts` |
| `reference/.../src/net/minecraft/world/level/block/Blocks.java` (`JUNGLE_LOG`, `JUNGLE_LEAVES`, `JUNGLE_SAPLING`, `VINE`, `COCOA`, `MELON`) | `src/world/level/generated-render-blocks.ts`, `src/renderer/block/block-colors.ts` |
| `reference/.../extracted/assets/minecraft/blockstates/{jungle_log,jungle_leaves,jungle_sapling,vine,cocoa,melon}.json` | `src/world/level/generated-render-blocks.ts` |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/renderer/generated-render-level.test.ts`, `test/browser/probes/jungle-parity.probe.ts` | same |

## What landed

- `GiantTrunkPlacer`, `MegaJungleTrunkPlacer`, `BushFoliagePlacer`, and `MegaJungleFoliagePlacer` are now direct TS ports, and `TreeFeatures` includes the vanilla jungle tree family instead of leaving jungle on the oak fallback path.
- `TrunkVineDecorator`, `LeaveVineDecorator`, and `CocoaDecorator` are now real translated tree decorators, and `VinesFeature` is live for the standalone jungle vine scatter path.
- `VegetationFeatures` now includes `PATCH_GRASS_JUNGLE`, `PATCH_MELON`, `VINES`, `TREES_JUNGLE_EDGE`, and `TREES_JUNGLE`, matching the first non-bamboo jungle table that vanilla uses for `jungle`, `jungle_hills`, and `jungle_edge`.
- `overworld-biome-generation-settings.ts` now has real translated tables for `minecraft:jungle`, `minecraft:jungle_hills`, and `minecraft:jungle_edge` instead of leaving those keys on the carver-only fallback.
- The generated render palette now includes jungle logs, leaves, saplings, vine, cocoa, and melon, and jungle leaves/vines ride the biome-average foliage tint path.
- Focused tests now pin jungle tree decorator output, jungle selector output families, melon/vine follow-through, biome-table wiring, and a worker-rendered jungle browser frame.

## Scope choice

- Landed here: the first jungle slice that actually changes jungle identity in generated chunks instead of only adding biome-key mappings.
- Kept intentionally narrow: no bamboo block/feature path and no `bamboo_jungle` / `bamboo_jungle_hills` biome-table follow-through. Vanilla non-edge jungle also wants light bamboo vegetation, but that is a distinct block/feature family and should stay a separate slice instead of being hidden inside the first vine/cocoa port.
- Also kept narrow: no bee-related decorators. Jungle needed vines/cocoa first; bees still belong to a broader decorator follow-through pass.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves the translated jungle tree path places jungle logs/leaves plus decorator output (`vine`, `cocoa`) instead of silently behaving like oak.
- `vegetation-parity.test.ts` proves the jungle selector emits only the intended jungle/oak/vine/cocoa family, the melon and standalone vine paths place translated blocks, and the jungle biome keys now route through translated vegetation entries instead of the fallback.
- `generated-render-level.test.ts` stays green after the jungle palette expansion.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/jungle-parity.probe.ts` passes.
- `/tmp/mclone-debug-jungle.png` is manually inspected for a dense jungle canopy frame in the worker-generated world, even though the repo’s current green fog/clear setup still dominates the distant background.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for the jungle / generated-render-level suites
- `pnpm probe:browser -- test/browser/probes/jungle-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` all stop listing jungle core parity as the next missing biome-identity slice

## Next

Tactical 36 is now landed as [`36-snowy-giant-taiga-and-mushroom-table-coverage.md`](36-snowy-giant-taiga-and-mushroom-table-coverage.md). The next broad table-coverage win after that is shoreline and transition parity: beach, river, and ocean follow-through first, with `snowy_beach`, `ice_spikes`, and bamboo-jungle still queued behind it.
