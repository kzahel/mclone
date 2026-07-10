# Vanilla Networking

Topic: vanilla-networking

Status: reference notes for Minecraft Java 1.17.1 vanilla networking, with
receipts from the decompiled reference tree under
`reference/minecraft-1.17.1/src/` (Mojang mappings). This document owns the
external facts about vanilla's network stack: connection pipeline, protocol
states, login/join sequence, tick loop, chunk/entity sync cadence, movement
validation, and threading/ordering assumptions. Line numbers refer to the
decompiled sources; decompiled local variable names appear as `var*`.

Sibling docs:

- [`../../minecraft-client-replica-research.md`](../../minecraft-client-replica-research.md)
  owns the client-replica topology argument (IntegratedServer, ClientLevel
  ownership, what the client does and does not simulate).
- [`../multiplayer-networking.md`](../multiplayer-networking.md) owns mclone's
  current networking state and plan.
- [`../client-prediction.md`](../client-prediction.md) owns mclone's
  movement-authority direction.

## Protocol version and states

- Protocol version `756` (`net/minecraft/SharedConstants.java:21,156-157`).
- `ConnectionProtocol` enum: `HANDSHAKING(-1)`, `PLAY(0)`, `STATUS(1)`,
  `LOGIN(2)` (`network/ConnectionProtocol.java:184,192,355,371`).
- **Packet IDs are registration order**: `PacketSet.addPacket` assigns the next
  sequential index per (protocol, flow) list (`ConnectionProtocol.java:461-467`).
  1.17.1 PLAY registers 91 clientbound and 48 serverbound packets
  (`ConnectionProtocol.java:198-300,305-352`). LOGIN has 5 clientbound
  (Disconnect, Hello, GameProfile, LoginCompression, CustomQuery) and 3
  serverbound (Hello, Key, CustomQuery) (`ConnectionProtocol.java:377-388`).

## Connection and pipeline

`Connection extends SimpleChannelInboundHandler<Packet<?>>`
(`network/Connection.java:50`). Client pipeline order
(`Connection.java:289-295`), with `TCP_NODELAY=true` (`Connection.java:285`):

```
timeout   ReadTimeoutHandler(30)
splitter  Varint21FrameDecoder
decoder   PacketDecoder(CLIENTBOUND)
prepender Varint21LengthFieldPrepender
encoder   PacketEncoder(SERVERBOUND)
packet_handler  <Connection>
```

The server pipeline is identical plus a `legacy_query` sniffer before the
splitter (`server/network/ServerConnectionListener.java:94-95`). Singleplayer
memory connections use a Netty `LocalChannel` with **only** `packet_handler` —
no framing, compression, or encryption (`Connection.java:305-313`).

- **Framing**: VarInt length prefix capped at 3 bytes / 21 bits
  (`Varint21FrameDecoder.java:13-41`, `Varint21LengthFieldPrepender.java:10-16`).
- **Packet size cap**: encoder rejects bodies over `8388608` bytes (8 MiB)
  (`PacketEncoder.java:43-44`).
- **Compression** (enabled during login, §Login): wire format is VarInt
  uncompressed-size, `0` = stored uncompressed; zlib deflate otherwise.
  Server-side decode validates size below threshold / above 8 MiB
  (`CompressionDecoder.java:12-39`, `CompressionEncoder.java:18-37`).
  `setupCompression` inserts `decompress`/`compress` inside the framing pair
  (`Connection.java:346-368`).
- **Encryption** (online mode): AES-128 `AES/CFB8/NoPadding` stream cipher
  wrapping the entire framed stream — `decrypt` before `splitter`, `encrypt`
  before `prepender` (`Connection.java:315-319`, `util/Crypt.java:19,107`).
- **Send path**: every packet is `channel.writeAndFlush(...)` at send time
  (`Connection.java:185-198`); packets sent before the channel connects queue
  in a `ConcurrentLinkedQueue` drained on the next send or tick
  (`Connection.java:66,156-167`). Cross-state packets pause `autoRead` until
  the protocol attribute flips (`Connection.java:96-100,173-190`).
