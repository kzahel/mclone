# Tactical 61 - Fossil follow-through

Port vanilla 1.17.1 overworld fossils as the last small known-missing overworld decoration slice that looks structure-like but is still an ordinary configured feature. This stays deliberately outside the deferred structure pipeline: overworld fossils are ordinary configured `Feature.FOSSIL` entries in `UNDERGROUND_STRUCTURES`, not `StructureFeature` starts/references.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/FossilFeature.java` | `src/worldgen/levelgen/feature/fossil-feature.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/FossilFeatureConfiguration.java` | `src/worldgen/levelgen/feature/configurations/fossil-feature-configuration.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`FOSSIL`) | `src/worldgen/levelgen/feature/underground-features.ts`, `src/worldgen/levelgen/feature/fossil-feature-defaults.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addFossilDecoration`) and `.../biome/VanillaBiomes.java` | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/structure/templatesystem/{StructureTemplate,StructurePlaceSettings,BlockRotProcessor,ProtectedBlockProcessor}.java` | `src/world/level/levelgen/structure/templatesystem/*`, `src/world/level/levelgen/structure/bounding-box.ts` |
| `reference/.../src/net/minecraft/data/worldgen/ProcessorLists.java` and `reference/.../extracted/data/minecraft/structures/fossil/*.nbt` | `src/worldgen/levelgen/feature/fossil-template-data.ts`, `src/world/level/generated-render-blocks.ts`, `src/tags/block-tags.ts` |
| browser-smoke mutation harness | `src/runtime/host/generated-world-host-factory.ts`, `test/browser/probes/fossil-parity.probe.ts` |

## What landed

- `FossilFeature` is now a direct port of the 1.17.1 Java placement flow: ocean-floor scan, rotated template footprint selection, empty-corner rejection, fossil processor pass, and overlay processor pass.
- `UndergroundFeatures.FOSSIL` now mirrors the vanilla configured feature chain and `addFossilDecoration(...)` wiring, so the translated biome tables place fossils through `UNDERGROUND_STRUCTURES` in desert and swamp families instead of treating them as deferred structure work.
- The runtime now has the minimal template-backed block placement support this ordinary feature needs: block-only `StructureTemplate` placement, `StructurePlaceSettings`, `BlockRotProcessor`, `ProtectedBlockProcessor`, and generated fossil template data extracted from the vanilla structure NBTs.
- The generated render palette now includes `minecraft:bone_block`, along with the minimal block/tag support fossils need during safe placement and rendering.
- Browser validation uses a deterministic `browser_smoke` worker-world fossil gallery instead of hunting a naturally generated fossil, and writes `/tmp/mclone-debug-fossil.png`.

## Scope choice

- Landed here: the known missing overworld fossil configured-feature path plus the narrow template/processor/runtime support needed for this one feature family.
- Kept intentionally narrow: no `StructureFeature` metadata pipeline, no general structure-template loading system, and no `NETHER_FOSSIL` or other true structure-start work.
- Kept intentionally honest: fossil placement is block-only for now. Chest/loot/spawner-style block-entity state remains deferred to later structure/runtime slices.

## Oracle / done-when

**Unit (Vitest):**

- `fossil-feature.test.ts` proves the configured feature chain, desert/swamp biome-table wiring, successful translated fossil placement in a synthetic solid chunk, and the empty-corner rejection rule.
- `surface-feature-palette.test.ts` proves the render/runtime palette includes `minecraft:bone_block` with the expected state/model wiring.

**Browser validation:**

- `test/browser/probes/fossil-parity.probe.ts` captures a worker-world screenshot of the translated fossil display at `/tmp/mclone-debug-fossil.png`.

## Done when

- `pnpm test -- test/worldgen/levelgen/feature/fossil-feature.test.ts test/renderer/block/surface-feature-palette.test.ts`
- `pnpm probe:browser -- test/browser/probes/fossil-parity.probe.ts`
- `pnpm typecheck`
- `git diff --check`

## Next

The ordinary configured-feature oddity lane is now complete: desert wells, monster rooms, and overworld fossils are all landed outside `StructureFeature` work. That next step landed in [`62`](./62-buried-treasure-structure-foundation.md): the real status/metadata foundation plus buried treasure as the first proof `StructureFeature`. After that, continue with desert pyramid as the next small custom structure.
