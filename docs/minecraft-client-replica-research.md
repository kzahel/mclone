# Minecraft Client Replica And Network Internals Research

Research notes from the Minecraft Java 1.17.1 source under `reference/minecraft-1.17.1/src/`. This document is about vanilla's runtime topology and client-side world model. It is not a plan to copy vanilla's 20 TPS player movement protocol for `mclone`.

## Summary

Vanilla Minecraft is strong evidence for a client replica architecture:

- Singleplayer still starts an `IntegratedServer`, then the client joins it through a local memory connection. The renderer does not read the server world directly.
- The client owns a real `ClientLevel`: visible chunks, block states, fluid states, block entities, entities, time, weather presentation state, and a client light engine.
- The client does not own worldgen, persistence, AI authority, block tick authority, or liquid tick authority.
- Local player movement is simulated on the client, then reported as position/rotation packets and validated/corrected by the server. It is not command replay.
- Remote entities are client replica objects updated by authoritative packets and smoothed with short interpolation.
- Lighting is a local client replica system fed by server chunk/light packets and local dirty checks after block updates.
- Water/lava spread is server-authoritative. The client carries current fluid/block state for rendering and local feel, but `ClientLevel` has empty block and liquid tick lists.
- Vanilla does limited speculative block mutation for responsiveness, then reconciles through explicit block-break acknowledgements.

The main takeaway for `mclone`: use vanilla to justify a host plus client-replica split, including singleplayer. Do not use vanilla to justify "latest input over polling" or a 20 TPS position-packet movement model.

## Singleplayer Topology

Vanilla singleplayer is not a direct render of the authoritative world. `Minecraft` spins an `IntegratedServer`, waits until it is ready, opens a memory channel, then creates a client `Connection` and performs the normal login handshake:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java:1885` creates the integrated server.
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java:1909` waits for server readiness.
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java:1925` starts the memory channel.
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java:1926` creates the local client connection and sends login packets.
- `reference/minecraft-1.17.1/src/net/minecraft/network/Connection.java:305` uses Netty `LocalChannel` for local server connections.

`IntegratedServer` extends `MinecraftServer`, owns normal server state, and still ticks through `super.tickServer(...)` when unpaused:

- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java:35`
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java:79`

This matters because vanilla's singleplayer model has three logical pieces even when they share a process:

| Logical piece | Vanilla shape |
|---|---|
| Authoritative host | `IntegratedServer` / `ServerLevel` |
| Client replica | `ClientLevel`, `ClientChunkCache`, client entities, client light engine |
| Presentation | client tick/render/UI code consuming `ClientLevel` |

For `mclone`, the same logical split should hold. Browser constraints may move pieces across workers differently from Java, but singleplayer should not grant the renderer or predictor privileged access to host internals.

## Packet Flow And Client Replica Hydration

Vanilla server-to-client state is carried as authoritative facts that hydrate or mutate the client replica.

Packet handlers consistently begin by rerouting work to the owning client or server thread through `PacketUtils.ensureRunningOnSameThread(...)`. Examples include player corrections, chunk blocks, full chunks, light updates, and server movement handling:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:530`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:602`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:609`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:2107`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:761`

This is another useful shape for `mclone`: transport delivery should be separate from the owner that mutates host or client-replica state.

### Chunk Baselines

When a chunk becomes visible, `ChunkMap.playerLoadedChunk(...)` sends a `ClientboundLevelChunkPacket` and a full `ClientboundLightUpdatePacket`:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java:1018`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java:1020`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java:1021`

The chunk packet includes chunk coordinates, available section mask, heightmaps, biomes, serialized non-empty sections, and block entity update tags (`reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkPacket.java:20`, `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkPacket.java:68`). The light packet carries sky/block section masks plus 2048-byte section light arrays where present (`reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java:17`, `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java:66`).

On the client, `ClientPacketListener.handleLevelChunk(...)` replaces or creates the local `LevelChunk`, loads block entity tags, and marks sections dirty:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:609`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:616`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:618`

`ClientChunkCache.replaceWithPacketData(...)` creates or updates a client `LevelChunk`, enables client light sources for that chunk, updates section light status, and calls `ClientLevel.onChunkLoaded(...)`:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:84`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:100`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:110`

### Chunk Deltas

Server chunk deltas are batched in `ChunkHolder.broadcastChanges(...)`. It sends light changes first when light section masks changed, then sends either a single block update or a section block update, plus block entity updates when needed:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java:183`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java:193`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java:207`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java:214`