- **Receive path / game-thread dispatch**: `channelRead0` runs on the Netty IO
  thread and calls `packet.handle(listener)` (`Connection.java:133-149`).
  Nearly every handler's first line is
  `PacketUtils.ensureRunningOnSameThread(packet, listener, eventLoop)`
  (`network/protocol/PacketUtils.java:17-28`): if not on the game thread it
  re-posts the handler to the owning `BlockableEventLoop` (re-checking
  `isConnected()` before handling) and aborts the Netty-thread invocation by
  throwing the stackless singleton `RunningOnDifferentThreadException`, which
  `channelRead0` swallows. Transport delivery is thereby decoupled from state
  mutation; the game thread applies packets in receive order.
- **Connection.tick()** (`Connection.java:215-236`): flush queue, tick
  login/game listeners, detect disconnect, `channel.flush()`, and once per 20
  ticks lerp `averageSent/ReceivedPackets` with factor `0.75`.
- **Disconnect**: `channelInactive` → `disconnect.endOfStream`; Netty
  `TimeoutException` → `disconnect.timeout`; first other exception sends a
  disconnect packet then makes the channel read-only
  (`Connection.java:102-131`). `handleDisconnection` is idempotent and invokes
  `packetListener.onDisconnect(reason)` (`Connection.java:370-383`).
- **Rate kick**: `RateKickingConnection` disconnects when average received
  packets/sec exceeds the `rate-limit` property (default `0` = off)
  (`network/RateKickingConnection.java:10-30`,
  `server/dedicated/DedicatedServerProperties.java:80`).

## Login and join sequence

Handshake: `ClientIntentionPacket` carries protocol version, host, port, and
intent (`network/protocol/handshake/ClientIntentionPacket.java:9-35`); version
mismatch disconnects with `outdated_client`/`incompatible`
(`server/network/ServerHandshakePacketListenerImpl.java:29-38`).

Login (`server/network/ServerLoginPacketListenerImpl.java`):

1. `ServerboundHelloPacket` (profile name). Online mode → `ClientboundHello`
   with RSA-1024 public key + 4-byte nonce (`:145-154`); offline mode skips
   straight to accept.
2. Client generates an AES key, hits Mojang `joinServer` off-thread, then
   sends `ServerboundKeyPacket` with a send-listener that enables encryption
   only after the packet flushes
   (`client/multiplayer/ClientHandshakePacketListenerImpl.java:56-87`).
3. Server verifies the nonce, enables encryption, authenticates on a
   `User Authenticator #N` thread (`:157-224`).
4. **Compression enable**: if `getCompressionThreshold() >= 0` and not a
   memory connection, send `ClientboundLoginCompressionPacket(threshold)` with
   a listener installing the codec only after the packet flushes (`:104-110`).
   Threshold default **256** (`DedicatedServerProperties.java:83`). The client
   installs without validation
   (`ClientHandshakePacketListenerImpl.java:137-141`).
5. `ClientboundGameProfilePacket` → client switches to PLAY (`:112`).
6. Login timeout: 600 ticks → `slow_login` (`:43,74-76`).

Join packet order — `PlayerList.placeNewPlayer`
(`server/players/PlayerList.java:126-256`):

1. `ClientboundLoginPacket`: entity id, game modes, **obfuscated seed**
   (`BiomeManager.obfuscateSeed`), registries, dimension, max players,
   **view distance**, flags (`:158-176`).
2. Brand custom payload; difficulty; abilities; carried slot; full recipes;
   tags; permission level; recipe book; scoreboard (`:177-190`).
3. Join chat broadcast; **position teleport with teleport id** (`:200`).
4. `ClientboundPlayerInfoPacket ADD_PLAYER` both directions (`:203-207`).
5. `level.addNewPlayer` → ChunkMap tracking begins:
   `ClientboundSetChunkCacheCenterPacket` then chunk streaming over the view
   square (`server/level/ChunkMap.java:804-834`).
6. World border, time, spawn position, weather events; resource pack; mob
   effects; vehicle remount; inventory menu init (`:210-255`).

Chunks are never sent inline in the join sequence — they stream through the
chunk tracking path below.

## Keepalive and timeouts

