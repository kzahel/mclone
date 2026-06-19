# 056: Native Player Movement Networking

Status: active; deeper server authority hardening deferred.

## Purpose

Promote player movement networking, prediction, server validation, and later
reconciliation into its own native tactical so gameplay interaction work can stay
focused on block actions.

The current direction is to stay mostly Minecraft Java 1.17.1 shaped, but keep
the near-term server posture permissive:

- the client simulates responsive local movement and sends proposed position /
  rotation / on-ground facts
- the server trusts client movement for gameplay bring-up, clamps obviously
  invalid values, and keeps the correction/ack packet shape available as a
  protocol scaffold
- chunk, fluid, block, and entity simulation can remain lower-frequency fixed
  ticks while movement packets are ingested between ticks
- an FPS-style model with input commands, higher-frequency authoritative
  movement, client prediction, and reconciliation is a possible future runtime
  mode, but it should be designed as a separate authority strategy rather than
  quietly replacing the Java-shaped path

## Reference Shape

Read these Java 1.17.1 files before changing this lane:

- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundMovePlayerPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/Connection.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`

Important reference facts:

- `ServerboundMovePlayerPacket` is a family of four packet shapes:
  `Pos`, `PosRot`, `Rot`, and `StatusOnly`
- each packet carries `onGround`
- packets expose `hasPos` and `hasRot`; missing position or rotation reads from
  the server's current player value
- `ServerGamePacketListenerImpl` tracks `firstGoodX/Y/Z`,
  `lastGoodX/Y/Z`, `receivedMovePacketCount`, `knownMovePacketCount`,
  `awaitingTeleport`, and `awaitingTeleportTime`
- each server listener tick calls `resetPosition()` and then copies
  `receivedMovePacketCount` into `knownMovePacketCount`
- `ServerConnectionListener` owns the server-side connection list and ticks each
  `Connection`; `Connection.tick()` delegates to `ServerGamePacketListenerImpl`
  when the connection has reached gameplay state
- the Java server listener uses Netty NIO/epoll event-loop groups for socket IO,
  sets `TCP_NODELAY`, adds each accepted `Connection` to a synchronized
  connection list, and lets the server tick thread walk that list
- `Connection.send(...)` flushes queued packets when connected, otherwise
  enqueues them; actual channel writes are scheduled back onto the Netty event
  loop when needed
- client gameplay packets are sent through `ClientPacketListener.send(...)`,
  which delegates to the long-lived `Connection` rather than reconnecting per
  packet
- `handleMovePlayer` rejects non-finite values, clamps world bounds, wraps
  rotation, handles pending teleports, detects excessive packet burst /
  too-fast movement, moves the server player, detects impossible movement, and
  teleports the client back when required

This is permissive compared with an input-only FPS server authority model. The
client is not blocked waiting for the server to compute heading at the tick
rate; the server validates and corrects the proposed movement stream.

For the current native gameplay bring-up, do not spend more time deepening the
server-authoritative side of this lane. Velocity-aware validation, collision
rollback, wrong-movement rollback, and prediction/reconciliation are deferred
until multiplayer correctness or anti-cheat pressure makes them worth the
complexity.

## Progress

- 2026-06-19: Added the Java-shaped movement packet family, client `PosRot`
  sync, server finite/clamp/wrap handling, movement packet counters, and
  first/last good tick-boundary bookkeeping.
- 2026-06-19: Added the first too-fast movement validation skeleton. The native
  server bootstraps the first positional sync because spawn/login position sync
  is not implemented yet, then applies Java's packet-count-scaled movement
  delta threshold and rejects excessive position deltas without mutating pose.
  Server velocity is still treated as zero until velocity becomes server-owned.
- 2026-06-19: Added the first correction/teleport acknowledgement path. Too-fast
  native movement now emits a Java-shaped absolute `PlayerPosition` correction,
  the server records an awaiting teleport id, normal movement is ignored until a
  matching `AcceptTeleport` command arrives, and the native window client applies
  the correction, sends the ack, then sends a post-correction `PosRot`.
- 2026-06-19: Added Java-shaped pending correction resend behavior. If another
  movement packet arrives while a correction remains unacked for more than 20
  simulation ticks, the server reissues the correction with a fresh teleport id;
  exactly 20 ticks is still below the resend threshold.
- 2026-06-19: Added initial server-issued spawn position sync. The integrated
  server chooses a safe surface-ish spawn near the first requested chunk view,
  sends it as an absolute player-position update, and the native client applies
  and acknowledges it before normal movement sync.
- 2026-06-19: Disabled normal too-fast movement correction for the near-term
  permissive posture. The server still rejects non-finite packets, clamps Java
  world bounds, and uses teleport acknowledgement for server-issued positions,
  but finite client movement deltas are trusted.
- 2026-06-19: Added Java-shaped native client movement packet selection. Normal
  sync now chooses `PosRot`, `Pos`, `Rot`, `StatusOnly`, or no packet using the
  reference position delta, rotation delta, on-ground, and 20-call reminder
  shape; post-correction acknowledgement still forces an explicit `PosRot`.
- 2026-06-19: Started Java-shaped dedicated movement ingestion. The native TCP
  transport can now read multiple client command frames per connection, and the
  dedicated server stages per-connection movement packets before flushing them
  in order to the shared server movement state.
- 2026-06-19: Split dedicated session handling out of the app entry point and
  made dedicated TCP sessions persistent. Each client command frame now receives
  its own update batch on the same connection, movement is staged/flushed at the
  session boundary, and the session tick marks Java-shaped movement counters
  after advancing the shared server simulation.
- 2026-06-19: Moved the native remote client onto a reusable TCP client
  session. `mclone-net` owns the low-level persistent native client session,
  the native app owns a focused remote-session wrapper for address/context
  handling, and `WindowSceneRuntime` stores the session instead of reconnecting
  for every gameplay command.
- 2026-06-19: Split dedicated socket IO from dedicated gameplay sessions and
  added multi-client accept. Native dedicated now has an accept/connection IO
  module that can keep multiple persistent TCP clients blocked on sockets while
  the server thread serializes command handling through one `IntegratedServer`,
  keeping the shape close to Java's IO-event-loop plus server-ticked connection
  list without adding an async runtime yet.

## Native Direction

Keep the first native implementation boring, reference-shaped, and permissive:

- protocol carries the same four movement packet variants
- client movement remains local and responsive
- native client chooses the Java-shaped movement packet variant for each normal
  sync instead of always sending `PosRot`
- server stores player position, rotation, `on_ground`, packet counters, and
  first/last good positions
- server command handling remains no-output for valid movement; correction
  packets are retained as explicit protocol messages but should not become the
  main gameplay focus yet
- server reach checks for block interaction use server-owned player state, never
  position embedded in block action payloads
- dedicated and integrated server paths should share the same movement state and
  validation code
- dedicated transport keeps movement buffering at the connection boundary so
  persistent sessions can preserve Java-shaped packet counters
- keep dedicated app modules focused: CLI/listener orchestration in `main`,
  per-connection command/tick/update flow in a session module
- keep dedicated network IO separate from gameplay sessions: socket accept/read
  and write-back workers live in a connection module, while the main server loop
  owns `IntegratedServer` mutation and one `DedicatedSession` per connection
- the current native dedicated transport uses one blocking IO worker per client
  and has no intentionally low hard cap; the bring-up target is to tolerate at
  least 32 connected clients, while automated unit coverage keeps the concurrent
  socket sample smaller to avoid brittle stress in the test harness
- keep native client remote transport focused: the scene runtime owns a
  long-lived remote session, while the remote-session module delegates framed
  command writes and update-batch reads to `mclone-net`

Do not introduce a giant movement manager file. Keep the shape modular:

- shared protocol packet shape in `mclone-protocol`
- client local input/movement/prediction in `mclone-client`
- server movement validation state in `mclone-server::player`
- integrated/dedicated transport glue at the server boundary
- collision and pose primitives in `mclone-core` or focused client/server
  modules only when they are shared for real

## FPS-Style Future Option

If we later want a more first-person-shooter-style network model, treat it as a
new authority strategy with explicit design work. Likely additions:

- input-command stream with sequence numbers and client timestamps
- server-side movement replay using the same collision/friction rules
- authoritative snapshots or corrections carrying acknowledged input sequence
- client-side prediction history and replay after correction
- interpolation for remote players
- either a higher movement tick rate or a decoupled movement substep lane

That does not necessarily mean chunk updates, worldgen, lighting, or block-fluid
simulation need the same tick rate. Movement can have its own packet cadence and
substep policy while chunk publication stays coarser and budgeted.

## First Implementation Slice

Implement the Java packet/state skeleton without full correction yet:

- `MovePlayerCommand::{Pos, PosRot, Rot, StatusOnly}` in `mclone-protocol`
- protocol codec round trips for all movement variants
- client movement sync emits Java-shaped `Pos`, `Rot`, `PosRot`, `StatusOnly`,
  or no packet based on changed fields and the 20-call position reminder
- server movement state applies missing-position / missing-rotation fallback
- server rejects non-finite position or rotation before mutating state
- server clamps horizontal and vertical Java movement bounds
- server wraps yaw/pitch into the Java-style `[-180, 180)` range
- server increments `received_move_packet_count` on accepted movement
- integrated simulation ticks record `first_good_position`,
  `last_good_position`, and `known_move_packet_count`
- server tracks Java-shaped movement packet counters but, for now, trusts finite
  client position deltas instead of issuing too-fast corrections
- server returns absolute player-position updates for server-issued position
  syncs and clears the awaiting correction only after the matching teleport ack
- server resends stale pending corrections on subsequent movement packets after
  Java's `> 20` tick threshold
- server sends the first spawn position instead of bootstrapping from the first
  movement packet

This slice intentionally does not yet implement, and now defers:

- wrong-movement / collision rollback
- server-owned velocity in movement validation
- active too-fast movement correction
- remote-player interpolation
- FPS-style prediction/reconciliation

## Near-Term Follow-Ups

1. Add remote-player state publication once the dedicated path has multiple
   connected clients worth visualizing.
2. Add player/session identities so dedicated movement state is not still a
   shared single-player server player behind multiple connections.
3. Revisit the FPS-style authority option only after the Java-shaped path is
   usable and measured.

## Deferred Authority Hardening

- Re-enable too-fast validation only with server-owned velocity facts so it can
  subtract the Java-equivalent `deltaMovement.lengthSqr()`.
- Add server collision sanity and wrong-movement rollback once server-side
  entity collision facts are worth owning.
- Tighten correction/reconciliation semantics only if multiplayer behavior or a
  future competitive mode needs it.

## Validation

Use the native validation lanes:

- `cargo test --manifest-path native/Cargo.toml`
- `pnpm native:movement:smoke`
- `pnpm native:timedemo:smoke`
- `pnpm native:web:build`
- `pnpm native:web:smoke`

For rendered-output-affecting movement slices, capture a native screenshot to
`/tmp` and inspect it before closing the slice.
