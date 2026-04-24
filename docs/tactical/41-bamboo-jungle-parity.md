# Tactical 41 — Bamboo-jungle parity

Finish the remaining broad jungle-family follow-through by porting the bamboo block/feature path, wiring the vanilla bamboo vegetation selectors, and giving `bamboo_jungle` / `bamboo_jungle_hills` real translated biome tables instead of leaving them on fallback settings.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/block/state/properties/{BambooLeaves,BlockStateProperties}.java` | `src/world/level/block/state/properties/{bamboo-leaves,block-state-properties}.ts` |
| `reference/.../src/net/minecraft/world/level/block/{BambooBlock,BambooSaplingBlock,Blocks}.java` | `src/world/level/block/{bamboo-block,bamboo-sapling-block}.ts`, `src/world/level/generated-render-blocks.ts`, `src/world/level/block/sound-type.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/BambooFeature.java` | `src/worldgen/levelgen/feature/bamboo-feature.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{Features,BiomeDefaultFeatures}.java` | `src/worldgen/levelgen/feature/{features,vegetation-features}.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`baseJungleBiome`, `bambooJungleBiome`, `bambooJungleHillsBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/data/minecraft/tags/blocks/bamboo_plantable_on.json` | `src/tags/block-tags.ts` |
| `reference/.../src/assets/minecraft/{blockstates,models,textures}/**/*bamboo*` | `src/world/level/generated-render-blocks.ts` |
| `reference/.../src/net/minecraft/client/renderer/ItemBlockRenderTypes.java` | `src/world/level/generated-render-blocks.ts`, `test/renderer/block/surface-feature-palette.test.ts` |
| `test/worldgen/levelgen/feature/vegetation-parity.test.ts`, `test/renderer/block/surface-feature-palette.test.ts`, `test/browser/probes/bamboo-jungle-parity.probe.ts` | same |

## What landed

- `BambooLeaves`, the missing bamboo block-state properties, `BambooBlock`, and `BambooSaplingBlock` are now real TS ports, with the minimal survival/tag logic the feature path depends on.
- `BlockTags` now exposes `BAMBOO_PLANTABLE_ON`, and the generated render palette now registers `minecraft:bamboo` / `minecraft:bamboo_sapling`, their sprites, and their cutout render-layer wiring.
- `BambooFeature` is now a direct TS port instead of a missing registry hole, including the podzol-ring branch behind the vanilla probability config.
- `Features` and `VegetationFeatures` now expose the vanilla bamboo entries: `BAMBOO_LIGHT`, `BAMBOO`, and `BAMBOO_VEGETATION`.
- `overworld-biome-generation-settings.ts` now mirrors the vanilla jungle branch split: `jungle` / `jungle_hills` get light bamboo follow-through, `jungle_edge` stays tree-only, and `bamboo_jungle` / `bamboo_jungle_hills` now use the dedicated bamboo vegetation path instead of falling back.
- Browser validation now includes a dedicated bamboo-jungle frame at `/tmp/mclone-debug-bamboo-jungle.png`, and the shot clearly reads as a dense bamboo thicket with podzol running through the slope.

## Scope choice

- Landed here: the broad bamboo-jungle identity slice needed to stop the last obviously reduced jungle-family biome from falling back.
- Kept intentionally narrow: no bee-related tree decorators, no later bamboo random-tick/runtime growth loop parity, and no `potted_bamboo` / scaffolding follow-through outside the generation-time path.
- Kept honest: the browser frame is a debug free-cam validation shot, not a polished cinematic render, but it visibly shows the bamboo-heavy biome identity this slice was meant to restore.

## Oracle / done-when

**Unit (Vitest):**

- `vegetation-parity.test.ts` proves the translated bamboo feature places bamboo and podzol, the bamboo-jungle selector emits the intended jungle-grass/tree family, and the bamboo-jungle biome keys now expose the translated feature tables.
- `surface-feature-palette.test.ts` proves the renderer keeps `bamboo` and `bamboo_sapling` in the expected cutout layer.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/bamboo-jungle-parity.probe.ts` passes.
- `/tmp/mclone-debug-bamboo-jungle.png` is manually inspected and reads as a dense bamboo-jungle slope rather than generic jungle canopy.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for bamboo-jungle vegetation and render-palette coverage
- `pnpm probe:browser -- test/browser/probes/bamboo-jungle-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` stop listing bamboo-jungle as the next missing broad parity slice

## Next

With the broad bamboo-jungle follow-through landed, the next parity slice should move underground: ore / underground decoration is now the biggest missing “this is actually Minecraft” content gap, with bee-related decorators as the remaining major surface-ecosystem follow-through behind it.
