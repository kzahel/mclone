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

## Current Limits

- Propagated non-direct holders are treated as clean lower-status dependency holders. Native does not yet generate Java's full feature-status halo for `level 34` chunks.
- The clean dependency buffer is still stored as a worldgen `MutableChunkBlockBuffer`, not a long-lived protochunk type with explicit status transitions.
- Moving scheduler-owned buffers through the worker currently clones chunk buffers. This is acceptable for correctness scaffolding, but performance work should replace it with shared/owned transfer accounting once the shape stabilizes.
- Pending unloads are still immediate for non-visible chunks when no propagated ticket keeps them active. Java's delayed `pendingUnloads` queue remains unported.
- There is no benchmark binary yet; metrics are available, but no automated movement perf budget has been recorded.

## Next Implementation Steps

1. Add a native movement benchmark/smoke that moves chunk interest across adjacent positions and records elapsed time plus `ChunkSchedulerMetrics`.
2. Replace per-job dependency buffer cloning with a cheaper ownership/transfer strategy or an `Arc`-backed clean dependency store.
3. Introduce an explicit protochunk/dependency-holder type so lower-status holder data is not stored as ad hoc worldgen buffers.
4. Add delayed pending-unload processing so save/unload work can be bounded per tick.
5. Decide whether to model Java's full `level 34 -> FEATURES` propagated halo before structures, or keep the MVP scoped to direct feature targets plus lower-status dependency holders.

