# Tactical 60 - Monster room follow-through

Port vanilla 1.17.1 monster rooms as the next small known-missing overworld decoration slice. This stays deliberately outside the deferred structure pipeline: monster rooms are ordinary configured features in `UNDERGROUND_STRUCTURES`, not `StructureFeature` starts/references.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/MonsterRoomFeature.java` | `src/worldgen/levelgen/feature/monster-room-feature.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`MONSTER_ROOM`) | `src/worldgen/levelgen/feature/underground-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addDefaultMonsterRoom`) and `.../biome/VanillaBiomes.java` | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../extracted/assets/minecraft/{blockstates,models,textures}/**/*{cobblestone,mossy_cobblestone,spawner}*` | `src/world/level/generated-render-blocks.ts` |
| browser-smoke mutation harness | `src/runtime/host/generated-world-host-factory.ts`, `test/browser/probes/monster-room-parity.probe.ts` |

## What landed

- `MonsterRoomFeature` is now a direct port of the 1.17.1 Java room-shape logic: opening-count validation, cobblestone/mossy floor weighting, air carving, up-to-two chest attempts, and spawner placement.
- `UndergroundFeatures.MONSTER_ROOM` now mirrors the vanilla configured feature chain and `addDefaultMonsterRoom(...)` wiring, so the translated biome tables place the feature through `UNDERGROUND_STRUCTURES` instead of treating it as deferred structure work.
- The generated render palette now includes the minimal dungeon block family this slice needs: cobblestone, mossy cobblestone, spawner, and chest, plus the `FEATURES_CANNOT_REPLACE` tag placeholder the vanilla feature checks during safe writes.
- Browser validation uses a deterministic `browser_smoke` worker-world chamber with a post-placement open roof for visibility instead of hunting a naturally generated dungeon, and writes `/tmp/mclone-debug-monster-room.png`.

## Scope choice

- Landed here: the known missing monster-room configured-feature path plus the minimal runtime/render support needed to see its blocks.
- Kept intentionally narrow: no `StructureFeature` metadata pipeline, no mineshafts, and no overworld fossil follow-through yet.
- Kept intentionally honest: chest loot and spawner mob block-entity data are still deferred because the current generation/runtime layer does not persist those block entities yet.

## Oracle / done-when

**Unit (Vitest):**

- `monster-room-feature.test.ts` proves the configured feature chain, biome-table wiring, carved room shape, dungeon block placement, and the invalid-opening rejection case.
- `surface-feature-palette.test.ts` proves the render/runtime palette includes the dungeon block family the feature now places.

**Browser validation:**

- `test/browser/probes/monster-room-parity.probe.ts` captures a worker-world screenshot of the translated room at `/tmp/mclone-debug-monster-room.png`.

## Done when

- `pnpm test -- test/worldgen/levelgen/feature/monster-room-feature.test.ts test/renderer/block/surface-feature-palette.test.ts`
- `pnpm probe:browser -- test/browser/probes/monster-room-parity.probe.ts`
- `pnpm typecheck`
- `git diff --check`

## Next

Stay in the same non-structure lane and do overworld fossils next. They are the remaining small structure-looking ordinary configured feature that should land before true `StructureFeature` starts/references work.
