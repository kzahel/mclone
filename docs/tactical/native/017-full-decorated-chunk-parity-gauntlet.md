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
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/TreeFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java`
- TypeScript reference: `src/world/level/generated-decoration-region.ts`
- TypeScript reference: `src/world/level/generated-render-level.ts`
- TypeScript reference: `src/runtime/host/generated-world-host.ts`

## Current Native Gap

Native already has exact oracle coverage for terrain-only, surface/bedrock, biome/noise/PRNG, and standalone air/liquid carver stages. Native does not yet have exact full decorated chunk parity.

The full fixture for chunk `0,0` still includes blocks or exact placements outside the current native feature implementation, including:

- `minecraft:cave_air`
- residual ore-placement offsets after true ore block families landed
- `glow_lichen`
- `snow`
- remaining spruce log/leaf placement offsets and fern/large fern decoration
- water/lava persisted from generation

## Current Diagnostic

After wiring air/liquid carvers into the native `Features` path, moving feature decoration onto a 3x3 region pass, correcting decorated-feature random interleaving, porting the first seven vanilla underground variety ore blobs, adding the active default ore block families, replacing the taiga placeholder with Java-shaped `TAIGA_VEGETATION` spruce/pine tree configs, and switching trees onto the Java `HEIGHTMAP_WITH_TREE_THRESHOLD` path, the ignored exact test reports:

```text
matched_blocks: 64,892 / 65,536
mismatched_blocks: 644
largest buckets:
  spruce_leaves -> air: 221
  air -> spruce_leaves: 212
  spruce_log -> air: 27
  air -> spruce_log: 25
  grass_block -> cave_air: 22
  dirt -> cave_air: 21
  dirt -> water: 21
  air -> cave_air: 14
  fern -> air: 14
  dirt -> grass_block: 11
  grass -> air: 11
  deepslate -> gravel: 9
  air -> fern: 6
  air -> large_fern: 4
  air -> snow: 4
  grass_block -> dirt: 4
