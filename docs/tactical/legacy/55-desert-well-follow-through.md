# Tactical 55 - Desert well follow-through

Port vanilla 1.17.1 desert wells as the next small known-missing overworld decoration slice. This stays deliberately outside the deferred structure pipeline: desert wells are ordinary configured features in `SURFACE_STRUCTURES`, not `StructureFeature` starts/references.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/DesertWellFeature.java` | `src/worldgen/levelgen/feature/desert-well-feature.ts` |
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`WELL`) | `src/worldgen/levelgen/feature/vegetation-features.ts` |
| `reference/.../src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` (`addDesertExtraDecoration`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/world/level/block/SlabBlock.java`, `.../SlabType.java`, and `assets/minecraft/blockstates/sandstone_slab.json` | `src/world/level/block/slab-block.ts`, `src/world/level/block/state/properties/slab-type.ts`, `src/world/level/generated-render-blocks.ts` |

## What landed

- `DesertWellFeature` is now a direct port of the 1.17.1 Java logic: descend to sand, reject unsupported cavities, place the two-layer sandstone pad, cardinal water arms, slab ring, roof, and pillars.
- `VegetationFeatures.WELL` and `addDesertExtraDecoration(...)` now wire the vanilla `SURFACE_STRUCTURES` desert-table entry back into `desert`, `desert_hills`, and `desert_lakes`.
- The renderer/runtime block palette now includes `minecraft:sandstone_slab` with a minimal translated `SlabBlock` / `SlabType` state path so the well roof renders from vanilla assets instead of faking a full-cube substitute.
- Browser validation uses a deterministic `browser_smoke` worker-world mutation pad to place the translated feature on demand in a loaded probe chunk, avoiding a slow natural-well hunt for a `rarity(1000)` feature.

## Scope choice

- Landed here: the known missing desert-well configured-feature path plus the one missing slab state family it depends on visually.
- Kept intentionally narrow: no `StructureFeature` plumbing, no monster-room / fossil follow-through, and no general-purpose slab gameplay behavior beyond the state/model support needed for this slice.

## Oracle / done-when

**Unit (Vitest):**

- `feature-placement.test.ts` proves the translated feature places the expected sandstone / slab / water pattern on supported sand and rejects the unsupported-cavity case.
- `vegetation-parity.test.ts` proves desert biome settings carry the translated `SURFACE_STRUCTURES` well path with the vanilla rarity.
- `surface-feature-palette.test.ts` proves `minecraft:sandstone_slab` is registered, modeled, and defaults to `type=bottom`.

**Browser validation:**

- `test/browser/probes/desert-well-parity.probe.ts` captures a worker-world screenshot of the translated well at `/tmp/mclone-debug-desert-well.png`.

## Done when

- `pnpm test -- test/renderer/block/surface-feature-palette.test.ts test/worldgen/levelgen/feature/feature-placement.test.ts test/worldgen/levelgen/feature/vegetation-parity.test.ts`
- `pnpm probe:browser -- test/browser/probes/desert-well-parity.probe.ts`
- `pnpm typecheck`
- `git diff --check`

## Next

The next slice in the same spirit is the other small non-structure decorated oddities that are explicitly outside the deferred structure-start pipeline: monster rooms first, then overworld fossils.