| Mechanism | Value | Receipt |
|---|---|---|
| Server keepalive interval | 15000 ms; unanswered by next interval → `disconnect.timeout` | `ServerGamePacketListenerImpl.java:157,245-258` |
| Keepalive ack | must echo the exact challenge id; latency lerped `(old*3+new)/4` | `ServerGamePacketListenerImpl.java:1416-1424` |
| Read timeout (both ends) | `ReadTimeoutHandler(30)` seconds | `ServerConnectionListener.java:94`, `Connection.java:290` |
| Login timeout | 600 ticks | `ServerLoginPacketListenerImpl.java:43,74-76` |
| Idle kick | `player-idle-timeout` minutes (0 = off) | `ServerGamePacketListenerImpl.java:267-271` |
| Move-packet flood | >5 packets/tick logged + anti-cheat allowance clamped | `ServerGamePacketListenerImpl.java:808-813` |

The client echoes keepalive **on the Netty thread** (no game-thread hop) so
latency measurement is independent of client main-thread stalls
(`client/multiplayer/ClientPacketListener.java:1498-1501`).

## Server tick loop

`server/MinecraftServer.java`: `MS_PER_TICK = 50` (`:166`), dedicated
`Server thread` (`:256`). Per `runServer` iteration (`:665-699`):

- If more than `2000` ms behind, **skip** the backlog:
  `nextTickTime += missedTicks * 50` with the "Can't keep up!" warning at most
  every 15 s (`:675-681`). Below 2 s behind, it catches up by running ticks
  back-to-back with no sleep.
- `tickServer` → `tickChildren` order (`:860-906`): every 20 ticks broadcast
  `ClientboundSetTimePacket` per dimension; `level.tick()` (which runs
  per-chunk `broadcastChanges` and entity-tracker `sendChanges` inside
  `ServerChunkCache.tick`, `server/level/ServerChunkCache.java:370-376`);
  then connection tick; then player list tick.
- Inter-tick wait `waitUntilNextTick` drains queued packet-handler tasks
  (`:739-742`); tasks up to 3 ticks stale run even without spare time
  (`:748-750`).
- Autosave every 6000 ticks (`:833-840`); average tick time EMA (`:854`).
- Because sends are `writeAndFlush`, gameplay packets flush the moment
  handlers send them mid-tick; `Connection.tick()`'s flush is a mop-up.

## Chunk streaming

- `ChunkMap.playerLoadedChunk` lazily builds one shared
  `ClientboundLevelChunkPacket` + full `ClientboundLightUpdatePacket` pair per
  chunk and sends **light before chunk data**
  (`server/level/ChunkMap.java:1018-1056`,
  `server/level/ServerPlayer.java:1457-1460`), then pairs entities in that
  chunk.
- Triggers: join/leave view-square sweep, player movement
  (checkerboard/Chebyshev distance vs view distance), view-distance change,
  and a chunk first reaching ticking status
  (`ChunkMap.java:607-611,682-722,804-916`).
- Out-of-range → `ClientboundForgetLevelChunkPacket`
  (`ServerPlayer.java:1462-1466`).
- View distance: property default 10; internal radius clamped
  `clamp(viewDistance + 1, 3, 33)` (`ChunkMap.java:683`,
  `DedicatedServerProperties.java:81`). The login packet carries the raw
  property value (`PlayerList.java:170`). Cache center updates via
  `ClientboundSetChunkCacheCenterPacket` on section change
  (`ChunkMap.java:831-836`).
- Chunk packet: coords, section BitSet, heightmaps NBT, biome int array,
  serialized non-empty sections, block-entity tags; read side rejects section
  buffers over 2 MiB
  (`network/protocol/game/ClientboundLevelChunkPacket.java:21,36-65,100-114`).
  1.17.1 chunk packets are always full replacements.
- **Block updates are batched per tick per section** — `ChunkHolder`
  accumulates changed positions; `broadcastChanges` sends one
  `ClientboundBlockUpdatePacket` for a single change or one
  `ClientboundSectionBlocksUpdatePacket` per section for many, with light
  resend when 64+ blocks changed (`server/level/ChunkHolder.java:153-225`).
  Section entries encode as `VarLong(stateId << 12 | posShort)`
  (`ClientboundSectionBlocksUpdatePacket.java:15,51-58`).

## Entity sync cadence (`server/level/ServerEntity.java`)

