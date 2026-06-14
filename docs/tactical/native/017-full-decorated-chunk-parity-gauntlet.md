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

After wiring air/liquid carvers into the native `Features` path, moving feature decoration onto a 3x3 region pass, correcting decorated-feature random interleaving, porting the first seven vanilla underground variety ore blobs, adding the active default ore block families, replacing the taiga placeholder with Java-shaped `TAIGA_VEGETATION` spruce/pine tree configs, switching trees onto the Java `HEIGHTMAP_WITH_TREE_THRESHOLD` path, enabling the Java taiga vegetal prefix (`PATCH_LARGE_FERN`, `GLOW_LICHEN`, then `TAIGA_VEGETATION`), and removing the erroneous nested `countExtra(6, 0.1, 1)` wrapper from the weighted `PINE` branch, the ignored exact test reports:

```text
matched_blocks: 65,221 / 65,536
mismatched_blocks: 315
largest buckets:
  coal_ore -> stone: 71
  stone -> coal_ore: 67
  spruce_leaves -> air: 23
  grass_block -> cave_air: 22
  dirt -> cave_air: 22
  dirt -> water: 21
  air -> cave_air: 13
  fern -> air: 13
  deepslate -> gravel: 9
  dirt -> grass_block: 7
  grass -> air: 7
  large_fern -> air: 7
  air -> fern: 6
  air -> dandelion: 4
  air -> snow: 4
  deepslate -> deepslate_coal_ore: 3
```

This replaces the previous `1270` mismatch baseline, which was inflated by a native-only pine sub-count. Java `TAIGA_VEGETATION` selects one weighted `PINE` or default `SPRUCE` per outer placement; it does not decorate `PINE` with an inner count. With that corrected, the top remaining blockers are coal ore placement drift, cave-air/liquid-visible surface deltas, small vegetation/top-layer features, and a much smaller residual spruce-leaf difference. The target biome for chunk `0,0` is `minecraft:taiga_mountains`. The fixture was generated with `generateStructures: false`, so structures are not part of this gauntlet.

Java `taigaBiome(...)` has two `VEGETAL_DECORATION` features before `TAIGA_VEGETATION`: `PATCH_LARGE_FERN` from `addFerns(...)`, then `GLOW_LICHEN` from `addDefaultCrystalFormations(...)`. Native now has the same local order, so the tree selector runs at vegetal feature index `2`. The native 3x3 feature-center order for this fixture is trace-guarded against `test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json`; do not chase this by reordering centers unless a new scheduler trace says to.

A focused native diagnostic can force only `TAIGA_VEGETATION` at feature index `2` over real liquid-carved terrain, one center at a time. For target chunk `(0,0)`, native isolated index-2 output now writes target trees from three nearby centers: `(0,-1)` writes `9`, `(0,0)` writes `236`, `(1,0)` writes `12`, and `(0,1)` writes `14`; `(-1,-1)`, `(1,-1)`, `(-1,0)`, and `(1,1)` write no target trees in this isolated diagnostic; `(-1,1)` is not a taiga-family center.

The Java scheduler trace oracle can now record full target-chunk spruce logs/leaves after each configured feature with `--probe-target-tree-blocks true`. A local diagnostic trace for seed `12345`, target chunk `(0,0)`, and `generateStructures=false` showed the actual vanilla full-table tree delta at `stepIndex=8`, `featureIndex=2`: center `(0,0)` adds `236` target tree blocks and center `(1,0)` adds `10`; the other target 3x3 centers add none at that feature event. Native now has an ignored Rust diagnostic for the same full-table event. It still shows the `23` extra target leaves from centers `(0,-1)` and `(0,1)`, while matching Java's per-feature random-call counts for the main tree centers.

Follow-up Java probes with target chunk `(0,-1)` showed why at least the north spillover survives natively: the disputed native trunk at local `(1, z=15)` would need a survivable block below `y=91`, but the server scheduler trace has `minecraft:cave_air` at `(localX=1, y=90, localZ=15)` after glow lichen and before `TAIGA_VEGETATION`. Native's dependency terrain lets that tree survive, so the remaining spruce-leaf bucket is currently a dependency terrain/carver parity symptom, not a conifer foliage-shape bug. The verbose Java traces live in `/tmp` during investigation and should not be committed unless reduced to a small fixture.

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
- Native configured features now support nested `DecoratedFeature` wrappers for configured features that use them. The weighted `PINE` branch inside `TAIGA_VEGETATION` is direct, matching Java 1.17.1's `RandomFeatureConfiguration`.
- Native block/asset registries and `RandomPatchFeature` support lower/upper `large_fern` blocks, and the taiga table now enables Java's `PATCH_LARGE_FERN` prefix before trees.
- Native block/asset registries include compact `glow_lichen` state support, and the taiga table now enables a Java-shaped `GLOW_LICHEN` feature before trees. The native block buffer still stores this as one compact state rather than full multiface/waterlogged state.
- Native feature-center commit order now has a Rust test against the committed vanilla scheduler trace for seed `12345`, chunk `(0,0)`. The current order is z-major over the target 3x3: `(-1,-1)`, `(0,-1)`, `(1,-1)`, then the next rows.
- Native test support can place only `TAIGA_VEGETATION` with a forced vegetal feature index. `taiga_vegetation_feature_index_shift_is_isolated_by_center` now guards that native's active taiga tree selector uses Java full-biome index `2`.
- Native feature table selection now uses Java's primary chunk biome path (`BiomeSource.getPrimaryBiome`) instead of fuzzy block-position biome lookup at the chunk center.
- Native has an ignored full-table taiga tree diagnostic, `taiga_full_table_tree_deltas_match_vanilla_scheduler_probe`, pinned to Java's `(0,0): +236` and `(1,0): +10` scheduler-probe deltas. It intentionally remains ignored until dependency terrain/carver parity stops the north/south spillover trees.
- `WorldgenRandom` now has a focused negative-`z` decoration feature-seed oracle test for the `TAIGA_VEGETATION` center `(0,-1)`.

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

1. Investigate server-backed cave/carver dependency terrain around the north/south tree spillover roots. The immediate known probe is Java target `(0,-1)`, local `(1,90,15)`, which is `cave_air` in the scheduler trace but survivable terrain natively when center `(0,-1)` runs `TAIGA_VEGETATION`.
2. Investigate the coal ore placement drift (`coal_ore -> stone` and `stone -> coal_ore`) once the cave/dependency terrain issue is understood, because ore is now the largest remaining bucket.
3. Port the remaining taiga surface vegetation/top-layer features visible in the fixture: snow/top-layer placement, liquid-visible underground decoration, and remaining grass/flower/mushroom/sugar-cane/pumpkin/berry-bush patches as needed by the top mismatch buckets.
4. Add post-feature heightmap updates and scheduled tick capture to the native region path.
5. Promote worker-local dependency reuse into scheduler-owned holder/status protochunk slots if structures or broader status scheduling need that before `018`.
6. Keep running the ignored exact test locally and reduce top mismatch buckets until it can become a normal test.

## Validation

Required checks for each sub-slice:

- `cargo test -p mclone-worldgen full_decorated_chunk_gauntlet_reports_current_native_gap`
- `cargo test -p mclone-worldgen -- --ignored full_decorated_chunk_zero_zero_matches_java_oracle` when intentionally checking the current exact-parity diff
- `cargo test --workspace`
- WASM compile gates when shared worldgen APIs change
- native headless capture when block IDs or rendered visible decoration change
