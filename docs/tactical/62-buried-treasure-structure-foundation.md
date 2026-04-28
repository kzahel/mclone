# Tactical 62 - Buried treasure structure foundation

Land the first true vanilla 1.17.1 `StructureFeature` slice only after the ordinary configured-feature oddities are done. Scope this to the minimum real structure pipeline: chunk-owned `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES`, clipped `StructureStart.placeInChunk(...)`, and buried treasure as the smallest proof structure.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/chunk/ChunkGenerator.java` (`createStructures`, `createReferences`, `applyBiomeDecoration`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/worldgen/levelgen/world-generator.ts`, `src/world/level/generated-render-level.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` | `src/worldgen/biome/biome.ts`, `src/worldgen/biome/biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/StructureFeatureManager.java` | `src/world/level/structure-feature-manager.ts`, `src/world/level/generated-decoration-region.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/{StructureFeature,ConfiguredStructureFeature,StructureFeatureConfiguration,StructureSettings}.java` | `src/worldgen/levelgen/structure/structure-feature.ts`, `src/worldgen/levelgen/structure/structure-features.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/structure/{StructureStart,StructurePiece,BoundingBox}.java` | `src/world/level/levelgen/structure/{structure-start,structure-piece,bounding-box}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/BuriedTreasureFeature.java` and `.../levelgen/structure/BuriedTreasurePieces.java` | `src/worldgen/levelgen/structure/{buried-treasure-feature,buried-treasure-pieces}.ts` |
| `reference/.../src/net/minecraft/data/worldgen/StructureFeatures.java` and `.../biome/VanillaBiomes.java` | `src/worldgen/levelgen/structure/structure-features.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| browser-smoke mutation harness | `src/runtime/host/generated-world-host-factory.ts`, `test/browser/probes/buried-treasure-parity.probe.ts` |

## What landed

- `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` are now real generated-status steps instead of metadata placeholders.
- Generated chunks now store starts-by-feature and references-by-feature metadata, and decoration-time lookups flow through a translated `StructureFeatureManager`.
- `Biome.generate(...)` now places referenced structures before ordinary configured features in the same decoration step, clipped to the current chunk via `StructureStart.placeInChunk(...)`.
- `BuriedTreasureFeature` is the first landed true `StructureFeature`, wired with vanilla probability/spacing and beach + snowy-beach biome eligibility.
- Browser validation uses a deterministic `browser_smoke` buried-treasure gallery and writes `/tmp/mclone-debug-buried-treasure.png`.

## Scope choice

- Landed here: the first real structure scheduler/data path plus the smallest proof structure that exercises it end to end.
- Kept intentionally narrow: no template-backed structures, no jigsaw pools, no strongholds, no mineshafts, and no noise-affecting `Beardifier` work.
- Kept intentionally honest: structure metadata is still runtime-only, and chest loot/block-entity state is still deferred even though the chest block now places through the structure path.

## Oracle / done-when

**Unit (Vitest):**

- `buried-treasure-feature.test.ts` proves biome-table wiring, start generation, reference recording, and chunk-local chest placement through the translated structure path.
- `generated-render-level.test.ts` and `generated-decoration-region.test.ts` prove the metadata-only dependency window and `FEATURES` read/write constraints that the new structure stages depend on.

**Browser validation:**

- `test/browser/probes/buried-treasure-parity.probe.ts` captures a worker-world screenshot of the buried-treasure smoke gallery at `/tmp/mclone-debug-buried-treasure.png`.

## Done when

- `pnpm test -- test/worldgen/levelgen/structure/buried-treasure-feature.test.ts test/renderer/generated-render-level.test.ts test/renderer/generated-decoration-region.test.ts`
- `pnpm probe:browser -- test/browser/probes/buried-treasure-parity.probe.ts`
- `pnpm typecheck`
- `git diff --check`

## Next

The ordinary configured-feature oddity lane came first on purpose and is now complete: desert wells, monster rooms, and overworld fossils are all landed outside `StructureFeature` work. The next true structure slice should stay in the same small custom lane and move to desert pyramid, then jungle temple and swamp hut, before mineshafts, templates, or jigsaw structures.
