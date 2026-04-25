# Tactical 51 - Bee tree-decorator follow-through

Finish the last broad, clearly known-missing tree-decorator slice by porting vanilla bee-nest tree decoration and wiring the bee-tagged oak/birch/fancy-oak variants back into the live flower-forest, birch, and plains selectors.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/BeehiveDecorator.java` | `src/worldgen/levelgen/feature/treedecorators/beehive-decorator.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/TreeDecoratorType.java` (`BEEHIVE`) | `src/worldgen/levelgen/feature/tree-features.ts`, translated decorator set |
| `reference/.../src/net/minecraft/world/level/block/BeehiveBlock.java` | `src/world/level/block/bee-nest-block.ts`, `src/world/level/block/state/properties/block-state-properties.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{Features,BiomeDefaultFeatures}.java` (`OAK_BEES_*`, `BIRCH_BEES_*`, `FANCY_OAK_BEES_*`, `SUPER_BIRCH_BEES_0002`, flower-forest/birch/plains selectors) | `src/worldgen/levelgen/feature/{tree-features,vegetation-features}.ts` |
| `reference/.../extracted/assets/minecraft/{blockstates,models,textures}/**/*bee_nest*` | `src/world/level/generated-render-blocks.ts` |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/renderer/block/surface-feature-palette.test.ts`, `test/browser/probes/bee-decorator-parity.probe.ts` | same |

## What landed

- `BeehiveDecorator` is now a direct TS port, including the vanilla probability gate, trunk/leaf Y selection, offset choice, and south-facing `bee_nest` placement check.
- `BeeNestBlock` and `honey_level` block-state support are now present in the generated block palette, with the extracted `bee_nest` model/textures and solid render-layer wiring.
- `TreeFeatures` now exposes the vanilla bee-tagged tree variants: `SUPER_BIRCH_BEES_0002`, `OAK_BEES_0002/_002/_005`, `BIRCH_BEES_0002/_002/_005`, and `FANCY_OAK_BEES_0002/_002/_005`.
- `VegetationFeatures` now routes the live consumers through those variants the same way vanilla does: `FOREST_FLOWER_TREES`, `BIRCH_TALL`, `TREES_BIRCH`, `BIRCH_OTHER`, and `PLAIN_VEGETATION`.
- Targeted validation covers block placement, selector wiring, palette/render support, and a browser probe. The validated frame is `/tmp/mclone-debug-bee-decorator.png`, which shows a generated bee nest attached under a plains-oak canopy.

## Scope choice

- Landed here: worldgen-visible bee-nest placement parity plus the block/render support required to see it in generated chunks.
- Kept intentionally narrow: no bee AI, no hive occupancy/runtime behavior, and no general block-entity storage layer. This slice only needs the generated block placement that the vanilla decorator writes during `FEATURES`.
- Divergence kept explicit: the decorator does not populate stored bee occupants yet because the current generation/runtime path has no block-entity persistence surface for that data.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves the bee-decorated oak path can emit a translated `bee_nest` block with the expected facing and honey-level defaults.
- `vegetation-parity.test.ts` proves the live flower-forest, birch, and plains selectors now point at the translated bee-decorated variants with the vanilla probabilities.
- `surface-feature-palette.test.ts` proves the generated render palette registers `minecraft:bee_nest`, its render layer, and its sprite coverage.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/bee-decorator-parity.probe.ts` passes.
- `/tmp/mclone-debug-bee-decorator.png` is manually inspected and visibly shows a generated bee nest in the worker-generated world.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for tree, vegetation-parity, and surface-palette coverage
- `pnpm probe:browser -- test/browser/probes/bee-decorator-parity.probe.ts` passes
- `docs/tactical/README.md` and `docs/worldgen-status.md` stop listing bees as the next known missing decoration slice

## Next

Stay in the same "known missing simple decorations" lane and do sunflower plains next: port the sunflower-plains-specific `PATCH_SUNFLOWER` path and wire it into the existing plains / sunflower-plains biome tables before chasing narrower table exactness.