`ClientboundSectionBlocksUpdatePacket` carries a section position, short local positions, block states, and a `suppressLightUpdates` flag. The client applies each state through `ClientLevel.setBlock(...)` with flags that can include the light-suppression bit:

- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundSectionBlocksUpdatePacket.java:16`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:602`

Single block updates are applied as known authoritative state:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:651`

### Entity Baselines And Deltas

Server entity tracking lives in `ServerEntity.sendChanges(...)`. It sends relative movement packets for small deltas, teleport packets for large or discontinuous changes, rotation packets, motion packets, passenger updates, and dirty entity data:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerEntity.java:74`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerEntity.java:101`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerEntity.java:128`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerEntity.java:132`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerEntity.java:146`

On the client, remote player/entity packets create replica entities and smooth authoritative updates:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:447`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:465`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:491`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:430`

`RemotePlayer.aiStep(...)` and `LivingEntity` move toward packet targets over a small number of client ticks:

- `reference/minecraft-1.17.1/src/net/minecraft/client/player/RemotePlayer.java:41`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java:2461`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java:2693`

This is interpolation of authoritative entity state, not client-side AI prediction.

## ClientLevel Contents And Limits

`ClientLevel` is a rich client world, but it is not a second server.

It ticks time, world border, client chunks, client entities, and block entities:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:127`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:161`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:173`

It starts/stops entity ticking when chunks load/unload:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:199`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:205`

It explicitly does not schedule block or fluid ticks:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:432`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java:437`

So the client replica owns enough state to render, interpolate, and locally simulate the local player against current world facts, but does not own the systems that make canonical world changes over time.

## Local Player Movement In Vanilla

Vanilla local player movement is a hybrid:

1. `LocalPlayer.tick()` runs local player logic first through `super.tick()`.
2. The client sends sprint/sneak commands when those booleans change.
3. The client sends `ServerboundMovePlayerPacket` variants containing position, rotation, and on-ground state when movement/rotation changes enough or every 20 ticks.
4. The server validates the reported position by clamping, checking packet rate, checking "moved too quickly", moving the authoritative player through server collision, and teleporting the client back when needed.
5. Corrections use `ClientboundPlayerPositionPacket` with a teleport id; the client applies it and sends `ServerboundAcceptTeleportationPacket`.

Key source points:

- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java:183`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java:213`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java:239`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundMovePlayerPacket.java:6`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:761`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:808`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:834`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:903`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:530`

This is not the movement model `mclone` wants for FPS-style prediction. It is still useful because it proves vanilla clients carry enough local world state to run immediate local movement and accept server correction.

For `mclone`, the movement divergence should be:

- client sends sequenced command records, not absolute position as authority
- host drains commands by sequence and simulates the same fixed command quantum the client predicted
- client replay/correction uses authoritative ack snapshots
- vanilla movement/collision source remains useful for lower-body collision concepts, fluid/ladder modes, and entity movement sharing

## Lighting

Vanilla lighting is the clearest example of "server authoritative facts, client local derived system".

The client chunk cache constructs its own `LevelLightEngine`:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:31`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:38`

The server sends initial and incremental light data through `ClientboundLightUpdatePacket`. The client handles that packet by queuing sky/block section data into its local light engine and marking neighboring sections dirty:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:2107`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:2154`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:2160`

Client block updates can also queue local light checks. `Level.setBlock(...)` calls `getChunkSource().getLightEngine().checkBlock(...)` when the block's light blocking, emission, or light-occlusion shape changed and the update did not set the light-suppression flag:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/Level.java:199`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/Level.java:207`

Implications for `mclone`:

- The client replica should own visible light state or a derived light cache for rendering. It should not need to ask the host for every mesh-light sample.
- The host should remain authoritative for canonical light arrays/deltas, especially across chunk boundaries and block mutations.
- Client light work belongs with the client replica/render-world path, not the player-prediction physics core.
- Predicted local block actions that affect light should use an overlay or dirty visual cache until acked, not mutate canonical client-replica light truth without a revision path.

## Water And Other Fluids

Vanilla fluid spread is server-authoritative.

`ServerLevel` owns `liquidTicks`, runs them during the server tick, and dispatches to `FluidState.tick(...)`:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java:165`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java:357`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java:607`

`FlowingFluid.tick(...)` computes the next liquid state, writes blocks, schedules follow-up liquid ticks, and spreads:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java:408`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java:418`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java:424`

`LiquidBlock` schedules liquid ticks on placement, neighbor shape updates, and neighbor changes:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java:111`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java:116`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java:125`

