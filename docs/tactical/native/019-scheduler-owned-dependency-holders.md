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
- The smoke records timing, but no fixed perf budget is enforced yet. Use release mode for performance comparisons.

## Movement Smoke

Default command:

```bash
pnpm native:scheduler:smoke
```

Release-mode perf command:

```bash
pnpm native:scheduler:perf
```

The smoke defaults to seed `12345`, radius `1`, and `3` adjacent movement steps. It fails if:

- direct ticket count differs from the client-visible interest square
- propagated active holder count differs from the expected Chebyshev ticket halo
- client-visible chunk count differs from direct interest
- movement steps stop publishing one new strip and one unload strip
- scheduler-owned dependency seeding no longer matches worker cache hits

Observed debug-mode baseline on this host after landing the smoke:

```text
steps: 3
radius: 1
total_elapsed_ms: ~25,082
step 0 elapsed_ms: ~14,397, snapshots 9, unloads 0
step 1 elapsed_ms: ~5,331, snapshots 3, unloads 3
step 2 elapsed_ms: ~5,347, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  client_visible_chunks: 9
  ready_dependency_chunks: 483
  total_seeded_dependency_chunks: 756
  total_dependency_cache_hits: 756
  total_dependency_cache_misses: 483
```

Observed release-mode baseline on this host:

```text
steps: 3
radius: 1
total_elapsed_ms: ~934
step 0 elapsed_ms: ~607, snapshots 9, unloads 0
step 1 elapsed_ms: ~165, snapshots 3, unloads 3
step 2 elapsed_ms: ~162, snapshots 3, unloads 3
final metrics:
  active_ticket_chunks: 841
  client_visible_chunks: 9
  ready_dependency_chunks: 483
  total_seeded_dependency_chunks: 756
  total_dependency_cache_hits: 756
  total_dependency_cache_misses: 483
```

## Next Implementation Steps

1. Replace per-job dependency buffer cloning with a cheaper ownership/transfer strategy or an `Arc`-backed clean dependency store if profiling confirms it is material.
2. Introduce an explicit protochunk/dependency-holder type so lower-status holder data is not stored as ad hoc worldgen buffers.
3. Add delayed pending-unload processing so save/unload work can be bounded per tick.
4. Decide whether to model Java's full `level 34 -> FEATURES` propagated halo before structures, or keep the MVP scoped to direct feature targets plus lower-status dependency holders.
