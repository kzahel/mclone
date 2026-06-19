# 056: Native Player Movement Networking

Status: active.

## Purpose

Promote player movement networking, prediction, server validation, and later
reconciliation into its own native tactical so gameplay interaction work can stay
focused on block actions.

The current direction is to stay mostly Minecraft Java 1.17.1 shaped:

- the client simulates responsive local movement and sends proposed position /
  rotation / on-ground facts
- the server accepts movement packets in a permissive validation lane, clamps
  obviously invalid values, and can later correct the player by teleporting them
  back
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

## Native Direction

Keep the first native implementation boring and reference-shaped:

- protocol carries the same four movement packet variants
- client movement remains local and responsive
- native client may continue to send `PosRot` every movement sync until we add
  packet suppression / variant selection
- server stores player position, rotation, `on_ground`, packet counters, and
  first/last good positions
- server command handling remains no-output for valid movement; later correction
  packets should be explicit protocol messages
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

This slice intentionally does not yet implement:

- too-fast movement validation
- wrong-movement / collision rollback
- teleport correction packets and teleport ack handling
- movement packet suppression or optimal `Pos`/`Rot`/`StatusOnly` emission
- remote-player interpolation
- FPS-style prediction/reconciliation

## Follow-Ups

1. Add Java-shaped too-fast validation from `handleMovePlayer`, using packet
   count delta, movement delta, and velocity delta.
2. Add server collision sanity once server-side entity collision facts are
   available.
3. Add correction protocol messages and teleport acknowledgement state.
4. Make the native client choose `Pos`, `Rot`, `PosRot`, or `StatusOnly` based
   on changed fields instead of always sending `PosRot`.
5. Move dedicated transport ingestion toward per-connection movement buffering
   that preserves Java-shaped packet counters.
6. Revisit the FPS-style authority option only after the Java-shaped path is
   usable and measured.

## Validation

Use the native validation lanes:

- `cargo test --manifest-path native/Cargo.toml`
- `pnpm native:movement:smoke`
- `pnpm native:timedemo:smoke`
- `pnpm native:web:build`
- `pnpm native:web:smoke`

For rendered-output-affecting movement slices, capture a native screenshot to
`/tmp` and inspect it before closing the slice.
