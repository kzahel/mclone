# 019: Scheduler-Owned Dependency Holders

Status: completed.

## Purpose

Promote chunk scheduling from direct client-interest chunks to a Java-shaped residency graph with propagated ticket levels and scheduler-owned lower-status dependency holders. This keeps chunk generation, retention, and future ticking decisions in the server scheduler instead of hiding them inside one feature worker cache.

This slice keeps client publication narrow: only chunks directly requested by current client interest are sent as `ChunkSnapshot` updates. Propagated and forced chunks may stay resident without being visible to the client.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTracker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

Key Java facts for this slice:

- `ChunkTracker` propagates levels through the eight neighboring chunks. Each Chebyshev step adds one level.
- `DistanceManager.ChunkTicketTracker` sets `ChunkHolder.ticketLevel` from propagated ticket levels, not just direct tickets.
- `ChunkHolder` uses ticket level to decide accessible status and full/ticking/entity-ticking state.
- `ChunkMap.updateChunkScheduling(...)` owns holder creation/removal from ticket levels; client tracking is a separate concern handled by view-distance/player tracking.

## Landed In First Slice

- Native `ChunkDistanceManager` now derives an active propagated level map from direct tickets using Java's Chebyshev neighbor shape.
- `ChunkScheduler` reconciles holders against propagated active levels, so a radius-1 client interest now owns a `29x29` active holder area while publishing only the `3x3` client-visible chunks.
- `ChunkHolder` separates resident snapshot state from `client_visible` state. Interest movement emits client unloads without necessarily dropping server-resident holders.
- Forced chunks can keep resident server state after a client moves away, without keeping that chunk visible to the client.
- Feature worker dependency state is now scheduler-owned:
  - completed jobs return retained clean lower-status dependency buffers
  - scheduler stores those buffers on dependency holders
  - later feature jobs seed the worker from scheduler-owned dependency buffers
  - the native worker no longer depends on hidden worker-local cache state across jobs
- `ChunkSchedulerMetrics` exposes ticket, holder, visibility, dirty, job, and dependency-reuse counters.
- Tests now assert direct ticket count, propagated active holder count, client-visible count, dependency buffer count, forced retention, stale ticket expiry, and adjacent feature-job dependency seeding.
- `scheduler_movement_smoke` now runs a deterministic adjacent chunk-interest path, validates scheduler invariants, and prints JSON metrics/timing.
- Feature-generation jobs now report phase timings for dependency seeding, dependency generation, retained dependency cloning, feature-region setup, decoration, and target extraction.
- `FeatureRegion` stores dependency chunks in an indexed rectangle instead of a `BTreeMap`, and hot decoration uses cached immutable biome feature tables instead of rebuilding feature vectors for each center.
- Scheduler batch feature generation bypasses `DecorationReport.added_non_air_blocks` accounting, which otherwise scans the whole feature-region dependency window before and after every decoration center. Public decoration report APIs still compute that field for tests/debug callers.
- Lower-status dependency generation now reuses one `NoiseBasedChunkGenerator` per feature job instead of recreating it for every dependency chunk, and reports generator setup, surface fill, surface/bedrock, air carver, liquid carver, and heightmap timing.
- Ticket removal now queues holders in a native pending-unload set instead of dropping/saving them synchronously. Returned tickets rescue pending holders, `ChunkScheduler::process_pending_unloads(...)` drains a bounded amount of save/drop work, `tick()` processes the Java-shaped default budget, and native window mode ticks the integrated server each frame.
- `ChunkSchedulerMetrics` now exposes exact active full-status rings (`Inaccessible`, `Border`, `Ticking`, `EntityTicking`) plus the block-ticking lane. Tests assert the vanilla player-ticket radius math and that client visibility is independent from ticking status.
- `ChunkScheduler::tick_report()` and `IntegratedServer::tick_report()` now expose deterministic z-major block-ticking and entity-ticking chunk lists, bounded pending-unload drain counts, and protocol updates/events. Native window mode uses the report and shows the tick id plus last unload-drain count in the title.
- `IntegratedServer::simulation_tick_report()` now consumes the chunk tick report through no-op block/entity phases, records phase counts and native timing, and preserves existing `tick()` protocol-update behavior. Native window diagnostics show simulation tick id, phase counts, and phase timing.
- After scheduled fluid ticks landed, chunks in the block-ticking or entity-ticking lanes are promoted to full generated live chunks even when they are not client-visible. This keeps client publication narrow while giving runtime systems mutable block access across chunk boundaries.

