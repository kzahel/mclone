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

`PROTOCOL_VERSION` (currently `34`) is exchanged in the transport handshake
before any messages — `MCLONE_NATIVE_TCP` for native TCP, `MCLONE_WS` for
WebSocket. The server replies accept or reject; a mismatch fails the connection
with `ProtocolVersionMismatch`.

The same handshake carries the unauthenticated local player profile UUID,
display name, and a `u64` supported-capability mask. The server accepts the
known client/server intersection and returns it in the handshake response.
`DEBUG_ACTIONS` is the first optional bit. The UUID selects one realm-scoped
player record, while the display name is last-seen presentation metadata. A
dedicated world rejects a second live connection claiming an already active
UUID. This is stable local identity and optional-feature negotiation, not
Mojang/Microsoft authentication, permissions, or proof of ownership.

Core versions still use **strict equality**; there is no version-range or
required-capability negotiation. That is a known cross-play gap once web
(deployed URL), Android (APK), and desktop (binary) builds can drift apart; tracked in
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
| `MovePlayer` | ordered, sequenced movement record — `Pos` / `PosRot` / `Rot` / `StatusOnly`, mirroring Java 1.17 `ServerboundMovePlayerPacket`; production sequences are wrapping nonzero `u32` values |
| `PlayerAction` | block-destroy lifecycle (start/stop/abort destroy) plus debug instant-break |
| `UseItemOn` | server-authoritative use/place against a `BlockHitResult` |
| `SetCarriedItem` | select the active hotbar slot |
| `AcceptTeleport` | acknowledge a server `PlayerPosition` teleport id |
| `SetPlayerAppearance` | publish the player's current model/appearance choice |
| `Respawn` | explicitly request revival after an authoritative death; ignored while living |
| `SetDebugHotbarSlot` | debug-only mutation of one server-owned hotbar slot |
| `ShootDebugPhysicsCube` | debug-only request to spawn a physics test entity |
| `KeepAlive` | immediate echo of the server's pending 64-bit liveness challenge |
| `Disconnect` | clean client close; currently `Quit` |

`SetChunkView` is an interest command, not a synchronous "load my whole view
now" RPC — chunk snapshots come back as host updates. There is no world-open or
session-join command: the world/seed is host-owned (the dedicated server is
launched with its seed) and play begins once the handshake is accepted. Client
commands are intents, not client-owned state mutations.

### Host updates (`ServerUpdate`)

| Update | Purpose |
|---|---|
| `SessionConfiguration` | negotiated capabilities, fixed gameplay/publication rates, and authoritative view ceilings |
| `SessionReady` | ordered admission from configuring to playing; follows configuration and precedes world state |
| `WorldInfo` | connection-scoped world facts currently carrying the obfuscated biome zoom seed |
| `ChunkSnapshot` | baseline chunk facts; **packed sky/block light rides inside the snapshot** (no separate light message) |
| `ChunkUnload` | release a chunk from client view/cache |
| `SectionBlockUpdates` | block mutations within a loaded section after the baseline |
| `TimeUpdate` | authoritative game time, day time, and daylight-cycle-running state; the client advances both 20 Hz between join/tick-1/20-tick corrections, conditionally advancing day time |
| `PlayerPosition` | authoritative local-player position/rotation correction with relative flags, latest accepted movement sequence, and teleport id |
| `PlayerExperience` | owner-only authoritative total experience restored from and dirtied into the realm-scoped player record |
| `PlayerStatistics` | owner-only typed realm statistics, including jumps, successful placements, and deaths |
| `PlayerLife` | owner-only ordered life epoch, finite/clamped health, maximum health, and optional typed death cause |
| `RemotePlayerAdd` / `RemotePlayerUpdate` / `RemotePlayerRemove` | other players entering / moving in / leaving the client's tracked view |
| `EntitySnapshot` | entity baseline for a visible chunk; passive mobs are stackless, item entities carry an `ItemStackSnapshot` |
| `EntityUpdate` | partial entity position/rotation/age update after a baseline; item entities also carry their current `ItemStackSnapshot` when stack data is present |
| `EntityRemove` | explicit entity untrack/remove for the replica |
| `KeepAlive` | server liveness challenge; remote transport actors echo without waiting for a drawable frame |
| `Disconnect` | typed, bounded logical close reason sent before transport shutdown |

