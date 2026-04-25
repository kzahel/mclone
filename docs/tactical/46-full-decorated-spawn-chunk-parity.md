# Tactical 46 — Full decorated spawn chunk parity

Reach exact full-block parity for one concrete, server-backed baseline: seed `12345`, chunk `(0, 0)`, compared against `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`.

This is intentionally not a claim of full overworld parity. It is a bounded end-to-end confidence milestone: prove that the current translated pipeline can produce one simple decorated overworld chunk exactly, then use the same harness to expand coverage later.

Status: ready to resume after the first bounded [`48-vanilla-scheduler-trace-oracle.md`](48-vanilla-scheduler-trace-oracle.md) result. [`47-generated-chunk-status-orchestration.md`](47-generated-chunk-status-orchestration.md) replaced the decorated/published shortcut with explicit `ChunkStatus`-shaped `FEATURES`, `LIGHT`, and publication gates; tactical 48 now records the seed `12345`, chunk `(0,0)` spawn-bootstrap `FEATURES` order and shows it matches the current host's z-major/x-major chunk order for the target 3x3.

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

After correcting the runtime `FEATURES` dependency model to use a `WorldGenRegion`-style read/write envelope, the full runtime decorated chunk is still close but not exact against the official-server integration fixture:

| Metric | Current value |
|---|---:|
| Full-block matches | `64,790 / 65,536` |
| Full-block mismatches | `746` |
| Full-block match rate | `98.86%` |
| Ground material matches | `232 / 256` columns |
| Exact ground block + Y matches | `225 / 256` columns |
| Unexpected dry-land sand | `0` |

Current full-block mismatch buckets:

| Bucket | Count |
|---|---:|
| Tree logs/leaves placement | `590` |
| Carver/fluid edge | `80` |
| Deep underground blobs/lava | `31` |
| Plants/snow decoration | `26` |
| Surface dirt/grass choice | `15` |
| Ores/glow lichen | `4` |

Top concrete block-pair mismatches:

```text
minecraft:air -> minecraft:spruce_leaves: 324
minecraft:spruce_leaves -> minecraft:air: 202
minecraft:air -> minecraft:spruce_log: 32
minecraft:cave_air -> minecraft:dirt: 22
minecraft:cave_air -> minecraft:grass_block: 22
minecraft:spruce_log -> minecraft:air: 22
minecraft:water -> minecraft:dirt: 21
minecraft:granite -> minecraft:deepslate: 20
minecraft:cave_air -> minecraft:air: 12
minecraft:grass_block -> minecraft:dirt: 11
minecraft:air -> minecraft:large_fern: 9
minecraft:gravel -> minecraft:deepslate: 9
```

This baseline includes two source-backed table corrections from `VanillaBiomes.taigaBiome(...)` and `BiomeDefaultFeatures`:

- taiga-family biome generation settings now keep the Java feature order for large ferns, underground variety, taiga trees, default flowers, springs, berry bushes, and top-layer freezing
- default overworld lakes/springs now wire the Java lava variants (`LAKE_LAVA`, `SPRING_LAVA`) in addition to water

## Dependency model correction

Vanilla `FEATURES` semantics are the first correctness bar for this slice:

- chunk-status dependency range is `8`
- biome decoration writes are constrained by `WorldGenRegion.ensureCanWrite(...)`
- for `FEATURES`, vanilla uses `writeRadiusCutoff = 1`

The runtime model for this tactical now follows that shape explicitly:

- published/view chunks stay at the existing client-facing radius
- a hidden authoritative chunk window stays resident for the `3x3 FULL` publication gate, `3x3 FEATURES` light input, and `FEATURES` read dependency radius
- decoration runs through a `WorldGenRegion`-style wrapper instead of the raw level
- normal feature writes are rejected outside the center chunk plus immediate neighbors

This removes the old “neighbor read accidentally generates/decorates more chunks” behavior and gives us a stable base for the remaining mismatch burn-down.

The first scheduler trace prerequisite is now available:
[`../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json`](../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json).
For the target 3x3, the observed `FEATURES` completion order is z-major/x-major:

```text
(-1,-1) -> (0,-1) -> (1,-1)
(-1, 0) -> (0, 0) -> (1, 0)
(-1, 1) -> (0, 1) -> (1, 1)
```

Use that committed trace together with `feature-order-trace` before editing tree placement. If later decorated diffs still suggest cross-chunk write ordering, regenerate a same-run scheduler trace rather than inferring a new order.

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

0. Complete tactical [`48`](48-vanilla-scheduler-trace-oracle.md).
   - Record the vanilla scheduler trace for the bounded seed `12345`, chunk `(0, 0)` load scenario.
   - Confirm the `FEATURES` completion/commit order for the target 3x3 neighborhood.
   - Compare that order against the current host order before changing tree placement.
1. Add a reusable decorated-chunk oracle diff module.
   - Compare all `16 * 16 * 256` block positions.
   - Keep the official-server fixture as the source of truth.
   - Report grouped mismatches without requiring ad hoc inline scripts.
2. Make the runtime test explicit about the current baseline.
   - Keep the existing ground-material sand regression guard.
   - Add a full-block diff assertion that can be ratcheted during the slice.
   - Do not leave the final test as a loose similarity threshold.
3. Lock the runtime dependency model before touching feature code.
   - Keep `FEATURES` reads at `±8` chunks and normal writes at `±1`.
   - Keep the host as the owner of authoritative chunks and client publication.
   - Do not let decoration reads recursively trigger unrelated chunk decoration.
4. Burn down tree placement first.
   - It still owns the majority of the remaining `746` mismatches.
   - Check spruce tree configured-feature selection, decorator coordinates, trunk height sampling, foliage radius/height sampling, and leaf/log placement predicates against Java.
5. Burn down non-tree decoration and underground helper mismatches.
   - Plants/snow: `26`
   - Deep blobs/lava: `31`
   - Ores/glow lichen: `4`
6. Resolve remaining carver/fluid and dirt/grass edges.
   - Carver/fluid edge: `80`
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
