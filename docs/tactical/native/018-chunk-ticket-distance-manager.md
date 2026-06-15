# 018: Chunk Ticket Distance Manager

Status: completed; successor tactical is `019-scheduler-owned-dependency-holders.md`.

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

- Java-shaped propagated ticket levels are now covered by `019-scheduler-owned-dependency-holders.md`.
- Holder futures are still simplified `ChunkStatusSlot`s, not Java's full `CompletableFuture` graph.
- Lower-status dependency chunks are now scheduler-owned clean buffers in `019`, but not yet explicit protochunk holder objects.
- Pending unloads are now covered by `019-scheduler-owned-dependency-holders.md`: holders leave the active ticket graph first, then bounded unload processing saves/removes them later.
- Only `FEATURES` publication is wired to the scheduler. `LIGHT`, `FULL`, ticking chunks, and entity ticking are represented by ticket/full-status facts but not by real runtime systems yet.

## Successor Work

The first four follow-ups moved into `019-scheduler-owned-dependency-holders.md`: propagated ticket levels, client visibility split, scheduler-owned clean dependency buffers, and aggregate scheduler metrics.

Remaining successor work:

1. Replace cloned dependency-buffer transfer with a cheaper ownership or shared-buffer strategy if the benchmark shows material overhead.
2. Keep the browser/WASM build compiling after the API changes; browser worker plumbing can remain deferred until the scheduler boundary is stable.
