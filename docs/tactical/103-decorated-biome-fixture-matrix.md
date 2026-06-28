# 103: Decorated Biome Fixture Matrix

Status: active.

## Purpose

Broaden decorated overworld parity beyond the seed `12345`, chunk `(0,0)`
taiga-mountains gauntlet without losing the clean scheduler `FEATURES` anchor
from `017-full-decorated-chunk-parity-gauntlet.md`.

The workflow for new biome families is:

```text
native primary-biome seed search
  -> clean scheduler FEATURES oracle at chunk (0,0)
  -> native-vs-Java block diff and mismatch buckets
  -> smallest Java-source-backed feature/decorator port
  -> committed fixture/test and matrix update
```

## Reference Source

Read before changing parity logic:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSource.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/OverworldBiomeSource.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/ConfiguredFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/DecoratedFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/placement/FeatureDecorator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`
- `native/crates/mclone-worldgen/src/biome.rs`
- `native/crates/mclone-worldgen/src/feature/tables.rs`
- `native/crates/mclone-worldgen/src/feature/placed.rs`

## Seed Selection

Keep `12345`, chunk `(0,0)` as the permanent regression anchor because it has
trace history, exact clean `FEATURES` parity, and known full-server/runtime
tails.

For new biome-family work, prefer searched seeds where chunk `(0,0)` has the
target primary biome. This keeps manual world loading, fixture paths, and oracle
traces easy to inspect while still exercising the vanilla biome-source path used
by feature table selection.

Use the native helper:

```bash
pnpm native:worldgen:find-biome-seed -- --biome plains --max-seeds 1000 --count 5
```

The helper checks `OverworldBiomeSource::get_primary_biome_definition(chunk_x,
chunk_z)`, which is the same native path used before decoration selects a biome
feature table. It accepts `--chunk-x` / `--chunk-z` for negative-coordinate or
boundary-specific fixtures, but the default biome matrix should stay at `(0,0)`
unless a bug specifically needs another coordinate.

## Oracle Fixture Command

For clean decorated snapshots, use the scheduler-trace oracle instead of the
older full-server integration fixture:

```bash
pnpm --silent oracle:gen scheduler-trace \
  --seed <seed> \
  --chunk-x 0 \
  --chunk-z 0 \
  --target-radius 1 \
  --record-radius 1 \
  --stop-status features \
  --generate-structures false \
  --dump-chunks true \
  --dump-only-target-chunk true \
  > test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-<seed>-chunk-0-0-<biome>.json
```

For feature-order or random-stream drift, generate a focused order trace:

```bash
pnpm --silent oracle:gen feature-order-trace \
  --seed <seed> \
  --chunk-x 0 \
  --chunk-z 0 \
  --generate-structures false
```

Do not use random seed breadth as the parity metric. The useful progress record
is exact fixture pass/fail plus mismatch buckets for the targeted biome.

## Matrix

| Biome family | Seed / chunk | Native table | Fixture status | Current purpose |
|---|---:|---|---|---|
| `minecraft:taiga_mountains` | `12345`, `(0,0)` | `taiga_feature_table` | exact clean `FEATURES`; older full fixture has 5 runtime-tail mismatches | permanent regression anchor from `017` |
| `minecraft:plains` | `16`, `(0,0)` | `plains_feature_table` | fixture landed; `65,359 / 65,536` blocks matched, `177` mismatches after soft disks | next low-noise vegetation baseline; first candidates from seeds `0..10000` were `16`, `17`, `27`, `41`, `67` |
| `minecraft:desert` | searched seed, `(0,0)` | `desert_feature_table` | pending | cactus/dead bush/lake and sand-family follow-up |
| `minecraft:forest` / `minecraft:birch_forest` | searched seed, `(0,0)` | `forest_feature_table` / `birch_forest_feature_table` | pending | tree selector and foliage breadth |
| `minecraft:swamp` | searched seed, `(0,0)` | `swamp_feature_table` | pending | wetter vegetation and later fluid-visible checks |
| `minecraft:badlands` | searched seed, `(0,0)` | `badlands_feature_table` | pending | surface/dead-bush/tree edge cases |
| snowy land biomes | searched seed, `(0,0)` | `snowy_feature_table` | pending | freeze/top-layer and spruce/fern follow-up |

## First Slice

Done:

1. Found plains seed `16` at chunk `(0,0)` with `native:worldgen:find-biome-seed`.
2. Generated `test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-16-chunk-0-0-plains.json`.
3. Added `plains_features_snapshot_reports_current_native_gap` beside the `017`
   gauntlet tests.
4. Ported default soft disks (`DISK_SAND`, `DISK_CLAY`, `DISK_GRAVEL`) and
   refreshed the current mismatch buckets below.

Current plains mismatch buckets:

```text
matched_blocks: 65,359 / 65,536
mismatched_blocks: 177
largest buckets:
  stone -> dripstone_block: 60
  air -> grass: 29
  grass -> air: 16
  water -> pointed_dripstone: 15
  granite -> dripstone_block: 9
  air -> pointed_dripstone: 8
  deepslate -> dripstone_block: 7
  poppy -> air: 5
  water -> glow_lichen: 4
```

Next: read the Java dripstone feature/decorator path and decide whether to port
the rare dripstone cluster/small-dripstone pair next, or first close the smaller
plains vegetation drift (`grass`, `poppy`, `tall_grass`) if that proves to be a
feature-index or decorator ordering issue.

## Validation

Focused checks for this lane:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen find_biome_seed
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen full_decorated_chunk_gauntlet_reports_current_native_gap
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen features_status_chunk_snapshot_excludes_runtime_liquid_tick_results
```

When a visible feature family changes, also run a native headless capture and
inspect the image before moving on.
