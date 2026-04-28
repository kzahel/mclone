# Tactical 63 - Entity residency and missing-collision parity

Bring runtime entity loading, ticking, unloading, and "player outruns terrain" behavior to a vanilla-shaped architecture. The goal is not to make missing chunks feel good by inventing terrain; it is to make the host keep the right chunks available through tickets/statuses, and to fail closed when authoritative collision is unavailable.

Status: first implementation slice landed. Generated entity chunk status now comes from an explicit `entity` ticket/status source instead of being promoted by chunk publication. Later slices should refine the exact vanilla ticket radii, entity persistence, player-as-entity treatment, and dynamic collision.

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
- `GeneratedWorldHost` now installs an explicit `entity` ticket and syncs entity chunk status through `PersistentEntitySectionManager.updateChunkStatus(...)`.
- `ensureOriginalMobsForChunk(...)` no longer promotes chunks to `ENTITY_TICKING`; publication only creates and sends snapshots for entities that the entity manager already considers tracked.
- `discardUnloadedChunkState(...)` re-applies the current ticket-derived entity status instead of blindly hiding the chunk. Today that still hides chunks outside the published/entity ticket, but it keeps the path open for tracked-only or entity-ticking-only windows.
- Host player state is session-owned movement state, not a `Player` entity in `EntityRuntime`. That is acceptable short-term but not full parity.
- Host player collision already fails closed: `createGeneratedLevelCollisionWorld(...)` returns `missing_authority_chunk` when the movement AABB touches a chunk missing from authority, and the movement loop leaves commands queued instead of moving through absent terrain.
- Mob pathing also fails closed: missing path blocks behave as blocking material, missing collision returns false, and stable standing scans require loaded block states.
- Runtime data-model docs already forbid client-side seed generation as a collision fallback.

## Gaps to close

1. **Entity status ownership**
   - Landed: entity chunk status is no longer promoted by chunk publication; it is synced from explicit `entity` / `player_view` tickets.
   - Remaining gap: those tickets are still host-radius decisions rather than holder-owned vanilla full-status transitions.
   - Parity target: entity status is derived from full-status state, then handed to `PersistentEntitySectionManager.updateChunkStatus(...)`.

2. **Tracked ring vs entity-ticking ring**
   - Gap: every published generated chunk is effectively entity-ticking.
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
   - Kept today's effective radius for behavior stability: entity ticking matches the published 5x5 radius-0 grid for now.
   - Added debug records and a walk sanity test that verifies entity statuses after the host settles.

2. **Tracked vs ticking split**
   - Keep `player_view` chunks at least `BORDER`.
   - Shrink or separately compute the `entity` ticking ticket to match vanilla's "view distance minus two rings" behavior once the mclone radius model is mapped cleanly to vanilla `ChunkMap.viewDistance`.
   - Add tests proving entities in tracked-only chunks remain visible but do not tick.

3. **Full-status owner**
   - Move entity status sync behind a holder/full-status transition path instead of direct host radius checks.
   - Add status-transition counters and tests for `BORDER -> ENTITY_TICKING -> BORDER -> INACCESSIBLE`.

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
- The radius-0 flat-grass sanity test verifies that a one-chunk walk settles to exactly 25 `ENTITY_TICKING` chunks, drops the old west edge, and adds the new east edge.
- Existing chunk-crossing generation counts remain unchanged: the new entity ticket overlaps the hidden generation dependency ticket, so resident holder count stays at 625.