Active-session failures are surfaced in-band when the server can send a typed
reason. Unexpected native/browser transport loss becomes a synthetic
end-of-stream/transport reason; the shared client keeps the first reason if a
later EOF follows an explicit close.

Native TCP and WebSocket remote transports are full-duplex publication
streams. Client commands and server publication frames move independently on
one reliable ordered connection. The dedicated host advances on its own
cadence, drains ready commands at host boundaries, and publishes routed chunk,
entity, remote-player, correction, and periodic time updates even when a client
sends no command. See
[`session-network-architecture.md`](./session-network-architecture.md) and
[`tactical/176-dedicated-autonomous-push-runtime.md`](./tactical/176-dedicated-autonomous-push-runtime.md).

A wire batch is only a publication-frame container; it does not acknowledge or
complete a command. Teleport acceptance and other gameplay acknowledgements
remain explicit logical messages. Producers decode off the drawable thread,
and every client applies decoded updates in receive order through the shared
budgeted runtime pump.

Current framing is deliberately simple:

- native commands are one little-endian `u32` payload length followed by one
  encoded `ClientCommand`;
- a native publication frame is a little-endian `u32` update count followed by
  one length-prefixed encoded `ServerUpdate` per entry; and
- WebSocket carries the same command payload and publication-batch contents in
  binary messages after its version handshake.

Frames are capped at 64 MiB. Each dedicated peer has an independent 64-frame /
64 MiB exact-encoded-byte outbound queue; saturation disconnects that slow peer
without blocking authority. Native client ingress is capped at 256 decoded
publication batches. Browser remote additionally caps worker-unacknowledged
publications and main transferable ingress at 64 MiB (4096 main-side updates)
and caps socket command buffering at 8 MiB.

### Entity Snapshots

`EntitySnapshot` covers tracked world entities. Current runtime kinds are:

| Kind | Extra snapshot data |
|---|---|
| `Cow` | none |
| `Chicken` | none; visible wing pose is client-derived presentation state |
| `Item` | `ItemStackSnapshot` with `ItemKind` and count; currently `Egg` is the only item kind |
| `DebugCube` | optional debug rotation |

`EntityUpdate` carries transform/age data and may carry item stack data. Current
server item updates include the current stack for item entities so stack merges
and partial pickups do not leave client replicas stale. Future non-item
metadata should still go through a general tracked entity data shape rather
than chicken-specific or one-off update messages.

### Likely future messages (design intent; not implemented)

| Message | Purpose |
|---|---|
| `chunk_delta` | broader block/section/light/block-entity mutation batching |
| broader `session_state` / `world_opened` / `world_error` | richer login/world-open disposition beyond the current configuration/ready/disconnect lifecycle |
| `inventory_state` | player inventory and container state |
| authenticated login result | add identity proof/player-record disposition before the implemented configuration/play admission |

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

The logical protocol must not imply that one command frame, one host world
tick, one snapshot, or one render frame equals one movement step. Movement is
sequenced command records plus authoritative correction snapshots. The host may
drain multiple ordered movement commands inside one lower-rate world/network tick, and
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
  independent command and publication streams. Desktop and native Android/XR
  remote-dedicated path.
- **WebSocket** — `mclone-net` owns the frame/handshake codec; the socket lives
  in transport adapters (tungstenite terminates directly in the dedicated
  connection registry; the production browser module worker owns
  `WebSocket`, handshake, command send, receipt, and decode). The playable
  browser app can join through `?remoteWsUrl=ws://HOST:PORT`, and
  `native:web:remote-smoke` validates that worker path against the native
  dedicated WebSocket server.

There is no HTTP, raw-UDP, WebTransport, or WebRTC gameplay transport yet, and
no Node/Deno host.

The reliable ordered publication lane is now implemented. Planned
carrier-neutral ephemeral messages for high-rate player pose will first gain a
dependency-free native UDP adapter and reliable-stream fallback. Later
WebTransport datagrams and browser-hosted WebRTC ephemeral channels must decode
to that same logical contract, preserve spawn/despawn, correction, and keyframe
barriers, and must not redesign authority around the carrier. Compression,
authentication, required capabilities, version ranges, and reconnect without
rebuilding the replica remain future protocol work.
