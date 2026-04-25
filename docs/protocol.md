# Runtime Protocol

Durable guidance for the logical host/client protocol used by browser singleplayer, browser remote clients, dedicated Node hosts, and tests.

[`architecture.md`](./architecture.md) owns the runtime boundary. [`runtime-data-model.md`](./runtime-data-model.md) owns chunk and block-state facts. [`loading-persistence.md`](./loading-persistence.md) owns world/chunk lifecycle and save policy. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md) owns host scheduler rules that keep input, ticks, and polling from blocking behind chunk jobs.

## Core Rule

Use one logical protocol for local and remote play.

Browser singleplayer should be a browser client joining a local authoritative host in a worker. Browser multiplayer should be the same browser client joining a remote authoritative host. Creating a new singleplayer world is a host-side world creation/open operation followed by the same join/session flow.

Only the transport changes:

- in-process tests can call the host directly
- local browser singleplayer can use `postMessage` or `MessagePort`
- remote play can use HTTP polling today and a push transport later

The renderer should not get a special protocol bypass.

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
   The client receives a session id, player id, world metadata, and initial authoritative state.

3. **Resume session**
   A client can present a previous session id. The host either resumes it or returns a stable error that lets the client reopen and resync.

4. **Set interest**
   The client tells the host which chunks it needs. Today this is `set_chunk_view`; the durable concept is chunk interest, with view-based interest as the first policy. Setting interest should acknowledge the new session/interest revision promptly and queue chunk work. It should not require the command response to contain every newly visible chunk.

5. **Run update loop**
   The client sends input/commands. The host ticks authoritative state and sends or queues updates.

6. **Close or expire session**
   The host can eventually release per-session interest and player state while preserving world/save state.

Current protocol names may evolve incrementally, but the semantics should move toward this lifecycle.

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
| `join_world` / `resume_session` | split session semantics from world creation/opening if useful |
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

## Client Prediction Inputs

Prediction is a client runtime concern, but the host protocol must provide enough authoritative facts for it without exposing host internals.

The client prediction worker should be able to hydrate a bounded prediction world from client-facing messages:

- authoritative local-player movement snapshots with ack sequence and body restart facts
- nearby chunk/collision snapshots or deltas for the local player's prediction window
- movement physics and collision revision facts
- dynamic collider snapshots for entities that can affect local prediction

Those messages may share source payloads with rendering, but their meaning should stay distinct. Chunk facts can feed a mesh worker and a prediction worker; mesh payloads must not become collision truth, and prediction caches must not become renderer-owned world state.

When the host cannot provide enough collision facts for prediction, the protocol should make that explicit through missing-collision diagnostics or revision mismatch facts. The client can then fall back to reduced prediction or accept correction instead of silently drifting.

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

## Polling And Push Transports

HTTP polling is acceptable as an interim transport because the current gameplay loop is still small.

Do not redesign authority around polling. The logical protocol should be ready to move to push delivery when measurements or gameplay needs require it.

Transport adapters should preserve three logical lanes:

- reliable ordered lane: open/join/resume, chunk interest, chunk snapshots/deltas, interactions, inventory, errors, and world/session state
- realtime superseding lane: movement command bundles and player/entity snapshots where newer data can replace older unprocessed data
- bulk/binary lane: packed chunk, light, and similar large payloads; these may use binary framing or transferables, but remain logically reliable unless a later protocol explicitly says otherwise

Movement correctness must not depend on HTTP request cadence. A polling response may carry movement commands and snapshots today, but the movement model is sequenced command records plus authoritative ack snapshots. WebSocket, WebTransport, or WebRTC adapters later should implement the same logical records rather than inventing a new gameplay protocol.

A push-capable transport becomes justified when:

- player/entity update frequency makes polling latency or overhead visible
- block/entity updates need server-initiated delivery between client commands
- reconnect/replay semantics are clear enough to preserve correctness

Moving to WebSocket or another push transport should not change world ownership or message meaning. WebSocket is the first likely push lane because it preserves reliable ordered delivery. WebRTC/WebTransport datagram lanes are later options only if measurements justify lossy realtime traffic, and they require explicit command bundling, sequence-gap handling, authoritative acknowledgements, periodic baselines, and reliable control/chunk lanes.