Constructed per tracked entity from `EntityType` parameters: tracking range =
`clientTrackingRange() * 16` blocks, update interval, delta-tracking flag
(`server/level/ChunkMap.java:930-936`). Defaults: range 5 chunks, interval 3
(`world/entity/EntityType.java:852-853`); **players: range 32, interval 2**
(`EntityType.java:496-498`); `trackDeltas()` is false for players (no velocity
packets for ordinary player movement, `EntityType.java:817-828`).

Per `sendChanges` (`ServerEntity.java:74-187`), gated on
`tickCount % updateInterval == 0 || hasImpulse || entityData.isDirty()`:

- Position quantization: 1/4096 block fixed point
  (`ClientboundMoveEntityPacket.java:12,23-28`); rotation in 256ths of a turn.
- Movement dirty threshold `lengthSqr() >= 7.6293945E-6` (`:120`); **position
  packet at least every 60 ticks** even when idle (`:122`).
- **Teleport vs delta**: any axis delta outside the signed-short range
  (±8 blocks), or 400 ticks since last teleport, or riding/on-ground change →
  `ClientboundTeleportEntityPacket`; else short-encoded `Pos`/`Rot`/`PosRot`
  (`:124-143`).
- Velocity: only when tracked/impulse/fall-flying, dirty threshold `1.0E-7`
  squared (`:146-154`); `hurtMarked` forces a broadcast including the owner.
- Head yaw at ≥1/256-turn change (`:173-177`). Dirty `SynchedEntityData`
  metadata ships the same tick regardless of interval
  (`network/syncher/SynchedEntityData.java:150-200`,
  `ServerEntity.java:262-276`).
- Spawn pairing sends add packet, full metadata, attributes, motion,
  equipment, effects, passengers, leash (`:199-260`).
- Broadcast range check is square (Chebyshev) on x/z, capped by view distance
  (`ChunkMap.java:1146-1159`), scaled by
  `entity-broadcast-range-percentage` (default 100).

## Player movement send (client)

`client/player/LocalPlayer.java`:

- `tick()` runs `super.tick()` (full local physics) **before** `sendPosition()`
  (`:183-194`). The client fully simulates its own player: `isEffectiveAi()`
  is unconditionally true client-side (`:487-490`), opening the normal
  `LivingEntity.travel`/`aiStep` physics paths
  (`world/entity/LivingEntity.java:1999-2001,2502-2542`).
- Send thresholds (`:238-253`): position when squared delta `> 9.0E-4`
  (0.03 blocks) **or** every 20 ticks (`POSITION_REMINDER_INTERVAL = 20`,
  `:81`); rotation on any change. Variants `Pos` / `PosRot` / `Rot` /
  `StatusOnly(onGround)` (`ServerboundMovePlayerPacket.java:63-145`).
- Sprint/sneak transitions send `ServerboundPlayerCommandPacket` (`:214-230`).
- Passengers send `Rot` + `ServerboundPlayerInputPacket(strafe, forward,
  jumping, sneaking)` every tick, plus `ServerboundMoveVehiclePacket` when
  controlling the vehicle (`:186-192`).

## Server movement validation (`ServerGamePacketListenerImpl.handleMovePlayer`)

`server/network/ServerGamePacketListenerImpl.java:760-883`. The server tracks
`firstGood*` (start of tick), `lastGood*` (last accepted), and packet counts;
`resetPosition()` re-bases them each server tick (`:202,274-281`). Each
network tick the server player is snapped back to the last client-reported
position before its own `doTick` side effects (`:206-207`) — vanilla is
client-authoritative for position, with these checks:

1. NaN/infinity → disconnect `invalid_player_movement` (`:763-764,324-326`).
2. **Pending-teleport gate**: while `awaitingPositionFromClient != null`,
   movement packets are ignored; the teleport is re-sent after 20 ticks
   unacked (`:772-782`).
3. Coordinate clamps: |x|,|z| ≤ 3.0E7, |y| ≤ 2.0E7 (`:328-334,785-789`).
4. Packet-burst factor: `receivedMovePacketCount - knownMovePacketCount`;
   >5 logs "sending move packets too frequently" and clamps to 1 (`:808-813`).
5. **"moved too quickly!"**: `distSq(firstGood → packet) - velocityLengthSq >
   (isFallFlying ? 300 : 100) * burstCount`, skipped for the singleplayer
   owner and during dimension change (`:815-823`).
