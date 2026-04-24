# Tactical 44 — Underground tail and soft disks

Finish the remaining live 1.17.1 overworld underground helpers that still sat behind the common-ore and biome-specific-extra foundation: glow lichen, rare dripstone clusters, rare small dripstone, and the soft-disk family. Keep the scope exact to the active vanilla overworld path and stop short of gameplay behavior, structures, or disabled Caves & Cliffs systems.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../data/worldgen/BiomeDefaultFeatures.java` (`addDefaultUndergroundVariety(...)`, `addDefaultSoftDisks(...)`, `addSwampClayDisk(...)`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../data/worldgen/Features.java` underground section (`DISK_*`, `RARE_DRIPSTONE_CLUSTER_FEATURE`, `RARE_SMALL_DRIPSTONE_FEATURE`, `GLOW_LICHEN`) | `src/worldgen/levelgen/feature/ore-features.ts` |
| `reference/.../world/level/levelgen/feature/GlowLichenFeature.java` | `src/worldgen/levelgen/feature/glow-lichen-feature.ts` |
| `reference/.../world/level/levelgen/feature/SmallDripstoneFeature.java` | `src/worldgen/levelgen/feature/small-dripstone-feature.ts` |
| `reference/.../world/level/levelgen/feature/DripstoneClusterFeature.java` | `src/worldgen/levelgen/feature/dripstone-cluster-feature.ts` |
| `reference/.../world/level/levelgen/feature/DripstoneUtils.java` | `src/worldgen/levelgen/feature/dripstone-utils.ts` |
| `reference/.../world/level/levelgen/Column.java` | `src/worldgen/levelgen/column.ts` |
| `reference/.../world/level/levelgen/feature/configurations/{GlowLichenConfiguration,SmallDripstoneConfiguration,DripstoneClusterConfiguration}.java` | `src/worldgen/levelgen/feature/configurations/*` |
| `reference/.../world/level/block/{GlowLichenBlock,MultifaceBlock,PointedDripstoneBlock}.java` | `src/world/level/block/*` |
| `reference/.../world/level/block/state/properties/DripstoneThickness.java` | `src/world/level/block/state/properties/dripstone-thickness.ts` |
| `reference/.../data/minecraft/tags/blocks/dripstone_replaceable_blocks.json` | `src/tags/block-tags.ts` |
| `reference/.../assets/minecraft/{blockstates,models,textures}/glow_lichen*`, `pointed_dripstone*`, `dripstone_block`, `calcite`, `clay` | `src/world/level/generated-render-blocks.ts` |

## What landed

- The translated feature registry now includes the remaining live underground feature entries:
  - `Feature.GLOW_LICHEN`
  - `Feature.DRIPSTONE_CLUSTER`
  - `Feature.SMALL_DRIPSTONE`
- The direct TS ports for `GlowLichenFeature`, `SmallDripstoneFeature`, `DripstoneClusterFeature`, `DripstoneUtils`, `Column`, and their configuration/value-provider helpers are now in place.
- Minimal block support for the translated underground features now exists:
  - `MultifaceBlock`
  - `GlowLichenBlock`
  - `PointedDripstoneBlock`
  - `DripstoneThickness`
  - `VERTICAL_DIRECTION` / `DRIPSTONE_THICKNESS` block-state properties
- `BlockTags.DRIPSTONE_REPLACEABLE` now mirrors the vanilla overworld tag used by dripstone block replacement.
- The generated render palette now registers the common underground-tail block set and their textures/render layers:
  - `clay`
  - `calcite`
  - `dripstone_block`
  - `pointed_dripstone`
  - `glow_lichen`
- `ore-features.ts` now mirrors the remaining live vanilla overworld underground configured entries:
  - `DISK_CLAY`
  - `DISK_GRAVEL`
  - `DISK_SAND`
  - `RARE_DRIPSTONE_CLUSTER_FEATURE`
  - `RARE_SMALL_DRIPSTONE_FEATURE`
  - `GLOW_LICHEN`
- `overworld-biome-generation-settings.ts` now matches the live vanilla helper wiring:
  - `addDefaultUndergroundVariety(builder, skipGlowLichen?)` includes glow lichen plus the rare dripstone tail
  - `addDefaultSoftDisks(builder)` lands sand / clay / gravel disks in the current overworld families
  - `addSwampClayDisk(builder)` keeps swamp on the vanilla clay-only disk path
  - oceans skip glow lichen but still keep the rare dripstone tail and soft disks, matching vanilla helper usage
- Focused tests now cover:
  - configured-feature shape and biome-table wiring for glow lichen, rare dripstone, and soft disks
  - direct placement smoke for glow lichen, small dripstone, and dripstone clusters
  - render-layer/default-state coverage for the new block palette
  - packed-snapshot round trips for glow lichen / pointed dripstone / dripstone block / calcite / clay
- Browser validation now includes:
  - `/tmp/mclone-debug-glow-lichen.png`
  - `/tmp/mclone-debug-dripstone-cluster.png`
  Both frames were inspected; the first shows an exposed glow-lichen sheet on stone, and the second shows a pointed dripstone formation in carved terrain.

## Scope choice

- Landed here: the rest of the active vanilla overworld underground helper surface that was still missing after tacticals 42 and 43.
- Kept intentionally narrow: no `OreVeinifier`, no disabled Caves & Cliffs cave systems, no monster-room / structure work, no extra gameplay behavior for pointed dripstone or glow lichen, and no broader underground oracle-fixture expansion yet.
- Kept future-shaped: the palette/configured-feature/biome-table surface now matches the live vanilla helper stack closely enough that later underground parity work can focus on confidence gaps or genuinely new feature families instead of missing foundation.

## Validation

- `pnpm test -- test/worldgen/levelgen/feature/ore-feature.test.ts test/worldgen/levelgen/feature/underground-decoration-feature.test.ts test/renderer/block/surface-feature-palette.test.ts test/world/packed-chunk-snapshot.test.ts` passes.
- `pnpm probe:browser -- test/browser/probes/underground-tail.probe.ts` passes.
- `/tmp/mclone-debug-glow-lichen.png` and `/tmp/mclone-debug-dripstone-cluster.png` were inspected.
- `pnpm typecheck` passes.
- `pnpm test:browser` passes.
- `pnpm perf:d5` still fails in this tree during remote debug-page boot with `TypeError: Failed to fetch` / `waitForDebugReady(...)` timeout before traversal begins.

## Next

With the live underground helper stack now covered, the next parity slice should move back to overworld table breadth and confidence:

1. Fill the remaining still-thin or still-empty overworld biome-table cases instead of continuing to widen generic underground helpers.
2. Revisit late-stage underground confidence with stronger decorated-stage/oracle coverage only if the current browser/unit surface proves too weak.
3. Keep structures, disabled Caves & Cliffs systems, and gameplay block behavior out until the target changes.
