# Runtime Protocol

Durable guidance for the logical host/client protocol used by browser singleplayer, browser remote clients, dedicated Node hosts, and tests.

[`architecture.md`](./architecture.md) owns the runtime boundary. [`runtime-data-model.md`](./runtime-data-model.md) owns chunk and block-state facts. [`loading-persistence.md`](./loading-persistence.md) owns world/chunk lifecycle and save policy. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md) owns host scheduler rules that keep input, ticks, and polling from blocking behind chunk jobs. [`minecraft-client-replica-research.md`](./minecraft-client-replica-research.md) records the vanilla client-replica and networking source review that informs the client/host split.

## Core Rule

Use one logical protocol for local and remote play.

Browser singleplayer should be a browser client joining a local authoritative host in a worker. Browser multiplayer should be the same browser client joining a remote authoritative host. Creating a new singleplayer world is a host-side world creation/open operation followed by the same join/session flow.

Only the transport changes:

- in-process tests can call the host directly
- local browser singleplayer can use `postMessage` or `MessagePort`
- remote play can use HTTP polling today and a push transport later

The renderer should not get a special protocol bypass.

The active client-runtime facade layer keeps this protocol split explicit:

- `WorldHost` is the logical authority surface shared by integrated and dedicated hosts.
- `IntegratedServer` is a browser-singleplayer facade over that same authority surface, not a new protocol.
- `ClientRuntime` owns protocol application and transport polling/push draining.
- `ClientWorld` owns hydrated replica facts and revision views.

Transport adapters still decide only how records move. They do not decide host ticks, player command quanta, snapshot cadence, transport cadence, or render frames.

Host protocol messages are the only source of canonical client chunk facts. Clients must not compensate for missing chunks by running seed-based worldgen or decoration locally. Missing data should stay explicit so rendering can show loading state and prediction can report missing collision facts.

## Logical Messages Vs Wire Codecs

Keep the message model separate from wire encoding.

The logical protocol defines commands and updates in engine terms:

- open or create world
- join or resume session
- set chunk interest
- send player input
- receive chunk snapshots/deltas
- receive entity snapshots/deltas
- receive player/session state

Wire codecs decide how those messages move:

- structured clone plus transferable buffers for workers
- JSON envelopes for current HTTP control messages
- binary records for packed chunk payloads
- WebSocket/WebTransport frames later if polling stops fitting

Do not let a current wire detail, such as HTTP JSON, become the canonical protocol model.

## Session Lifecycle

The durable lifecycle should be explicit even if current code still folds some steps together.

1. **Open or create world**
   The host validates save id, seed, preset, storage schema, and world metadata. If a requested save does not exist and creation is allowed, the host creates metadata.

2. **Join session**
   The client presents a player name/profile for the world. The host allocates or resumes an authoritative player slot, then returns a session id, player id, player name/profile facts, world metadata, and initial authoritative state.

3. **Resume session**
   A client can present a previous session id. The host either resumes it or returns a stable error that lets the client reopen and resync.

4. **Set interest**
   The client tells the host which chunks it needs. Today this is `set_chunk_view`; the durable concept is chunk interest, with view-based interest as the first policy. Setting interest should acknowledge the new session/interest revision promptly and queue chunk work. It should not require the command response to contain every newly visible chunk.

5. **Run update loop**
   The client sends input/commands. The host ticks authoritative state and sends or queues updates.

6. **Close or expire session**
   The host can eventually release per-session interest and player state while preserving world/save state.

Current protocol names may evolve incrementally, but the semantics should move toward this lifecycle.

## Player Identity And Slots

Keep connection/session identity separate from player identity:

- `sessionId` is a connection/resume handle. It owns transport state, chunk interest, pending messages, and reconnect behavior.
- `playerId` is the authoritative player slot inside a world/save. Movement, inventory, game mode, spawn position, and future persistence attach to this slot.
- player name/profile data is the human-facing join identity. Local singleplayer can default it, but multiplayer joins should provide it explicitly.
- resuming by `sessionId` reconnects the same active session and therefore the same player slot while the session exists.
- future profile-based resume may reclaim a persisted player slot even after the transport session expired, but that should be explicit and not inferred from a random session id.

The current implementation still folds open/create and join into `open_world`, and the remote debug path currently uses `playerId === sessionId` as a compatibility shortcut. Do not build grounded movement, inventory, or persisted player state on that shortcut. The next multiplayer-facing runtime slice should split or model join semantics so a session references a tracked player slot with a name/profile instead of being the player slot.

## Client Commands

Current and near-term client-to-host commands:

| Command | Purpose |
|---|---|
| `open_world` | open/create a world and establish baseline metadata |
| `set_chunk_view` | current view-shaped chunk-interest command |
| `set_player_input` | send sequenced movement command records to authoritative host |
| `poll_world_updates` | drain queued server-originated updates on polling transports |

`set_chunk_view` is an interest command, not a synchronous "load my whole view now" RPC. Chunk snapshots are host-originated updates that may be returned by a polling response or delivered later by a push transport.

Likely future commands:

| Command | Purpose |
|---|---|
| `join_world` / `resume_session` | split named player-slot join semantics from world creation/opening |
| `set_chunk_interest` | generalize beyond camera-centered chunk views |
| `ack_world_updates` | support reliable streaming transports |
| `interact_block` / `use_item` | server-authoritative gameplay commands |