6. Server-side jump replay when ascending off server-ground (`:829-832`),
   then **collision replay**: `player.move(MoverType.PLAYER, delta-from-
   lastGood)` (`:834`).
7. **"moved wrongly!"**: residual distSq between packet position and
   post-move server position `> 0.0625` (0.25 blocks), except dimension
   change/sleeping/creative/spectator (`:835-851`). Note the decompiled
   always-true quirk `if (var21 > -0.5 || var21 < 0.5) var21 = 0.0;` — the Y
   residual is always zeroed (`:837-839`); this is faithful to vanilla.
8. Accept: `absMoveTo(packet pos)`; roll back with a teleport to the
   pre-packet position if (moved-wrongly and the old AABB was collision-free)
   or the accepted position intersects newly-collided geometry
   (`:853-877,885-889`).
9. **Floating kick**: `clientIsFloating` when vertical residual ≥ -0.03125
   with no flight rights and no blocks in the AABB expanded 0.55 down;
   80 consecutive ticks → disconnect `multiplayer.disconnect.flying`
   (`:210-219,857-863,401-403`).

Vehicle path `handleMoveVehicle` (`:336-399`) mirrors this with 100 dist-sq
threshold; corrections resend `ClientboundMoveVehiclePacket` instead of a
player teleport.

## Teleport/ack protocol

- Server `teleport(...)` (`:895-921`): per-axis relative flags
  (X/Y/Z/Y_ROT/X_ROT bits), monotonically increasing `awaitingTeleport` id
  (wraps at `Integer.MAX_VALUE`), immediate server-side `absMoveTo`, then
  `ClientboundPlayerPositionPacket(deltas-or-absolutes, flags, id,
  dismountVehicle)` (`ClientboundPlayerPositionPacket.java:49-51,90-131`).
- `handleAcceptTeleportPacket` only accepts the current id; applies the
  awaited position to `lastGood*` and clears the gate (`:405-426`).
- Client handling (`ClientPacketListener.java:530-599`): resolve
  relative/absolute per axis (relative axes keep that velocity component,
  absolute axes zero it), rewrite old-position fields to kill render
  interpolation, `absMoveTo`, reply `ServerboundAcceptTeleportationPacket(id)`
  **plus** an immediate `PosRot` move packet with onGround=false. A hard
  snap — vanilla does no input replay on correction.
- Resync triggers funneling through `teleport(...)`: failed validation,
  sleep-move, teleport re-send, `/tp`, respawn, dimension change/spectate,
  bed exit, mount/dismount (`TeleportCommand.java:289`,
  `PlayerList.java:484`, `ServerPlayer.java:868-909,1177-1186,1390-1415`).

## Client-side interpolation of remote entities

- `LivingEntity.lerpTo` stores a target and `lerpSteps = N`; each tick moves
  `current + (target - current) / lerpSteps` then decrements — exact
  convergence in N ticks (`world/entity/LivingEntity.java:201-208,2456-2478,
  2693-2705`).
- **N = 3** for all entity move/teleport/head packets
  (`ClientPacketListener.java:465-521`); boats use 10, minecarts packet+2
  (`world/entity/vehicle/Boat.java:244-250`,
  `AbstractMinecart.java:753-762`).
- Deltas apply against tracked **packet coordinates**, not render position,
  so accumulated short deltas never drift (`ClientboundMoveEntityPacket.java:
  31-36`, `ServerEntity.updateSentPos`, `ServerEntity.java:278-282`).
- Velocity packets decode as short/8000 blocks/tick
  (`ClientPacketListener.java:429-436`).
- `RemotePlayer` has `noPhysics = true` and an `aiStep` that only consumes
  lerps — no local physics for other players
  (`client/player/RemotePlayer.java:11-56`).
- Locally controlled entities zero their lerp and ignore server move nudges
  (`LivingEntity.java:2456-2459`, `Entity.java:2667-2670`,
  `ClientPacketListener.java:473,495`).

## Speculative client actions and reconciliation (1.17.1 specifics)

