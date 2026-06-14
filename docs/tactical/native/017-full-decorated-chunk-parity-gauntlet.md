# 017: Full Decorated Chunk Parity Gauntlet

Status: active.

## Purpose

Drive one native Rust chunk to exact block parity with a full vanilla 1.17.1 server-generated chunk, including feature decoration and neighbor spillover effects. This tactical supersedes the previous plan to move directly from biome feature breadth to structures.

The first target is:

```text
seed: 12345
chunk: 0,0
fixture: test/fixtures/integration/overworld-seed-12345-chunks-0-0.json
status: full
generateStructures: false
```

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/WorldGenRegion.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/ChunkGenerator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/TreeFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java`
- TypeScript reference: `src/world/level/generated-decoration-region.ts`
- TypeScript reference: `src/world/level/generated-render-level.ts`
- TypeScript reference: `src/runtime/host/generated-world-host.ts`

## Current Native Gap

Native already has exact oracle coverage for terrain-only, surface/bedrock, biome/noise/PRNG, and standalone air/liquid carver stages. Native does not yet have exact full decorated chunk parity.

The full fixture for chunk `0,0` includes blocks outside the current native feature implementation, including:

- `minecraft:cave_air`
- ores such as `coal_ore`, `iron_ore`, `copper_ore`, `redstone_ore`, and deepslate ore variants
- stone variants and underground materials such as `granite`, `diorite`, `andesite`, `tuff`, and `deepslate`
- `glow_lichen`
- `snow`
- spruce logs/leaves and fern/large fern decoration
- water/lava persisted from generation

## Current Diagnostic

After wiring air/liquid carvers into the native `Features` path, the ignored exact test reports:

```text
matched_blocks: 60,528 / 65,536
mismatched_blocks: 5,008
largest buckets:
  stone -> diorite: 858
  stone -> deepslate: 856
  stone -> granite: 822
  stone -> andesite: 784
  stone -> dirt: 331
  stone -> gravel: 307
  stone -> coal_ore: 219
  air -> spruce_leaves: 215
  spruce_leaves -> air: 149
  stone -> iron_ore: 110
```

This points to underground decoration/ore families as the largest exactness gap, followed by tree placement shape/spillover. The fixture was generated with `generateStructures: false`, so structures are not part of this gauntlet.

## Landed So Far

- Native `generate_overworld_features_chunk(...)` now applies air and liquid carvers before feature decoration.
- The native block registry now names and validates the carver-written block IDs for `snow`, `lava`, `granite`, `diorite`, `andesite`, `obsidian`, and `magma_block`.
- A Rust full-chunk fixture parser expands the vanilla server fixture's section palettes into a 65,536-block expected array.
- `full_decorated_chunk_gauntlet_reports_current_native_gap` reports the current native-vs-vanilla mismatch buckets without failing the normal suite.
- `full_decorated_chunk_zero_zero_matches_java_oracle` is an ignored exact-parity test that should be unignored when the gauntlet is expected to pass.

## Required Architecture

The TypeScript implementation models feature decoration with:

```text
FEATURES_CHUNK_DEPENDENCY_RADIUS = 8
FEATURES_WRITE_RADIUS_CUTOFF = 1
```

Native needs the same durable concept before exact decorated parity is realistic:

- a `WorldGenRegion`-style feature region over multiple mutable chunks
- read access across the `FEATURES` dependency window
- writes accepted only for the center chunk plus immediate neighbors
- center exactness produced by running feature passes for the 3x3 chunks that can write into the target chunk
- lower-status dependency chunks available out to the feature read radius
- post-feature heightmap updates and scheduled tick capture

## Next Steps

1. Add a native feature-region type with read radius `8` and write cutoff `1`.
2. Change native `Features` generation from single-buffer mutation to a region pass that can write into neighboring chunks.
3. Add scheduler support for the dependency window needed to publish one exact center chunk.
4. Replace placeholder tree/vegetation profiles with vanilla feature registries for the target chunk's biome path.
5. Port ore/disk/underground decoration buckets visible in the full fixture.
6. Keep running the ignored exact test locally and reduce top mismatch buckets until it can become a normal test.

## Validation

Required checks for each sub-slice:

- `cargo test -p mclone-worldgen full_decorated_chunk_gauntlet_reports_current_native_gap`
- `cargo test -p mclone-worldgen -- --ignored full_decorated_chunk_zero_zero_matches_java_oracle` when intentionally checking the current exact-parity diff
- `cargo test --workspace`
- WASM compile gates when shared worldgen APIs change
- native headless capture when block IDs or rendered visible decoration change
