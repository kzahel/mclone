# 018: Chunk Ticket Distance Manager

Status: active.

## Purpose

Move native chunk loading from direct interest-set ownership toward Java's ticket-driven residency model. This is the foundation for generation scheduling, chunk ticking, entity ticking, forced chunks, stale request cleanup, and future scheduler-owned dependency protochunks.

The immediate goal is not to port the entire Java async future graph. It is to make chunk holders resident because tickets require them, expose Java-shaped ticket levels/full chunk status, and keep generation/load/unload decisions behind a distance-manager boundary.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/Ticket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/TicketType.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

Key Java facts for this slice:

- `Ticket` ordering is by ticket level, ticket type, then key; `createdTick` is used only for timeouts.
- `TicketType.PORTAL`, `POST_TELEPORT`, and `UNKNOWN` time out after `300`, `5`, and `1` ticks respectively.
- `DistanceManager.PLAYER_TICKET_LEVEL = 33 + ChunkStatus.getDistance(FULL) - 2`, which is `31` for vanilla 1.17.1.
- `ChunkMap.MAX_CHUNK_DISTANCE = 33 + ChunkStatus.maxDistance()`, which is `44` because `STATUS_BY_RANGE` has `11` entries.
- `ChunkHolder.getStatus(level)` treats levels below `33` as `FULL`; `level 33` is full-border, `34` maps to `FEATURES`, and larger levels degrade toward lower generation statuses.
- `ChunkHolder.getFullChunkStatus(level)` maps `31 -> ENTITY_TICKING`, `32 -> TICKING`, `33 -> BORDER`, and `34+ -> INACCESSIBLE`.
- `ChunkMap.updateChunkScheduling(...)` creates holders when the new level is within `MAX_CHUNK_DISTANCE`, marks holders for drop when level exceeds it, and lets unload/save happen through the pending-unload path.

## Landed In First Slice

- Native `mclone-server` now has Java-shaped `ChunkTicketType`, `ChunkTicketKey`, `ChunkTicket`, and `ChunkDistanceManager`.
- `ChunkScheduler::apply_interest(...)` translates the protocol's current `ChunkInterest` into `PLAYER` tickets instead of directly owning the desired holder set.
- Chunk holders now store and expose numeric `ticket_level` plus `FullChunkStatus`.
- The scheduler reconciles holder load/unload from active tickets and still feeds the existing `FEATURES` worker pipeline.
- Forced tickets can keep a chunk resident after player interest moves away.
- Stale `UNKNOWN` tickets expire through `ChunkScheduler::tick()`.
- `IntegratedServer::tick()` now exposes the same ticket-purge/reconcile path at the server layer.

## Current Limits

- The native distance manager does not yet implement Java's full `ChunkTracker` distance propagation. Current player interest creates direct player tickets for the protocol view chunks, preserving existing runtime behavior.
- Holder futures are still simplified `ChunkStatusSlot`s, not Java's full `CompletableFuture` graph.
- Lower-status dependency chunks are still worker-local clean buffers. They are not yet scheduler-owned protochunk holders with ticket levels.
- Pending unloads are immediate after ticket removal, except dirty chunks are saved before removal. Java's delayed `pendingUnloads` queue is still a future refinement.
- Only `FEATURES` publication is wired to the scheduler. `LIGHT`, `FULL`, ticking chunks, and entity ticking are represented by ticket/full-status facts but not by real runtime systems yet.

## Next Implementation Steps

1. Add distance propagation over ticket sources so non-player tickets can retain lower-status halos the way Java's `ChunkTicketTracker` does.
2. Split holder target status from publication status so propagated dependency holders can stop at `Terrain`, `Surface`, or `Features` without necessarily publishing snapshots to clients.
3. Promote worker-local clean dependency chunks into scheduler-owned protochunk/dependency holders, with reuse keyed by status and seed.
4. Add scheduler performance counters: ticketed holder count, propagated holder count, generated dependency count, reused dependency count, unload count, and per-job elapsed time.
5. Add a small native benchmark/smoke for interest movement over a radius view so chunk residency and worldgen reuse regressions are visible.
6. Keep the browser/WASM build compiling after the API changes; browser worker plumbing can remain deferred until the scheduler boundary is stable.

