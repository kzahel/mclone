# Runtime Protocol

The logical host/client protocol shared by native and browser singleplayer,
remote clients, the native dedicated server, and tests.

**Authoritative source:** the Rust enums in
[`../native/crates/mclone-protocol/src/lib.rs`](../native/crates/mclone-protocol/src/lib.rs).
This doc explains the shape and the durable rules; the code is the truth.

[`architecture.md`](./architecture.md) owns the runtime boundary.
[`runtime-data-model.md`](./runtime-data-model.md) owns chunk and block-state
facts. [`loading-persistence.md`](./loading-persistence.md) owns world/chunk
lifecycle and save policy. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md)
owns host scheduler rules. [`minecraft-client-replica-research.md`](./minecraft-client-replica-research.md)
records the vanilla client-replica and networking review behind the split.
Per-platform connect status lives in [`topics/platform-parity.md`](./topics/platform-parity.md).

> This protocol began in the retired TypeScript engine and was reshaped around
> the Rust client/server crates. The old TypeScript-era names (`open_world`,
> `set_player_input`, `poll_world_updates`, `chunk_light_delta`,
> `session_state`) and the HTTP-polling / Node lineage are gone. This doc
> describes only the current Rust protocol plus clearly-labeled future intent.

## Core Rule

One logical protocol for local and remote play. Singleplayer is a client
joining a local authoritative host; multiplayer is the same client joining the
native dedicated server. Only the transport changes:

- native singleplayer and tests use the in-process `LocalTransport`
- browser singleplayer runs the authoritative host in a worker and talks to it
  over a worker channel
- remote play uses native TCP (desktop) or WebSocket (browser)

The renderer never gets a protocol bypass. It consumes replicated
chunk/light/entity facts; it does not read host internals or run worldgen.

Host messages are the only source of canonical client chunk facts. Clients must
not fill missing chunks by running seed-based worldgen or decoration locally.
Missing data stays explicit so rendering can show loading state and prediction
can report missing collision facts.

## Protocol Version And Handshake

`PROTOCOL_VERSION` (currently `15`) is exchanged in the transport handshake
before any messages — `MCLONE_NATIVE_TCP` for native TCP, `MCLONE_WS` for
WebSocket. The server replies accept or reject; a mismatch fails the connection
with `ProtocolVersionMismatch`.

This is **strict equality** — there is no capability or version-range
negotiation yet. That is a known cross-play gap once web (deployed URL), Android
(APK), and desktop (binary) builds can drift apart; tracked in
[`topics/platform-parity.md`](./topics/platform-parity.md).

## Logical Messages Vs Wire Codecs

Keep the message model separate from wire encoding. The logical protocol is two
Rust enums; each transport decides how their variants move (length-prefixed
binary frames over TCP, WebSocket frames in the browser, in-process handoff for
`LocalTransport`). A current wire detail must not become the canonical protocol
model.

### Client commands (`ClientCommand`)

| Command | Purpose |
|---|---|
| `SetChunkView` | view-shaped chunk-interest command (center + render/tracking radius) |
| `MovePlayer` | sequenced movement record — `Pos` / `PosRot` / `Rot` / `StatusOnly`, mirroring Java 1.17 `ServerboundMovePlayerPacket` |
| `PlayerAction` | block-destroy lifecycle (start/stop/abort destroy) plus debug instant-break |
| `UseItemOn` | server-authoritative use/place against a `BlockHitResult` |
| `SetCarriedItem` | select the active hotbar slot |
| `AcceptTeleport` | acknowledge a server `PlayerPosition` teleport id |

`SetChunkView` is an interest command, not a synchronous "load my whole view
now" RPC — chunk snapshots come back as host updates. There is no world-open or
session-join command: the world/seed is host-owned (the dedicated server is
launched with its seed) and play begins once the handshake is accepted. Client
commands are intents, not client-owned state mutations.

### Host updates (`ServerUpdate`)

| Update | Purpose |
|---|---|
| `ChunkSnapshot` | baseline chunk facts; **packed sky/block light rides inside the snapshot** (no separate light message) |
| `ChunkUnload` | release a chunk from client view/cache |
| `SectionBlockUpdates` | block mutations within a loaded section after the baseline |
| `TimeUpdate` | authoritative world day-time (ticks) for the day/night cycle |
| `PlayerPosition` | authoritative local-player position/rotation correction with relative flags and a teleport id |
| `RemotePlayerAdd` / `RemotePlayerUpdate` / `RemotePlayerRemove` | other players entering / moving in / leaving the client's tracked view |
| `EntitySnapshot` | entity baseline for a visible chunk; passive mobs are stackless, item entities carry an `ItemStackSnapshot` |
| `EntityUpdate` | partial entity position/rotation/age update after a baseline |
| `EntityRemove` | explicit entity untrack/remove for the replica |

