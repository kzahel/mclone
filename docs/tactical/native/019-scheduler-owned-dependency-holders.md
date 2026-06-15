# 019: Scheduler-Owned Dependency Holders

Status: active.

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

## Current Limits

- Propagated non-direct holders are treated as clean lower-status dependency holders. Native does not yet generate Java's full feature-status halo for `level 34` chunks.
- The clean dependency buffer is still stored as a worldgen `MutableChunkBlockBuffer`, not a long-lived protochunk type with explicit status transitions.
- Moving scheduler-owned buffers through the worker currently clones chunk buffers. This is acceptable for correctness scaffolding, but performance work should replace it with shared/owned transfer accounting once the shape stabilizes.
- Pending unloads are still immediate for non-visible chunks when no propagated ticket keeps them active. Java's delayed `pendingUnloads` queue remains unported.
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

Observed debug-mode baseline on this host with default `poll_mode=completion`:

```text
steps: 3
radius: 1
completion_wait_ms: 30,000 timeout cap, zero observed timeouts
total_elapsed_ms: ~25,033
step 0 elapsed_ms: ~14,395, completion_wait_ms: ~14,335, main_thread_scheduler_ms: ~60.739, polls: 1, snapshots 9, unloads 0
step 1 elapsed_ms: ~5,302, completion_wait_ms: ~5,282, main_thread_scheduler_ms: ~20.294, polls: 1, snapshots 3, unloads 3
step 2 elapsed_ms: ~5,329, completion_wait_ms: ~5,309, main_thread_scheduler_ms: ~19.174, polls: 1, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  client_visible_chunks: 9
  ready_dependency_chunks: 483
  total_seeded_dependency_chunks: 756
  total_dependency_cache_hits: 756
  total_dependency_cache_misses: 483
```

Observed release-mode baseline on this host with default `poll_mode=completion`:

```text
steps: 3
radius: 1
completion_wait_ms: 30,000 timeout cap, zero observed timeouts
total_elapsed_ms: ~925
step 0 elapsed_ms: ~589, completion_wait_ms: ~586, main_thread_scheduler_ms: ~2.815, polls: 1, snapshots 9, unloads 0
step 1 elapsed_ms: ~170, completion_wait_ms: ~168, main_thread_scheduler_ms: ~1.946, polls: 1, snapshots 3, unloads 3
step 2 elapsed_ms: ~165, completion_wait_ms: ~164, main_thread_scheduler_ms: ~1.774, polls: 1, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  client_visible_chunks: 9
  ready_dependency_chunks: 483
  total_seeded_dependency_chunks: 756
  total_dependency_cache_hits: 756
  total_dependency_cache_misses: 483
```

The release baseline suggests adjacent movement is primarily worker throughput / scheduler-idle latency, not active scheduler CPU work: default adjacent steps take about `165-170ms` end-to-end, while synchronous scheduler work is about `1.8-2.0ms`. In completion mode the smoke blocks while waiting for that worker result; a renderer/tick loop should keep using nonblocking `poll()` or event-loop wakeups instead of waiting on the render thread.

The old sleep-poll lane is still available for comparison:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --poll-mode sleep
```

On this host it kept similar end-to-end latency (`~930ms` total) but needed hundreds of empty polls on initial load and about a hundred empty polls per adjacent movement step.

Spin polling is a contention/stress mode:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --poll-mode spin --max-polls 50000000
```

On this host it kept similar end-to-end latency (`~910ms` total), but burned caller-thread time on millions of empty polls (`~76ms` per adjacent step inside `poll()` calls). The normal scheduler should therefore wake from tick/frame cadence or completion notification, not tight spin-poll.

## Next Implementation Steps

1. Replace per-job dependency buffer cloning with a cheaper ownership/transfer strategy or an `Arc`-backed clean dependency store if profiling confirms it is material.
2. Introduce an explicit protochunk/dependency-holder type so lower-status holder data is not stored as ad hoc worldgen buffers.
3. Add delayed pending-unload processing so save/unload work can be bounded per tick.
4. Decide whether to model Java's full `level 34 -> FEATURES` propagated halo before structures, or keep the MVP scoped to direct feature targets plus lower-status dependency holders.