```

This replaces the earlier lower `579` mismatch shortcut baseline with a more faithful Java-shaped baseline: tree positions now go through water-depth threshold plus ocean-floor heightmap placement, and the weighted `PINE` branch includes its nested vanilla `countExtra(6, 0.1, 1)` decorator. The remaining top buckets are now mostly exact spruce/pine log and leaf offsets rather than missing heightmap plumbing. Cave-air/liquid-visible feature deltas and remaining small vegetation/top-layer blocks are still visible. The target biome for chunk `0,0` is `minecraft:taiga_mountains`. The fixture was generated with `generateStructures: false`, so structures are not part of this gauntlet.

## Landed So Far

- Native `generate_overworld_features_chunk(...)` now applies air and liquid carvers before feature decoration.
- The native block registry now names and validates the carver-written block IDs for `snow`, `lava`, `granite`, `diorite`, `andesite`, `obsidian`, and `magma_block`.
- A Rust full-chunk fixture parser expands the vanilla server fixture's section palettes into a 65,536-block expected array.
- `full_decorated_chunk_gauntlet_reports_current_native_gap` reports the current native-vs-vanilla mismatch buckets without failing the normal suite.
- `full_decorated_chunk_zero_zero_matches_java_oracle` is an ignored exact-parity test that should be unignored when the gauntlet is expected to pass.
- Native `FeatureRegion` mirrors the Java/TypeScript dependency/write-window shape with read radius `8`, write cutoff `1`, metrics, blocked far writes, and mutable multi-chunk access.
- Native `generate_overworld_features_chunk(...)` now builds the union dependency window, runs surface plus air/liquid carvers for dependency chunks, and applies feature passes for the 3x3 centers that can write into the target chunk.
- Native `generate_overworld_features_chunks(...)` batches multiple publish targets through one union dependency window. A 3x3 publish view now plans 25 feature-center passes over 441 dependency chunks instead of rebuilding nine separate 361-chunk windows.
- The integrated server scheduler now batches missing `FEATURES` chunks per interest update before publishing snapshots. The `mclone-server` test suite dropped from roughly 52s to roughly 18s on this host.
- The native radius-1 headless runtime capture dropped from just over 50s to roughly 14s on this host while producing the same framed decorated terrain output.
- Native `ChunkStatusJob` records now persist completed `FEATURES` work metadata. Each generated holder `FEATURES` slot records the owning job id, and each job records its publish targets, 3x3 feature centers, dependency chunks, and queued/running/complete state.
- Native `ChunkScheduler` now routes batched feature generation through a `WorldgenMailbox`. Desktop/native uses a persistent worker thread; WASM uses the same mailbox API with inline execution until browser worker plumbing exists.
- `ChunkScheduler::apply_interest(...)` now only updates interest/holder status and enqueues feature jobs. `ChunkScheduler::poll(...)` drains completed worldgen jobs and publishes snapshots, so native command handling no longer waits for the worker thread to finish.
- `OverworldFeatureDependencyCache` retains clean liquid-carved lower-status dependency chunks inside the worldgen worker. Adjacent single-chunk feature jobs now reuse 342 of 361 dependency chunks and generate only the new 19-column strip, while repeated jobs keep deterministic output because feature writes are applied to cloned region chunks.
- Native decorated feature execution now mirrors Java's nested `DecoratedFeature` order: decorator branches are evaluated depth-first, so feature placement consumes random before the next `count` branch. This fixed the initial underground ore drift.
- Native `OreFeature` now covers the active default underground variety blobs for `dirt`, `gravel`, `granite`, `diorite`, `andesite`, `tuff`, and `deepslate`, including Java `Mth.sin` radius sampling, overlap culling, and natural-stone target matching.
- Native default ore block families now cover the active overworld `ORE_COAL`, `ORE_IRON`, `ORE_GOLD`, `ORE_REDSTONE`, `ORE_DIAMOND`, `ORE_LAPIS`, and `ORE_COPPER` features with normal/deepslate replacement targets, terrain registry IDs, asset validation, and mesh fallback colors.
- Native taiga vegetation now uses Java-shaped `RandomSelectorFeature` behavior for `TAIGA_VEGETATION`, including the `PINE` weighted branch, default `SPRUCE`, `StraightTrunkPlacer`, `SpruceFoliagePlacer`, `PineFoliagePlacer`, `TwoLayersFeatureSize`, and vanilla `countExtra(10, 0.1, 1)` placement count for the target `taiga_mountains` biome.
- Native feature placement now supports world-backed `HeightmapDecorator` and `WaterDepthThresholdDecorator` semantics over `FeatureRegion` columns, including reduced Java material predicates for `WORLD_SURFACE`, `OCEAN_FLOOR`, `MOTION_BLOCKING`, and `MOTION_BLOCKING_NO_LEAVES`.
- Native configured features now support nested `DecoratedFeature` wrappers. The weighted vanilla `PINE` branch inside `TAIGA_VEGETATION` now carries its own `countExtra(6, 0.1, 1)` decorator before tree placement.

## Reference Scheduler Notes

Java 1.17.1 does not regenerate a dependency world per published chunk. The relevant shape is:

- `ChunkHolder` owns one future slot per `ChunkStatus`.
- `ChunkMap.schedule(...)` asks the parent status first, then calls `scheduleChunkGeneration(...)` only when generation is needed.
- `ChunkMap.scheduleChunkGeneration(...)` calls `getChunkRangeFuture(center, status.getRange(), dependencyStatusFn)` to collect the required neighboring holder futures.
- For `FEATURES`, `ChunkStatus.FEATURES` has range `8`, then constructs `WorldGenRegion(..., FEATURES, 1)` and calls `ChunkGenerator.applyBiomeDecoration(...)`.
- Work is submitted through `worldgenMailbox` / `ChunkTaskPriorityQueueSorter`, so threading is layered on top of the holder/status dependency graph instead of replacing it.

Native should follow that order: first make holder/status dependency ownership explicit and reusable, then move worldgen jobs to a worker pool/mailbox. Do not add ad hoc threads around direct chunk generation calls; that would parallelize duplicate work and make parity scheduling harder to reason about.

## Required Architecture

The TypeScript implementation models feature decoration with:

```text
FEATURES_CHUNK_DEPENDENCY_RADIUS = 8
FEATURES_WRITE_RADIUS_CUTOFF = 1
```

Native has the first version of this concept:

- a `WorldGenRegion`-style feature region over multiple mutable chunks
- read access across the `FEATURES` dependency window
- writes accepted only for the center chunk plus immediate neighbors
- center exactness produced by running feature passes for the 3x3 chunks that can write into the target chunk
- batched publish-target generation reuses the union dependency window for one interest update
- durable scheduler job records link generated `FEATURES` holder slots to the batch job that produced them
- worker/mailbox execution exists for native `FEATURES` jobs, with an inline WASM fallback behind the same scheduler API
- status job publication is split into enqueue and poll phases; one-shot native callers explicitly pump the server outside command handling when they need a full response batch
- worker-local lower-status dependency materialization survives across jobs/interests and records hit/miss counters on completed `ChunkStatusJob`s

Still required:

- post-feature heightmap updates and scheduled tick capture
- full Java-style holder/status materialization for dependency protochunks; the current retained cache is worker-local clean lower-status buffers, not durable scheduler-owned dependency holders
- browser worker execution for WASM once the web runtime has worker/message plumbing

## Next Steps

1. Continue exact taiga tree parity now that placement uses the Java decorator chain: compare native `TreeFeature`, `StraightTrunkPlacer`, `SpruceFoliagePlacer`, and `PineFoliagePlacer` behavior against the source/oracle and reduce the remaining spruce log/leaf offset buckets.
2. Port the remaining taiga surface vegetation/top-layer features visible in the fixture: large ferns, snow/top-layer placement, and glow lichen/liquid-visible underground decoration as needed by the top mismatch buckets.
3. Add post-feature heightmap updates and scheduled tick capture to the native region path.
4. Promote worker-local dependency reuse into scheduler-owned holder/status protochunk slots if structures or broader status scheduling need that before `018`.
5. Keep running the ignored exact test locally and reduce top mismatch buckets until it can become a normal test.

## Validation

Required checks for each sub-slice:

- `cargo test -p mclone-worldgen full_decorated_chunk_gauntlet_reports_current_native_gap`
- `cargo test -p mclone-worldgen -- --ignored full_decorated_chunk_zero_zero_matches_java_oracle` when intentionally checking the current exact-parity diff
- `cargo test --workspace`
- WASM compile gates when shared worldgen APIs change
- native headless capture when block IDs or rendered visible decoration change