## Current Limits

- Propagated non-direct holders are treated as clean lower-status dependency holders. Native does not yet generate Java's full feature-status halo for `level 34` chunks.
- The clean dependency buffer is still stored as a worldgen `MutableChunkBlockBuffer`, not a long-lived protochunk type with explicit status transitions.
- Moving scheduler-owned buffers through the worker currently clones chunk buffers. Current profiling shows this is not the first bottleneck for radius-1 movement, but performance work should still replace it with shared/owned transfer accounting once the shape stabilizes.
- Block-ticking chunks now have live generated block buffers for scheduled fluid mutation. General block ticks, random ticks, entity ticking, and spawn systems are still not implemented.
- The smoke records split timing, but no fixed perf budget is enforced yet. Use release mode for performance comparisons.

## Movement Smoke

Default command:

```bash
pnpm native:scheduler:smoke
```

Release-mode perf command:

```bash
pnpm native:scheduler:perf
```

The smoke defaults to seed `12345`, radius `1`, `3` adjacent movement steps, and `poll_mode=completion`. Completion mode waits for the native worldgen worker to post a completed job, then calls nonblocking `poll()` once to publish it. It fails if:

- direct ticket count differs from the client-visible interest square
- propagated active holder count differs from the expected Chebyshev ticket halo
- pending unloads remain after the smoke's explicit unload-drain step
- full-status/ticking-lane counts differ from the expected player-ticket rings
- client-visible chunk count differs from direct interest
- movement steps stop publishing one new strip and one unload strip
- scheduler-owned dependency seeding no longer matches worker cache hits

Timing fields:

- `elapsed_ms`: end-to-end latency for applying one interest center and waiting until scheduled worldgen is idle.
- `apply_interest_ms`: synchronous ticket/holder reconciliation and job enqueue time.
- `poll_call_ms`: total synchronous caller-thread time spent inside scheduler `poll()` calls.
- `main_thread_scheduler_ms`: `apply_interest_ms + poll_call_ms`.
- `main_thread_publish_path_ms`: `apply_interest_ms + publish_poll_call_ms`, the frame-sensitive path when only completion polls return events.
- `completion_wait_ms`: time spent blocked in the smoke waiting for worker completion notification. This is a diagnostic/server wait path, not the renderer frame path.
- `main_thread_blocked_ms`: `apply_interest_ms + completion_wait_ms + poll_call_ms`; useful for headless smoke latency, not a frame budget when the app uses nonblocking polling.
- `worker_wait_ms`: wall-clock wait after `apply_interest`, including poll sleeps and poll calls.
- `non_poll_wait_ms`: wall-clock wait minus time spent inside `poll()`.
- `max_main_thread_call_ms`: largest single synchronous scheduler call in the step.
- `feature_timing`: worker-side feature-generation phase timings. `feature_decoration_ms` is the active biome feature placement loop; `feature_decoration_steps` breaks that loop down by vanilla `DecorationStep`; `dependency_generate_ms` is lower-status carved dependency generation; `dependency_generation` breaks that down into generator setup, surface fill, surface/bedrock, air carvers, liquid carvers, and heightmap priming; clone/retain fields measure current buffer-copy overhead.

Observed debug-mode baseline on this host with default `poll_mode=completion`:

```text
steps: 3
radius: 1
completion_wait_ms: 30,000 timeout cap, zero observed timeouts
total_elapsed_ms: ~7,041
step 0 elapsed_ms: ~6,042, feature_total_ms: ~5,854, dependency_generate_ms: ~5,351, surface_fill_ms: ~2,661, surface_bedrock_ms: ~1,193, air_carvers_ms: ~663, feature_decoration_ms: ~493, polls: 1, snapshots 9, unloads 0
step 1 elapsed_ms: ~499, feature_total_ms: ~456, dependency_generate_ms: ~240, surface_fill_ms: ~118, surface_bedrock_ms: ~51, air_carvers_ms: ~34, feature_decoration_ms: ~210, polls: 1, snapshots 3, unloads 3
step 2 elapsed_ms: ~493, feature_total_ms: ~450, dependency_generate_ms: ~238, surface_fill_ms: ~119, surface_bedrock_ms: ~51, air_carvers_ms: ~32, feature_decoration_ms: ~209, polls: 1, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  inaccessible_status_chunks: 792
  border_status_chunks: 24
  ticking_status_chunks: 16
  entity_ticking_status_chunks: 9
  block_ticking_chunks: 25
  client_visible_chunks: 9
  loaded_snapshot_chunks: 35
  dirty_chunks: 35
  ready_dependency_chunks: 575
  total_seeded_dependency_chunks: 828
  total_dependency_cache_hits: 828
  total_dependency_cache_misses: 575
```