The client does not run those liquid tick queues. It stores current block/fluid state from chunks and block updates, and that current state can affect local rendering and movement feel, but spread itself is not predicted.

Implications for `mclone`:

- Start with host-authoritative fluid simulation and block/fluid deltas.
- Do not bake client fluid tick authority into the base client replica.
- If packet volume becomes a real problem, add fluid-specific compression or sparse section deltas before adding full client fluid prediction.
- If fluid prediction is later needed for responsiveness, make it an overlay with revision/ack semantics. The overlay can improve visuals or local feel, but canonical fluid state still comes from the host.

## Speculative Client Actions

Vanilla is not purely passive on the client. For some block actions, it speculates locally and later reconciles.

`MultiPlayerGameMode.destroyBlock(...)` mutates the `ClientLevel` immediately by replacing a destroyed block with its fluid legacy block and invoking local destroy behavior:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:100`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:115`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:117`

Block action packets are tracked in an `unAckedActions` map. On `ClientboundBlockBreakAckPacket`, the client restores authoritative state if the server disagrees, and may restore the player's prior position if the correction collides:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:443`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:449`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:453`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java:455`

The server side remains authoritative for player actions and item use:

- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:924`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:974`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java:1001`

For `mclone`, this argues for explicit speculative overlays and acknowledgements for block interaction UX. It does not argue for giving the client general block, fluid, or worldgen authority.

## Recommended Mclone Shape

Use four logical owners:

| Owner | Responsibilities |
|---|---|
| Authoritative host | canonical chunks, entities, player bodies, AI, worldgen, block/liquid ticks, canonical lighting, persistence, gameplay consequences |
| Client replica/runtime | visible/interested chunk facts, entity replicas, local light/render facts, world time/weather presentation, interpolation buffers, speculative overlays, local prediction services |
| Prediction service | sequenced command buffer, local body replay, reconciliation, diagnostics, collision-window read view over client-replica facts |
| UI/render thread | raw input sampling, pointer lock, UI, GPU resources, draw submission, compact presentation-state consumption |

The prediction service can be a submodule of the client replica/runtime. It does not have to be a dedicated worker on day one. The important architectural rule is ownership, not thread count:

- prediction must not read authoritative host internals
- render/UI code must not own canonical chunk or collision facts
- singleplayer must hydrate the client replica from the same client-facing messages as multiplayer
- worker placement can change after measurement without changing the logical protocol

For browser singleplayer, the target is:

```text
authoritative local host
  -> client-facing protocol messages
  -> client replica/runtime
  -> compact presentation state
  -> UI/render thread
```

That can be physically implemented as host worker plus client replica worker plus UI thread, or as host worker plus a main-thread client-replica module behind a strict facade while the system is still small. The durable model should remain host plus client replica plus presentation.

## Decisions This Should Inform

- Pause `Movement3+` movement tacticals until the client runtime arc establishes integrated-server, client-world, prediction-service, and presentation ownership.
- Movement prediction needs collision-relevant client replica facts, but it does not need lighting, worldgen, AI, persistence, or scheduled fluid ticks.
- Lighting should be planned as client-replica/render-world data, server-fed but locally usable for meshing and dirty visual updates.
- Water should start as host-authoritative fluid state plus deltas. Client water prediction is a later overlay if measured packet volume or local feel demands it.
- Remote players/NPCs should initially use interpolation over authoritative snapshots. Full local AI prediction is not a vanilla precedent.
- Block interaction responsiveness should use explicit speculative overlays with action ids/acks/corrections.
- Protocol messages should distinguish source facts from derived products: chunk facts can hydrate rendering, lighting, and collision views, but meshes are not collision truth and prediction caches are not renderer-owned truth.

## Open Questions

- Does the first `mclone` client replica runtime live in the existing render-world worker, a new client-runtime worker, or a strict main-thread module with worker-ready interfaces?
- Do packed chunk snapshots feed both render/light and collision views, or should the host publish separate render and collision snapshots from the start?
- What is the first light protocol: host-published section light arrays only, client incremental light checks only for local visual dirties, or both as vanilla does?
- What water delta format is enough for early gameplay: block-level deltas, section-paletted deltas, or fluid-run records?
- Which actions need speculative overlays first: block breaking, block placement, doors/levers, bucket/fluid placement, or local player body only?