Client commands are intents. They are not client-owned state mutations.

## Host Updates

Current and near-term host-to-client updates:

| Update | Purpose |
|---|---|
| `world_opened` | world dimensions and save metadata |
| `session_state` | session/player/save ids, revision, current interest state |
| `player_state` | authoritative player position/rotation/tick/revision |
| `chunk_snapshot` | baseline chunk facts |
| `chunk_light_delta` | stored light changes for an already loaded chunk |
| `chunk_unload` | release a chunk from client view/cache |
| `entity_snapshot` | authoritative generated entity baseline data for a visible chunk |
| `world_error` | stable failure surface |

Likely future updates:

| Update | Purpose |
|---|---|
| `chunk_delta` | block, section, light, and block-entity mutations |
| `entity_delta` | authoritative entity movement, metadata changes, and removals after an entity baseline exists |
| `tick` / `time_state` | world time and tick metadata |
| `inventory_state` | player inventory and container state |

Updates should carry revision or tick context where ordering matters.

`entity_snapshot` is currently a baseline/interested-chunk update, similar to chunk snapshots. Polling transports should keep session/player state ahead of entity snapshots when capped, then drain entity snapshots with other chunk-interest bulk data.

## Client Replica And Prediction Inputs

Prediction is a client runtime concern, but it sits on top of the broader client replica. The host protocol must provide enough authoritative facts for the client replica and predictor without exposing host internals.

The client replica/runtime should be able to hydrate visible world state and a bounded prediction view from client-facing messages:

- authoritative local-player movement snapshots with ack sequence and body restart facts
- nearby chunk/collision snapshots or deltas for the local player's prediction window
- visible chunk, light, block-entity, and entity snapshots for presentation
- movement physics and collision revision facts
- dynamic collider snapshots for entities that can affect local prediction

Those messages may share source payloads with rendering, lighting, and prediction, but their meaning should stay distinct. Chunk facts can feed a mesh worker and a prediction view; mesh payloads must not become collision truth, and prediction caches must not become renderer-owned world state.

When the host cannot provide enough collision facts for prediction, the protocol should make that explicit through missing-collision diagnostics or revision mismatch facts. The client can then fall back to reduced prediction or accept correction instead of silently drifting.

## Rate Separation Rule

The logical protocol must not imply that one transport request, one host world tick, one snapshot, or one render frame equals one movement step.

Future movement command records should keep these facts explicit:

- command sequence
- `commandQuantumUs`
- `stepCount`
- button and edge-button state for the command window
- movement physics revision
- collision/world revision facts when dynamic collision can affect prediction
- authoritative snapshot ack of the last processed command sequence

Polling, worker `postMessage`, WebSocket, WebTransport, and future WebRTC adapters may bundle or schedule records differently, but they must carry the same logical command/snapshot facts. The host may drain multiple fixed-quantum movement commands during one lower-rate world/network tick, and the client may render many frames from predicted/interpolated presentation state without creating additional authoritative movement.

## Versioning And Errors

Remote envelopes should stay protocol-versioned.

Stable remote error codes should exist for at least:

- protocol version mismatch
- unknown or expired session
- incompatible world request
- malformed message
- unauthorized or unsupported operation when those concerns become real

Worker transports should validate the same logical conditions even if they do not need HTTP-style envelopes.

## Chunk Payloads

Chunk messages should use the logical chunk model from [`runtime-data-model.md`](./runtime-data-model.md):

- numeric block-state ids
- packed sections
- binary-friendly buffers
- sparse non-empty sections

Do not send hot chunk payloads as large name/property object graphs once the packed model exists.

For remote transports, it is acceptable to keep small control messages in JSON while moving chunk section payloads to binary records or transferable buffers.

## WebSocket Remote Transport

The next remote transport target is a persistent WebSocket-backed message channel.

Do not redesign authority around WebSocket. The transport changes delivery mechanics only; logical messages, host authority, client-world hydration, prediction facts, and chunk ownership remain the same.

Transport adapters should preserve three logical lanes:

- reliable ordered lane: open/join/resume, chunk interest, chunk snapshots/deltas, interactions, inventory, errors, and world/session state
- realtime superseding lane: movement command bundles and player/entity snapshots where newer data can replace older unprocessed data
- bulk/binary lane: packed chunk, light, and similar large payloads; these may use binary framing or transferables, but remain logically reliable unless a later protocol explicitly says otherwise

Movement correctness must not depend on transport cadence. The movement model is sequenced command records plus authoritative ack snapshots. WebSocket should carry those same logical records instead of inventing a different gameplay protocol.

The current HTTP polling implementation is now a compatibility adapter, not the target remote path. WebSocket migration should:

- replace remote `poll_world_updates` network requests with server-pushed updates
- keep a local drain API on `ClientRuntime` so the presentation loop can hydrate queued pushed messages without owning sockets
- split the existing HTTP-specific envelope code from reusable message serialization
- keep session resume, player slot identity, chunk interest, and reconnect behavior explicit
- make packed chunk/light payloads ready for binary WebSocket frames after the initial JSON message-channel migration is stable

WebRTC/WebTransport datagram lanes are later options only if measurements justify lossy realtime traffic, and they require explicit command bundling, sequence-gap handling, authoritative acknowledgements, periodic baselines, and reliable control/chunk lanes.
