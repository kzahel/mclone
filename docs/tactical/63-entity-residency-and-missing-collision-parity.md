# Tactical 63 - Entity residency and missing-collision parity

Bring runtime entity loading, ticking, unloading, and "player outruns terrain" behavior to a vanilla-shaped architecture. The goal is not to make missing chunks feel good by inventing terrain; it is to make the host keep the right chunks available through tickets/statuses, and to fail closed when authoritative collision is unavailable.

Status: first three implementation slices landed. Generated entity chunk status now comes from generated-holder full-status transitions, those holder statuses are derived from explicit `entity` / `player_view` ticket sources, and the entity-ticking ticket uses the vanilla-shaped two-ring interior of the published player view. Later slices should replace radius helpers with real ticket levels, add entity persistence, resolve player-as-entity treatment, and add dynamic collision.

## Vanilla source anchors

| Source | Purpose |
|---|---|
| [`PersistentEntitySectionManager.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java) | entity section ownership, status changes, pending loads, unload/save, `UNLOADED_TO_CHUNK` removal |
| [`Visibility.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/Visibility.java) | `FullChunkStatus` to entity visibility mapping: hidden, tracked, ticking |
| [`EntityTickList.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java) | copy-on-write active entity ticking list |
| [`ServerLevel.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java) | server entity tick loop, entity manager callbacks, `isPositionTickingWithEntitiesLoaded(...)` |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | `INACCESSIBLE -> BORDER -> TICKING -> ENTITY_TICKING` full-status futures |
| [`DistanceManager.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java) | player tickets, player-ticket throttling, `PLAYER_TICKET_LEVEL`, view-distance-derived ticket coverage |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | player move updates, view tracking, chunk full-status notifications |
| [`Level.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/Level.java) | runtime collision asks for loaded `FULL` chunks without forcing generation |
| [`CollisionSpliterator.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/CollisionSpliterator.java) | block collision iteration skips missing collision chunks instead of synthesizing blocks |
| [`ServerGamePacketListenerImpl.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java) | server movement validation, too-fast/wrong-move rollback, floating checks |
| [`Player.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java) | players are always ticking |
| [`Entity.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java) | ordinary entities are not always ticking |
| [`PlayerRespawnLogic.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/PlayerRespawnLogic.java) | spawn/respawn ground uses loaded chunk heightmaps and block scans |

## Vanilla behavior summary

- Ordinary entities live in `PersistentEntitySectionManager` sections. Full chunk status controls whether their sections are hidden, tracked, or ticking.
- `INACCESSIBLE` maps to hidden; `BORDER` and `TICKING` map to tracked; `ENTITY_TICKING` maps to ticking.
- When a chunk becomes hidden, vanilla queues entity unload. The unload waits if the entity chunk load is still pending, stores saveable entities, then removes them with `UNLOADED_TO_CHUNK`.
- Players are special. `Player.isAlwaysTicking()` returns `true`; players are not unloaded like passive mobs just because surrounding chunks unload.
- Player movement updates tickets through `ChunkMap.move(...)` and `DistanceManager.addPlayer/removePlayer(...)`. Player tickets keep a region around the player generated/loaded; entity ticking is derived from ticket/full-status levels, not from client publication packets.
- Vanilla runtime collision does not invent ground height for missing chunks. Server `Level.getChunkForCollisions(...)` requests a loaded `FULL` chunk with `load=false`; collision iteration skips null chunks. The intended safety comes from chunk tickets keeping needed chunks available, server movement validation, and teleport/floating checks.
- Spawn and respawn placement use loaded chunks and heightmaps. Missing chunks are not replaced by a seed-derived client guess.

## Current mclone behavior

- `EntityRuntime` and `PersistentEntitySectionManager` already port the vanilla-shaped add/move/remove/load/unload/status semantics.
- `GeneratedWorldHost` now installs an explicit `entity` ticket and stores the current `FullChunkStatus` on generated chunk holders. Entity manager updates are emitted from holder full-status transitions via `PersistentEntitySectionManager.updateChunkStatus(...)`.
- `ensureOriginalMobsForChunk(...)` no longer promotes chunks to `ENTITY_TICKING`; publication only creates and sends snapshots for entities that the entity manager already considers tracked.
- `player_view` chunks are at least `BORDER`, while the `entity` source marks only the interior as `ENTITY_TICKING`. At radius 0 this means 25 published/tracked chunks, 1 ticking chunk, and 24 tracked-only chunks.
- `discardUnloadedChunkState(...)` re-applies the current ticket-derived holder full status instead of blindly hiding the chunk, so chunks outside the published/entity ticket become hidden while tracked-only chunks remain accessible.
- Host player state is session-owned movement state, not a `Player` entity in `EntityRuntime`. That is acceptable short-term but not full parity.
- Host player collision already fails closed: `createGeneratedLevelCollisionWorld(...)` returns `missing_authority_chunk` when the movement AABB touches a chunk missing from authority, and the movement loop leaves commands queued instead of moving through absent terrain.
- Mob pathing also fails closed: missing path blocks behave as blocking material, missing collision returns false, and stable standing scans require loaded block states.
- Runtime data-model docs already forbid client-side seed generation as a collision fallback.

## Gaps to close

1. **Entity status ownership**
   - Status: landed for generated-host ownership.
   - Landed: entity chunk status is no longer promoted by chunk publication; generated holders own current full status, and entity visibility/ticking updates are emitted from full-status transitions.
   - Remaining gap: those holder statuses are still computed from host-radius ticket decisions rather than vanilla ticket-level propagation.
   - Parity target: entity status is derived from full-status state, then handed to `PersistentEntitySectionManager.updateChunkStatus(...)`.

2. **Tracked ring vs entity-ticking ring**
   - Status: landed for generated host ticket/status behavior.
   - Remaining gap: the split is still computed by radius helpers, not by vanilla-like ticket-level propagation.
   - Parity target: distinguish tracked chunks from entity-ticking chunks. Published chunks can be tracked without ticking; only `ENTITY_TICKING` chunks tick ordinary entities.

3. **Ticket levels instead of radius helper decisions**
   - Gap: mclone now has named ticket sources, but not vanilla-like ticket levels or full-status derivation for entities/light/forced tickets.
   - Parity target: `ChunkHolder`-like records own full status, and entity manager updates are a consequence of full-status promotion/demotion.

4. **Entity persistence backend**
   - Gap: generated host uses the entity manager with default memory storage; chunk storage race-safety work has not been applied to entity records.
   - Parity target: file/IndexedDB entity storage follows the same pending-load/unload, pending-write read-through, session epoch, and save queue rules as chunk data.

5. **Player entity parity**
   - Gap: local/remote players are movement session records rather than `Player` entities in the section manager.
   - Parity target: either bridge player state into an always-ticking entity record or make the divergence explicit and prove equivalent ticket/tracking behavior.

6. **Fast movement and generation backpressure**
   - Gap: missing collision is explicit and safe, but the host does not yet have a vanilla-like movement/backpressure trace proving how command queues, chunk generation, and view updates settle when the player outruns terrain.
   - Parity target: movement cannot commit through missing authority data; after generation settles, queued commands replay against authoritative chunks without losing order.

7. **Ground/spawn placement**
   - Gap: live player spawn still has provisional placement.
   - Parity target: spawn/respawn placement scans loaded chunks/heightmaps like vanilla and returns "missing/not ready" instead of seed-guessing.

8. **Dynamic entity collision**
   - Gap: player movement collision is block-only; dynamic entity colliders are not part of the host movement collision world.
   - Parity target: host collision combines block collision and accessible entity sections, gated by entity visibility/status.

## Implementation sequence

1. **Explicit entity ticket/status source**
   - Status: landed.
   - Added an `entity` ticket for generated-host entity status.
   - Synced `PersistentEntitySectionManager` chunk status from `entity` and `player_view` tickets.
   - Stopped promoting entity chunks from `ensureOriginalMobsForChunk(...)`.
   - Added debug records and a walk sanity test that verifies entity statuses after the host settles.

2. **Tracked vs ticking split**
   - Status: landed.
   - `player_view` chunks remain at least `BORDER`.
   - The `entity` ticking ticket is computed as `view radius - 2`, clamped at 0, matching vanilla's `PlayerTicketTracker.haveTicketFor(level <= viewDistance - 2)` shape.
   - Added tests proving entities in tracked-only chunks remain visible but do not tick until their chunk is promoted to `ENTITY_TICKING`.

3. **Full-status owner**
   - Status: landed.
   - Generated chunk holders now store current `FullChunkStatus`.
   - Entity manager updates run from holder full-status changes instead of direct host radius checks.
   - Added status counters and a transition test for `BORDER -> ENTITY_TICKING -> BORDER -> INACCESSIBLE`.

4. **Entity storage integration**
   - Add host-provided entity storage adapters for memory/file/IndexedDB.
   - Apply pending-write read-through and session epoch protections.
   - Test unload while entity save is pending, immediate return, and stale write rejection.

5. **Fast movement settling harness**
   - Create a controlled host test that blocks generation, sends movement across a chunk boundary, verifies missing collision leaves input queued, releases generation, then verifies final chunk/entity states and processed input order.
   - Track whether the result is a mclone safety divergence or vanilla-equivalent behavior.

6. **Player/entity bridge**
   - Decide whether to place players in the section manager as always-ticking entities or keep a documented session-state bridge.
   - Prove chunk tickets, tracking, and dynamic collision see players consistently.

7. **Ground and spawn parity**
   - Replace provisional spawn height with loaded-chunk/heightmap scans.
   - Missing chunk data must return a not-ready result, not a generated fallback.

8. **Dynamic collision**
   - Extend collision worlds with entity shapes from accessible sections.
   - Keep missing entity/collision windows explicit in prediction diagnostics.

## Acceptance bar

- A player walking faster than generation can finish must not fall because an unloaded chunk was treated as air.
- No authoritative ground level is guessed from seed or presentation data.
- Ordinary entities outside entity-ticking chunks do not tick; entities outside tracked/accessible chunks stop being visible and unload/save through the entity manager.
- Players remain active and ticket-owning even when ordinary entities would unload.
- Chunk/entity save and reload ordering is deterministic under pending saves, stale preloads, and rapid return to a chunk.

## Landed first slice

- `GeneratedWorldHost` installs an `entity` ticket alongside `player_view` and `generation_dependency`.
- Entity chunk status is tracked in host debug records and synchronized into `EntityRuntime` from ticket state.
- `entity_chunk_statuses_current`, `entity_chunk_statuses_ticking_current`, and `entity_chunk_statuses_tracked_current` counters expose the current entity-status window.
- Existing chunk-crossing generation counts remain unchanged: the new entity ticket overlaps the hidden generation dependency ticket, so resident holder count stays at 625.

## Landed second slice

- `GeneratedWorldHost.getGeneratedWorldEntityChunkRadius(...)` now keeps the entity-ticking ticket two rings inside the published `player_view` radius, clamped at 0.
- `GeneratedRenderLevel` records the same smaller `entity` source in its authority ticket set.
- The radius-0 flat-grass sanity test now verifies 25 entity-status chunks after a one-chunk walk: 1 `ENTITY_TICKING` center chunk and 24 `BORDER` tracked-only chunks.
- A moving-cow regression publishes a cow in the east border ring, verifies no entity update while the chunk is tracked-only, then moves the view so the cow's chunk becomes `ENTITY_TICKING` and verifies the first tick/update occurs.
- Existing chunk-crossing generation counts remain unchanged: the tracked/ticking split changes entity status only, not the 5/7/9/11 terrain/features/full/publish work.

## Landed third slice

- `GeneratedChunkHolder` now owns a `fullStatus` field initialized to `INACCESSIBLE`.
- Chunk-view ticket changes update holder full statuses first; entity chunk status is then synchronized into `EntityRuntime` from those holder transitions.
- New debug records expose holder full statuses, and new counters expose current holder full-status distribution plus `chunk_full_status_updates.<status>`.
- The radius-0 flat-grass walk now records 625 holder full statuses: 600 `INACCESSIBLE`, 24 `BORDER`, and 1 `ENTITY_TICKING`.
- A transition regression follows one chunk through `BORDER -> ENTITY_TICKING -> BORDER -> INACCESSIBLE` and verifies the entity status view follows the holder state.
