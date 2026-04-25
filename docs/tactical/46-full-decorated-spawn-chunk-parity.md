# Tactical 46 — Full decorated spawn chunk parity

Reach exact full-block parity for one concrete, server-backed baseline: seed `12345`, chunk `(0, 0)`, compared against `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`.

This is intentionally not a claim of full overworld parity. It is a bounded end-to-end confidence milestone: prove that the current translated pipeline can produce one simple decorated overworld chunk exactly, then use the same harness to expand coverage later.

Status: exact for the pinned fixture after the fixture-equivalent generated-liquid tick window. [`47-generated-chunk-status-orchestration.md`](47-generated-chunk-status-orchestration.md) replaced the decorated/published shortcut with explicit `ChunkStatus`-shaped `FEATURES`, `LIGHT`, and publication gates; tactical 48 records the seed `12345`, chunk `(0,0)` spawn-bootstrap `FEATURES` order and shows it matches the current host's z-major/x-major chunk order for the target 3x3.

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

After correcting the runtime `FEATURES` dependency model to use a `WorldGenRegion`-style read/write envelope, fixing `RepeatingDecorator` to sample its count once like Java, preserving vanilla `LakeFeature` `cave_air`, regenerating the server-backed fixture through the scheduler-pinned integration harness, and making feature-time block-light reads match vanilla's pre-lighting darkness, the static post-generation snapshot is close but intentionally still short of the measured official-server fixture:

| Comparison | Full-block matches | Full-block mismatches |
|---|---:|---:|
| Static host snapshot, `liquidSimulationMode: none` | `65,533 / 65,536` | `3` |
| Static host snapshot, `liquidSimulationMode: vanilla17`, before host ticks | `65,533 / 65,536` | `3` |
| `vanilla17` liquid simulation after `1` deterministic host tick | `65,535 / 65,536` | `1` |
| `vanilla17` liquid simulation after `10` deterministic host ticks | `65,536 / 65,536` | `0` |

The static mismatch cells are all fluid follow-through:

```text
minecraft:water -> minecraft:air: 2
minecraft:lava -> minecraft:air: 1
```

The scheduler trace probe for those three positions shows they are still `minecraft:air` at the end of `FEATURES`; they appear in the committed official-server fixture only after the server continues startup and executes generated liquid ticks. This means the remaining exactness question is fixture-equivalent host tick timing, not another decoration, carver, or feature-placement bug.

[`test/runtime/generated-world-boundary.test.ts`](../../test/runtime/generated-world-boundary.test.ts) now encodes that distinction: it first asserts the `3` pre-tick fluid differences, advances `10` deterministic host ticks with `liquidSimulationMode: vanilla17`, then asserts exact `65,536 / 65,536` full-block parity.

Ground surface remains exact throughout this window:

| Metric | Current value |
|---|---:|
| Ground material matches | `256 / 256` columns |
| Exact ground block + Y matches | `256 / 256` columns |
| Unexpected dry-land sand | `0` |

This baseline includes two source-backed table corrections from `VanillaBiomes.taigaBiome(...)` and `BiomeDefaultFeatures`:

- taiga-family biome generation settings now keep the Java feature order for large ferns, underground variety, taiga trees, default flowers, springs, berry bushes, and top-layer freezing
- default overworld lakes/springs now wire the Java lava variants (`LAKE_LAVA`, `SPRING_LAVA`) in addition to water
- `RepeatingDecorator.getPositions(...)` now evaluates `count(...)` once before looping, matching Java `IntStream.range(0, this.count(...))`; this fixed the target spruce trunk origins exactly.
- `LakeFeature` now writes and snapshots `minecraft:cave_air` for upper lake cavities like Java `Blocks.CAVE_AIR`, removing the `59`-block `cave_air -> air` mismatch bucket.
- the committed full decorated fixture is now generated with `./oracle/integration/gen-fixture.sh --scheduler-pins ...` to use the same JVM scheduler pins as the scheduler trace oracle; treat it as an exact empirical run fixture, because edge decoration writes are scheduler/run-shape sensitive.
- `GeneratedDecorationRegion` now reports block light `0` during feature placement, matching vanilla's pre-lighting `WorldGenRegion` behavior. This lets `SnowAndFreezeFeature` place the same cold-slope snow blockers that reject the extra neighboring `(0,1)` spruce and removes the tree/leaves plus plants/snow mismatch buckets.
- `LevelChunk` now keeps `WORLD_SURFACE_WG` / `OCEAN_FLOOR_WG` as pre-feature heightmaps and restricts decoration writes to vanilla's post-feature heightmaps. This prevents neighboring tree leaves from changing later ore height gates and removes the `10` gravel/deepslate mismatches.
- `liquidSimulationMode: vanilla17` reaches exact fixture parity after `10` deterministic host ticks. With liquid simulation disabled, or before host ticks are allowed to run, the expected fixture-fluid cells remain air.

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
   - Center-chunk spruce trunk origins now match the oracle exactly after the `RepeatingDecorator` count fix.
   - Edge leaves from the neighboring `(0,1)` tree are now gone after matching feature-time block light and top-layer snow behavior.
5. Burn down non-tree decoration and underground helper mismatches.
   - Plants/snow: `0`
   - Deep blobs/lava after fixture-equivalent liquid ticks: `0`
6. Encode fixture-equivalent liquid tick timing in the full parity harness.
   - Done: static post-generation diff before host liquid ticks: `3`
   - Done: `vanilla17` diff after `10` deterministic host ticks: `0`
   - Surface dirt/grass choice: `0`
7. Promote the exact full-block test.
   - Done: `65,536 / 65,536` block names match for seed `12345`, chunk `(0, 0)` under the same generated-fluid tick window represented by the official-server fixture.

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

- the full decorated runtime chunk `(0, 0)` for seed `12345` matches the committed official-server fixture at all `65,536` block positions after the fixture-equivalent generated-liquid tick window
- the exact comparison is encoded in a normal test, not only a diagnostic script
- the existing staged terrain/surface/carver oracle tests remain exact
- the dry-land sand regression stays covered
- docs are updated with the final mismatch count of `0`

## Next

After this lands, tactical [`50`](50-beach-river-full-decorated-parity.md) should use the same full-decorated diff harness on seed `12345`, chunk `(5,115)`, the existing sand/gravel surface-oracle target. That is the best second fixture because it proves legitimate shoreline/river loose material, soft disks, fluids, and decoration all stay exact together.

After that, choose the next small fixture set from measured full-block diffs rather than from broad biome breadth:

1. one taiga/snowy slope like the visual regression area
2. one desert or badlands chunk where loose material checks must not overfit grassland assumptions
3. one ocean/shoreline chunk if the remaining table exactness issues are still visible
