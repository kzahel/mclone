# Tactical 46 — Full decorated spawn chunk parity

Reach exact full-block parity for one concrete, server-backed baseline: seed `12345`, chunk `(0, 0)`, compared against `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`.

This is intentionally not a claim of full overworld parity. It is a bounded end-to-end confidence milestone: prove that the current translated pipeline can produce one simple decorated overworld chunk exactly, then use the same harness to expand coverage later.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/chunk/ChunkGenerator.java` (`applyBiomeDecoration(...)`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/runtime/host/generated-world-host.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` (`generate(...)`) | `src/worldgen/biome/biome.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/WorldgenRandom.java` | `src/worldgen/random/worldgen-random.ts` and decoration seeding call sites |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/ConfiguredFeature.java`, `DecoratedFeature.java`, and decorators used by the `(0, 0)` biome set | `src/worldgen/levelgen/feature/`, `src/worldgen/levelgen/placement/` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/TreeFeature.java`, trunk placers, foliage placers, and tree decorators used by spruce/taiga placement | `src/worldgen/levelgen/feature/tree-feature.ts`, `src/worldgen/levelgen/feature/trunkplacers/`, `src/worldgen/levelgen/feature/foliageplacers/` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/OreFeature.java`, `DiskReplaceFeature.java`, `GlowLichenFeature.java`, underground blob features | matching TS feature ports under `src/worldgen/levelgen/feature/` |
| `reference/.../src/net/minecraft/world/level/levelgen/carver/` for any residual cave/fluid-edge mismatch | `src/worldgen/carver/` |

## Current baseline

The staged generator path is already exact for this chunk:

- `fillFromNoise(...)` terrain stage matches the Java terrain oracle block-for-block.
- `buildSurfaceAndBedrock(...)` matches the Java surface oracle block-for-block.
- AIR carvers match the Java carved oracle block-for-block, including scheduled tick capture.

The full runtime decorated chunk is close but not exact against the official-server integration fixture:

| Metric | Current value |
|---|---:|
| Full-block matches | `64,768 / 65,536` |
| Full-block mismatches | `768` |
| Full-block match rate | `98.83%` |
| Ground material matches | `232 / 256` columns |
| Exact ground block + Y matches | `225 / 256` columns |
| Unexpected dry-land sand | `0` |

Current full-block mismatch buckets:

| Bucket | Count |
|---|---:|
| Tree logs/leaves placement | `589` |
| Carver/fluid edge | `83` |
| Deep underground blobs/lava | `48` |
| Plants/snow decoration | `27` |
| Surface dirt/grass choice | `15` |
| Ores/glow lichen | `6` |

Top concrete block-pair mismatches:

```text
minecraft:air -> minecraft:spruce_leaves: 324
minecraft:spruce_leaves -> minecraft:air: 200
minecraft:air -> minecraft:spruce_log: 32
minecraft:spruce_log -> minecraft:air: 22
minecraft:cave_air -> minecraft:dirt: 22
minecraft:cave_air -> minecraft:grass_block: 22
minecraft:water -> minecraft:dirt: 21
minecraft:granite -> minecraft:deepslate: 20
minecraft:tuff -> minecraft:granite: 13
minecraft:air -> minecraft:large_fern: 12
minecraft:grass_block -> minecraft:dirt: 11
```

## Scope

Landed before this tactical: a narrow dry-land soft-disk regression test prevents sand/clay/gravel disks from placing unless their Java water-origin guard passes. This tactical starts from that fixed baseline and pushes the whole decorated chunk to exact parity.

In scope:

- reusable full-decorated chunk diff helpers that compare every `(x, y, z)` block name in the committed integration fixture
- readable mismatch grouping by feature family, y band, expected block, actual block, and first coordinates
- exact runtime parity for seed `12345`, chunk `(0, 0)`
- fixes to decoration seeding, decoration ordering, tree placement, plant placement, underground helpers, or carver/fluid edges when the oracle diff proves they are responsible
- ratcheting temporary thresholds only while actively burning down mismatches; the final state must be exact

Out of scope:

- claiming broad overworld parity from a single chunk
- structures
- disabled Caves & Cliffs Part 1 paths from `AGENTS.md`
- bee decorators or gameplay block behavior unless the `(0, 0)` oracle diff proves they affect this chunk
- new biome breadth unrelated to the pinned chunk

## Plan

1. Add a reusable decorated-chunk oracle diff module.
   - Compare all `16 * 16 * 256` block positions.
   - Keep the official-server fixture as the source of truth.
   - Report grouped mismatches without requiring ad hoc inline scripts.
2. Make the runtime test explicit about the current baseline.
   - Keep the existing ground-material sand regression guard.
   - Add a full-block diff assertion that can be ratcheted during the slice.
   - Do not leave the final test as a loose similarity threshold.
3. Verify generation context before touching feature code.
   - Confirm the runtime path decorates the same chunk neighborhood needed for vanilla feature spillover.
   - Confirm chunk decoration order, decoration seed setup, and biome feature-step iteration match Java.
4. Burn down tree placement first.
   - It owns `589 / 768` current mismatches.
   - Check spruce tree configured-feature selection, decorator coordinates, trunk height sampling, foliage radius/height sampling, and leaf/log placement predicates against Java.
5. Burn down non-tree decoration and underground helper mismatches.
   - Plants/snow: `27`
   - Deep blobs/lava: `48`
   - Ores/glow lichen: `6`
6. Resolve remaining carver/fluid and dirt/grass edges.
   - Carver/fluid edge: `83`
   - Surface dirt/grass choice: `15`
   - Re-check whether any apparent carver mismatch is actually tree/feature spillover or cross-chunk decoration context before editing carvers.
7. Promote the exact full-block test.
   - Done means `65,536 / 65,536` block names match for seed `12345`, chunk `(0, 0)`.

## Validation

Required:

- `pnpm test -- test/runtime/generated-world-boundary.test.ts`
- `pnpm test -- test/worldgen/levelgen/noise-based-chunk-generator.test.ts test/worldgen/levelgen/surface-oracle-fixture.test.ts test/worldgen/levelgen/carver-oracle-fixture.test.ts`
- targeted feature/decorator tests for every port fix made during the slice
- `pnpm typecheck`

Run browser validation if a fix changes rendered pixels materially:

- `pnpm test:browser`
- the smallest relevant `pnpm probe:browser -- test/browser/probes/<name>.probe.ts`, with screenshots saved to `/tmp`

## Done when

- the full decorated runtime chunk `(0, 0)` for seed `12345` matches the committed official-server fixture at all `65,536` block positions
- the exact comparison is encoded in a normal test, not only a diagnostic script
- the existing staged terrain/surface/carver oracle tests remain exact
- the dry-land sand regression stays covered
- docs are updated with the final mismatch count of `0`

## Next

After this lands, use the same full-decorated diff harness to choose the next small fixture set deliberately:

1. one beach/river boundary where sand is legitimate
2. one taiga/snowy slope like the visual regression area
3. one desert or badlands chunk where loose material checks must not overfit grassland assumptions
4. one ocean/shoreline chunk if the remaining table exactness issues are still visible