- **Block breaking**: the client predicts the entire dig (progress, then local
  `setBlock` on completion) and records every action in an `unAckedActions`
  map (cap 50). Every server-side `handleBlockBreakAction` path replies with
  `ClientboundBlockBreakAckPacket(pos, serverState, action, allGood)`; on a
  failed ack the client restores the server state and may snap the player
  back if the restored block collides
  (`client/multiplayer/MultiPlayerGameMode.java:100-224,443-465`,
  `server/level/ServerPlayerGameMode.java:120-225`). Server distance gate:
  eye-offset dist-sq > 36 (6 blocks) → "too far".
- **Block placement/use**: client executes `use`/`useOn` locally with **no
  sequence id**; the server unconditionally replies with two
  `ClientboundBlockUpdatePacket`s (clicked + face-adjacent position),
  overwriting any wrong prediction
  (`MultiPlayerGameMode.java:259-298`,
  `ServerGamePacketListenerImpl.java:984-1002`).
- **Inventory**: 1.17.1 has **no transaction-ack packets** — it is the first
  version of the `stateId` scheme. The client predicts the click, sends
  changed slots + stateId; a server-side stateId mismatch triggers a full
  container resend, no reject packet
  (`MultiPlayerGameMode.java:352-374`,
  `ServerGamePacketListenerImpl.java:1298-1325`,
  `world/inventory/AbstractContainerMenu.java:763-765`).
- 1.17.1 has **no** block-change sequence numbers (1.19+) —
  `ServerboundUseItemOnPacket` carries only hand + hit result.

## Ordering, reliability, threading model

- One ordered TCP stream per client, `TCP_NODELAY` both ends; NIO/epoll; the
  protocol leans on strict ordering everywhere: registration-order packet
  ids, pipeline mutations sequenced by flush listeners, protocol-state
  switches pausing autoRead, teleport-id gating, disconnect-then-read-only.
- Threads: Netty IO pools, one game thread per side (`Server thread` /
  client main), `User Authenticator #N` for Mojang auth, HTTP pool for
  client `joinServer`.
- Of 46 serverbound PLAY handlers, 41 hop to the game thread via
  `ensureRunningOnSameThread` — **including all movement**. Exceptions that
  run (partly) on the Netty thread: keepalive (latency accuracy), pong,
  custom payload (ignored), and chat/book/sign text, which validate on the
  Netty thread then re-post mutation to the server thread after async text
  filtering (`ServerGamePacketListenerImpl.java:1105-1144`).
- Off-thread re-posted packets are dropped if the connection has closed;
  queued handler tasks drain during the inter-tick wait, adding at most about
  one tick of dispatch latency.

## Constants quick reference

| Constant | Value | Receipt |
|---|---|---|
| Tick interval | 50 ms (20 Hz) | `MinecraftServer.java:166` |
| Catch-up skip threshold | 2000 ms | `MinecraftServer.java:675-681` |
| Compression threshold | 256 bytes (default) | `DedicatedServerProperties.java:83` |
| Max packet body | 8 MiB | `PacketEncoder.java:43-44` |
| Chunk packet read cap | 2 MiB | `ClientboundLevelChunkPacket.java:21` |
| Keepalive / read timeout | 15 s / 30 s | `SGPLI.java:157`, `Connection.java:290` |
| Movement send threshold | 9.0E-4 dist-sq or 20 ticks | `LocalPlayer.java:239,81` |
| Moved too quickly | 100 (300 elytra) dist-sq × burst | `SGPLI.java:815-823` |
| Moved wrongly | 0.0625 dist-sq | `SGPLI.java:845` |
| Floating kick | 80 ticks | `SGPLI.java:210-219` |
| Entity lerp steps | 3 (boats 10) | `ClientPacketListener.java:501` |
| Entity delta precision | 1/4096 block; ±8 block short range | `ClientboundMoveEntityPacket.java:12` |
| Entity keyframe / forced teleport | 60 / 400 ticks | `ServerEntity.java:122,129` |
| Player tracking | range 32 chunks, interval 2 | `EntityType.java:496-498` |
| View distance | default 10, clamp 3..33 (+1 internal) | `ChunkMap.java:683` |
| Time broadcast | every 20 ticks | `MinecraftServer.java:867-875` |
| Autosave | every 6000 ticks | `MinecraftServer.java:833-840` |
