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

## Native Direction

Keep the first native implementation boring, reference-shaped, and permissive:

- protocol carries the same four movement packet variants
- client movement remains local and responsive
- native client may continue to send `PosRot` every movement sync until we add
  packet suppression / variant selection
- server stores player position, rotation, `on_ground`, packet counters, and
  first/last good positions
- server command handling remains no-output for valid movement; correction
  packets are retained as explicit protocol messages but should not become the
  main gameplay focus yet
- server reach checks for block interaction use server-owned player state, never
  position embedded in block action payloads
- dedicated and integrated server paths should share the same movement state and
  validation code

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
- client movement sync emits `PosRot` through the new variant
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
- movement packet suppression or optimal `Pos`/`Rot`/`StatusOnly` emission
- remote-player interpolation
- FPS-style prediction/reconciliation

## Near-Term Follow-Ups

1. Make the native client choose `Pos`, `Rot`, `PosRot`, or `StatusOnly` based
   on changed fields instead of always sending `PosRot`.
2. Move dedicated transport ingestion toward per-connection movement buffering
   that preserves Java-shaped packet counters.
3. Add remote-player state publication once the dedicated path has multiple
   connected clients worth visualizing.
4. Revisit the FPS-style authority option only after the Java-shaped path is
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