Failures are surfaced through the transport handshake and connection errors, not
yet an in-band error update.

The loop is **request/response**: the host emits queued updates as the reply to
a client command. A server-push lane — so a player who stops sending commands
still sees others move — is still to come.

### Entity Snapshots

`EntitySnapshot` covers tracked world entities. Current runtime kinds are:

| Kind | Extra snapshot data |
|---|---|
| `Cow` | none |
| `Chicken` | none; visible wing pose is client-derived presentation state |
| `Item` | `ItemStackSnapshot` with `ItemKind` and count; currently `Egg` is the only item kind |
| `DebugCube` | optional debug rotation |

`EntityUpdate` intentionally carries transform/age data only in the current
slice. If an item stack mutates later, add that through a general tracked entity
data path rather than a chicken-specific or item-specific update side channel.

### Likely future messages (design intent; not implemented)

| Message | Purpose |
|---|---|
| `chunk_delta` | broader block/section/light/block-entity mutation batching |
| `session_state` / `world_opened` / `world_error` | explicit session/world lifecycle and a stable in-band failure surface |
| `ack_world_updates` | client acknowledgement for a reliable streaming/push lane |
| `inventory_state` | player inventory and container state |
| `join_world` / `resume_session` | named player-slot join split from world open, once persisted players exist |

## Client Replica And Prediction Inputs

Prediction is a client-runtime concern sitting on top of the client replica. The
protocol must provide enough authoritative facts for the replica and predictor
without exposing host internals:

- authoritative local-player position corrections with teleport ids (`PlayerPosition`)
- visible chunk, light, and entity snapshots/deltas for presentation
- nearby chunk/collision facts for the local player's prediction window

Those facts may share source payloads with rendering, lighting, and prediction,
but their meaning stays distinct: mesh payloads must not become collision truth,
and prediction caches must not become renderer-owned world state. When the host
cannot provide enough collision facts, that should be explicit so the client can
reduce prediction instead of silently drifting.

## Rate Separation Rule

The logical protocol must not imply that one transport request, one host world
tick, one snapshot, or one render frame equals one movement step. Movement is
sequenced command records plus authoritative correction snapshots. The host may
drain multiple movement commands inside one lower-rate world/network tick, and
the client may render many interpolated frames without creating additional
authoritative movement. Any future transport (push WebSocket, WebRTC) must carry
the same logical records rather than inventing a different gameplay protocol.

## Chunk Payloads

Chunk messages use the logical chunk model from
[`runtime-data-model.md`](./runtime-data-model.md): numeric block-state ids,
packed sections, binary-friendly buffers, sparse non-empty sections. Do not send
hot chunk payloads as large name/property object graphs. Small control messages
may stay simple while packed chunk/light section payloads use binary records or
transferable buffers.

## Transports

Implemented today (all in
[`../native/crates/mclone-net/src/lib.rs`](../native/crates/mclone-net/src/lib.rs)
plus app-owned sockets):

- **`LocalTransport`** — in-process loopback for native singleplayer and tests.
- **Native TCP** — length-prefixed binary frames, versioned handshake,
  request/response batches. Desktop's remote-dedicated path.
- **WebSocket** — `mclone-net` owns the frame/handshake codec; the socket lives
  in the app (tungstenite on the dedicated server's `--listen-ws` bridge,
  `web-sys` WebSocket in the browser client). The playable browser app can join
  through `?remoteWsUrl=ws://HOST:PORT`, and `native:web:remote-smoke` validates
  that path against a native dedicated WebSocket server.

There is no HTTP, WebTransport, or WebRTC gameplay transport, and no Node/Deno
host.

A future **server-push** lane (and later, if measured to be worth it, a WebRTC
datagram lane for high-rate entity/player snapshots) should preserve three
logical lanes — reliable ordered (interest, chunks, interactions, errors),
realtime superseding (movement/entity snapshots where newer replaces older), and
bulk/binary (packed chunk/light payloads) — without redesigning authority around
the transport.