Release-mode baseline on this host should be refreshed after the block-ticking live-chunk promotion. The older pre-promotion release baseline was:

```text
steps: 3
radius: 1
completion_wait_ms: 30,000 timeout cap, zero observed timeouts
total_elapsed_ms: ~337
step 0 elapsed_ms: ~288, feature_total_ms: ~283, dependency_generate_ms: ~267, surface_fill_ms: ~161, surface_bedrock_ms: ~50, air_carvers_ms: ~35, feature_decoration_ms: ~12.4, polls: 1, snapshots 9, unloads 0
step 1 elapsed_ms: ~25.0, feature_total_ms: ~22.6, dependency_generate_ms: ~12.4, surface_fill_ms: ~7.3, surface_bedrock_ms: ~2.2, air_carvers_ms: ~2.0, feature_decoration_ms: ~7.5, polls: 1, snapshots 3, unloads 3
step 2 elapsed_ms: ~23.0, feature_total_ms: ~21.1, dependency_generate_ms: ~12.5, surface_fill_ms: ~7.4, surface_bedrock_ms: ~2.2, air_carvers_ms: ~2.1, feature_decoration_ms: ~7.2, polls: 1, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  inaccessible_status_chunks: 792
  border_status_chunks: 24
  ticking_status_chunks: 16
  entity_ticking_status_chunks: 9
  block_ticking_chunks: 25
  client_visible_chunks: 9
  loaded_snapshot_chunks: 9
  dirty_chunks: 9
  ready_dependency_chunks: 483
  total_seeded_dependency_chunks: 756
  total_dependency_cache_hits: 756
  total_dependency_cache_misses: 483
```

The old release baseline suggested adjacent movement was primarily worker throughput / scheduler-idle latency, not active scheduler CPU work. Re-run `pnpm native:scheduler:perf` before treating the current live block-ticking shape as performance-budgeted.

The debug worker profile now says adjacent movement pays for both a new generated strip and the wider retained live block-ticking set. In debug mode, adjacent steps spend about `~238-240ms` in dependency generation and `~209-210ms` in feature decoration. Recheck release mode before deciding whether the next optimization target is noise-column sampling, feature decoration, or live-chunk retention.

The old sleep-poll lane is still available for comparison:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --poll-mode sleep
```

On this host it keeps similar end-to-end latency to completion mode, but needs repeated empty polls while the worker is still generating.

Spin polling is a contention/stress mode:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --poll-mode spin --max-polls 50000000
```

On this host it keeps similar end-to-end latency, but burns caller-thread time on many empty polls. The normal scheduler should therefore wake from tick/frame cadence or completion notification, not tight spin-poll.

## Next Implementation Steps

1. Add a first real block-tick queue/scheduled-tick lane on top of the no-op block phase, starting with deterministic reports before mutation.
2. Profile deeper inside `ImprovedNoise::noise_scaled` / `sample_and_lerp` if surface fill remains the target; otherwise shift to feature decoration because it is now comparable to surface fill in adjacent movement.
3. Consider a feature-family split inside `underground_ores` if decoration remains material after the noise-column work.
4. Introduce an explicit protochunk/dependency-holder type so lower-status holder data is not stored as ad hoc worldgen buffers.
5. Replace per-job dependency buffer cloning with a cheaper ownership/transfer strategy if profiling changes or larger view-distance movement makes it material.
6. Decide whether to model Java's full `level 34 -> FEATURES` propagated halo before structures, or keep the MVP scoped to direct feature targets plus lower-status dependency holders.
