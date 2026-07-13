# 176: Dedicated Autonomous Push Runtime

Status: active; Slices 0-2 completed 2026-07-13. Slice 3 autonomous dedicated
host cutover is next.

Topic: `multiplayer-networking`

Workstream: native Rust shared server/runtime/protocol, native TCP transport,
native desktop/Android/XR consumers, and native web/WASM worker transport.
Desktop TCP loopback is the first implementation lane. Web-worker and
cross-platform conformance are completion requirements, not separate product
features.

## Milestone

Replace the dedicated server's command-clocked request/response scaffolding
with one autonomously paced authoritative host and independent ordered command
and update streams. A connected client must receive chunk, entity, remote
player, correction, and periodic time updates without sending a polling
command. The world must advance at the configured host/gameplay cadence with
zero, one, or many clients, independent of aggregate command rate.

Preserve the client ingress work already completed by Tacticals
[`151`](151-remote-inbound-update-pipeline.md) and
[`154`](154-client-ingress-adapter-cleanup.md): producer-side decode, one
shared `ClientConnection` boundary, strict receive-order application, and a
budgeted thin frame pump. This tactical changes producer and authoritative-host
ownership; it must not create another client runtime path.

## Why Tick And Push Are One Coordinated Milestone

An autonomous server without an independent writer produces updates that the
current response-paired connection cannot deliver until another command
arrives. A push-capable writer without an autonomous host still leaves world
time, entities, worldgen publication, and remote-player observation driven by
client traffic. The two defects have separate internal seams but one product
cutover.

Implementation remains staged so commits stay reviewable:

1. separate global simulation from per-player publication;
2. make native connections capable of independent read/write and unsolicited
   update frames while retaining the old command-driven server behavior;
3. switch the dedicated host to paced tick-boundary command drain and push;
4. converge native consumers and remove response-pair compatibility state;
5. move production web remote socket/decode ownership into a worker and attach
   WebSocket sessions directly to the shared dedicated host;
6. close with cross-adapter conformance, performance evidence, and cleanup.

No intermediate commit may leave the default remote lane unable to complete
its existing smoke tests. The strict protocol-version gate permits an explicit
version bump at the product cutover; maintaining compatibility with old
binaries is not required.

## Governing Docs And Reference

| Doc | Ownership |
|---|---|
| [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md) | Current state, durable decisions, broader session/persistence/prediction ordering. |
| [`../topics/vanilla/networking.md`](../topics/vanilla/networking.md) | Minecraft Java 1.17.1 connection, server tick, publication, keepalive, and ordering receipts. |
| [`../session-network-architecture.md`](../session-network-architecture.md) | Shared client command/update boundary, frame-thread contract, and native/web topology. |
| [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md) | Tick-thread authority, asynchronous worldgen publication, and nonblocking host rules. |
| [`116-simulation-cadence-and-stepping.md`](116-simulation-cadence-and-stepping.md) | Existing shared host/gameplay/physics cadence model. |
| [`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md) | Historical bus/pacing implementation; Slice 6 hands push broadening here. |
| [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md) | Completed decoded remote ingress and app-frame decoupling. |
| [`154-client-ingress-adapter-cleanup.md`](154-client-ingress-adapter-cleanup.md) | Completed compatibility-path classification before push broadening. |

Vanilla 1.17.1 establishes the behavioral baseline:

- `MinecraftServer.runServer` owns a 50 ms tick clock independent of packet
  arrival;
- packet handlers reach the game thread through ordered task dispatch;
- chunk completion, section block batches, entity trackers, and time sync send
  from server-owned tick/publication work;
- `Connection.send` writes independently of inbound packets on one ordered,
  reliable, full-duplex stream; and
- autosave is tick-periodic (6000 ticks), not command-periodic.

Mclone deliberately keeps its existing wall-clock slip and configurable clean
cadence lanes. This tactical does not implement vanilla burst catch-up,
compression, login/auth, or movement validation.

## Verified Starting Shape

### Dedicated authority

- `mclone-dedicated-server/src/main.rs::run_server_loop_inner` blocks on
  `DedicatedNetwork::recv`.
- Every `DedicatedNetworkEvent::Command` carries a zero-capacity response
  sender. The authoritative loop handles one command and returns its update
  vector through that sender.
- `DedicatedSession::handle_client_command` handles the command, flushes
  movement, waits up to 120 seconds for all server jobs, advances one global
  simulation tick for that player target, and saves dirty chunks.
- Aggregate client command frequency therefore controls simulation frequency.

### Server state and publication

- `IntegratedServer::try_simulation_tick_report_for_player` advances global
  simulation but drains routed updates for one selected player.
- `TimeUpdate` is inserted into that selected tick report every simulation
  tick rather than broadcast on a publication period.
- Chunk, section, remote-player, and entity routing already accumulate ordered
  per-player updates in `PlayerChunkTracking`; this is the useful shared
  substrate for push.
- The local `NativeIntegratedServerRunner` already owns a paced server thread,
  command channel, update channel, and `SimulationCadence`. Its loop mechanics
  are reusable, but its local-player and single-outbound-queue assumptions are
  not the dedicated multiplayer runtime.

### Native transport and client

- A dedicated connection thread reads one command, sends it to authority,
  waits for its paired response, writes one update batch, and only then reads
  another command.
- `NativeClientIoSession` has one actor loop that writes one queued command and
  then blocks reading its paired update batch. Decode is correctly off-frame,
  but receipt still depends on outbound traffic.
- Desktop flat, desktop XR, flat Android, and Android XR normal remote paths
  already converge on `NativeClientIoSession` plus the shared
  `ClientConnection` pump.
- Response-era diagnostics and adapters still expose
  `pending_response_batches`, `response_sequence`, and
  `drain_command_updates` names.

### Web transport

- Browser remote uses WebSocket callbacks to decode response batches and feed
  the shared runtime pump, but it still counts pending command responses.
- The dedicated WebSocket listener creates a one-WebSocket-to-one-native-TCP
  lockstep bridge, so it inherits the response-paired upstream.
- Production browser completion does not yet prove that remote WebSocket
  ownership and decode are off the browser drawable/main event loop.

## Shared Ownership Map

| Owner | Responsibilities in this milestone |
|---|---|
| `mclone-protocol` | Logical command/update enums, codec validation, explicit semantic acknowledgements, protocol-version cutover. |
| `mclone-server` | Target-neutral simulation advance, tick cadence, command dispatch, movement tick boundaries, per-player routing/drain, periodic time publication, autosave policy. |
| `mclone-net` | Update-frame framing, native TCP reader/writer lanes, bounded queue accounting, decode metadata, connection errors. |
| `mclone-app-runtime` | One `ClientConnection`, ready-only drain, decoded envelopes, common queue diagnostics, budgeted apply. |
| `mclone-client` | Ordered authoritative replica application; no transport awareness. |
| `mclone-scene` | One frame pump point and render-dirty handoff; no socket or protocol policy. |
| dedicated app | Listener/CLI/process lifecycle and thin TCP/WebSocket adapter wiring around the shared host. |
| platform apps | Address/browser API/lifecycle sources only; no alternate ordering, polling, or queue policy. |

Code that grows platform app crates with gameplay publication, drain policy,
or response bookkeeping violates this ownership map. If the shared dedicated
host needs a reusable actor/connection registry, it belongs in `mclone-server`
or a narrowly shared host crate rather than in desktop or XR app code.

## Target Topology

```text
native client frame
  -> try-enqueue ordered ClientCommand
  -> ready-only drain of decoded ServerUpdate envelopes
  -> budgeted ordered replica apply

native client transport
  command queue -> TCP writer thread -> socket
  socket -> TCP reader thread -> decode -> update queue

browser client frame
  -> nonblocking message/command handoff
  -> ready-only drain through the same ClientConnection semantics

browser remote worker
  command messages -> WebSocket
  WebSocket events -> decode -> transferable/shared update handoff

dedicated authoritative host
  TCP/WS readers -> bounded command inbox
  tick boundary -> drain commands -> advance world once
                -> route/drain updates per player
  per-connection bounded outbound queues -> TCP/WS writers
```

There is one ordered reliable logical update stream per player. The existing
batch encoding may remain as a publication-frame container, but a batch is no
longer evidence that one command completed. Gameplay acknowledgements remain
explicit logical updates such as teleport corrections/acceptance; do not add a
generic request id solely to preserve the retired RPC shape.

## Nonblocking And Backpressure Invariants

### Drawable/frame thread

The frame may only:

- try-enqueue a command without waiting for physical write or free capacity;
- drain already-decoded ready updates;
- apply them in receive order under `RuntimeUpdatePumpBudget`; and
- flip resident dirty facts and feed bounded render work.

It must never perform or await socket IO, frame/chunk decode, a response,
worldgen, persistence, connect/reconnect, queue capacity, or mesh compilation.
Unlimited startup pumping relaxes only update-apply limits.

### Authoritative server thread

The server thread may drain ready commands, mutate authority, advance cadence,
route updates, and try-enqueue outbound publications. It must not wait for a
socket writer, a slow client, worldgen completion, or persistence IO. Worker
completion is polled at the normal publication gate.

### Queue policy

- Core commands and updates are reliable and ordered.
- Server outbound queues have count and byte bounds. Saturation marks the
  connection as a slow consumer and disconnects it without blocking authority.
- Client inbound queues are bounded/observable. Producer blocking or browser
  message backpressure is off-frame and preserves order; the runtime never
  waits for an update.
- Client outbound enqueue returns a visible full/disconnected result. A
  latest-wins slot is allowed only for a command whose protocol semantics
  explicitly permit supersession; reliable actions are never silently lost.
- No entity transform coalescing is introduced here. A future lane must define
  spawn/despawn, correction, and keyframe ordering first.

Exact bounds are selected from measured encoded bytes and churn tests. They
must be shared configuration/diagnostic facts, not different constants hidden
in flat, XR, Android, and web apps.

## Cross-Platform Conformance Model

One reusable semantic suite should drive a connection/host harness and run as
far as each environment permits against:

1. local integrated runner;
2. native TCP loopback;
3. native dedicated multi-client TCP;
4. browser integrated worker; and
5. browser remote WebSocket worker.

The suite proves:

- command ordering and update ordering;
- unsolicited updates while no command is pending;
- snapshot before later delta/unload coherence;
- idle-observer remote-player/entity publication;
- send does not drain or apply inbound updates;
- ready-only drain does not wait;
- budget exhaustion defers without loss or reordering;
- queue depth/bytes/oldest-age conservation;
- deterministic saturation/disconnect behavior; and
- disconnect/reconnect state is surfaced consistently.

Native XR, Android XR, and flat Android do not get copied protocol tests. They
prove they instantiate the same native adapter and add platform/device frame
accounting: zero socket/decode/response wait in the drawable frame and bounded
apply time. Browser tests additionally prove the production remote socket and
decode execute in the worker, not merely that the main-thread pump is
nonblocking.

## Implementation Slices

### Slice 0: Documentation And Characterization Baseline — Complete

Status: completed 2026-07-13.

Before the first runtime edit:

- preserve a focused source/call-graph receipt for the response sender,
  `wait_for_server_jobs`, per-command tick/save, client actor write/read loop,
  WebSocket loopback bridge, and response-era counters;
- capture current dedicated smoke behavior and current native/web build gates;
- add or identify a test clock/cadence seam so autonomous tests do not depend
  on flaky wall-clock sleeps; and
- record current frame/byte formats and the protocol version being replaced.

Exit: the exact compatibility surface and test migration order are recorded;
no production behavior changes.

Recorded baseline:

- The replaced protocol is version `19`. Native commands are one little-endian
  `u32` byte length plus one encoded command. A native update batch is a
  little-endian `u32` update count followed by one length-prefixed encoded
  update per entry. WebSocket commands use one binary message; WebSocket
  update batches use the same count plus per-update length/payload shape inside
  one binary message. No command id exists; batch position is the response
  pairing contract.
- `DedicatedNetworkEvent::Command` carries a `sync_channel(0)` sender. Its
  connection thread reads one command, waits for the authoritative loop to
  return one batch, writes that batch, and only then reads again.
- `DedicatedSession::handle_client_command` flushes movement, calls
  `wait_for_server_jobs`, advances `try_simulation_tick_report_for_player`,
  and calls `save_dirty_chunks`. The selected-player tick increments global
  simulation, inserts `TimeUpdate`, and drains only that target's routed queue.
- `NativeClientIoSession` has one actor that writes one command then blocks on
  its paired batch. Browser remote mirrors the same dependency through
  `pending_response_batches` and response-sequence waiters. The WebSocket
  server is a one-client/one-upstream-TCP lockstep bridge.
- `SimulationCadence::advance_host_frame` and `advance_elapsed` are already
  deterministic and directly tested. Live `NativeIntegratedServerRunner`
  pacing and the dedicated loop use `Instant`/channel waits directly. Slice 3
  will isolate a small host-step/clock seam instead of putting sleeps in core
  cadence assertions.

Baseline validation on macOS native flat/browser:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server -p
  mclone-net -p mclone-app-runtime -p mclone-dedicated-server`: passed (386
  server, 16 net, 243 app-runtime, 20 dedicated tests plus integration/doc
  tests).
- `pnpm --silent native:dedicated:smoke`: passed the two-client command-driven
  fixture and recorded the expected per-command batches/ticks.
- `pnpm --silent native:web:remote-smoke`: passed in local Chromium with 49
  loaded chunks, 600 applied updates, worker worldgen/light mailboxes, and the
  current diagnostic equality `requestFrames=46`, `responseFrames=46`.
- `pnpm --silent native:remote:smoke`: both clients connected and rendered 166
  sections, one remote player, two entities, and three actors, but the wrapper
  failed after capture because its report parser no longer accepts the current
  screenshot summary. Both `/tmp` images were inspected and show valid remote
  terrain/actors. Repair that pre-existing harness drift when Slice 3 adds
  unsolicited-publication assertions.
- Quest/device evidence is unavailable for this tactical run. Native flat,
  offscreen, AVD, local Chromium/Playwright, and synthetic stereo are the
  accepted implementation lanes; real Quest scheduling/frame evidence remains
  a named deferred receipt.

### Slice 1: Target-Neutral Simulation And Per-Player Publication — Complete

Status: completed 2026-07-13.

Refactor `mclone-server` without changing the live wire:

- separate one global simulation/cadence advance from draining any selected
  player's update queue;
- route worldgen, block/fluid, entity, and remote-player results to all
  eligible player queues during that one advance;
- expose a cheap ordered drain for a specific player that does not tick or
  repoll global scheduler work;
- make periodic time publication a routed broadcast at the vanilla 20-tick
  period rather than a selected-target update every tick;
- preserve direct command-result ordering by routing semantic command updates
  into the issuing player's stream; and
- preserve local integrated behavior through a local-target adapter over the
  same global primitive.

Tests must prove two players observe one global tick, draining player A does
not consume B, and repeated drains do not advance time. Existing integrated
runner tests remain green.

Exit: simulation advancement is not structurally parameterized by the player
whose updates happen to be returned.

Implemented result:

- `IntegratedServer` now exposes global scheduler and gameplay advances that
  route publications without draining a selected player. The local integrated
  and dedicated-player compatibility methods are adapters that drain only
  after the shared advance.
- Worldgen, block/fluid, entity, remote-player, initial-spawn, and physics
  results remain in ordered per-player queues. A command-enqueue entry point
  preserves command-result ordering for the autonomous host cutover.
- Dedicated joins receive the current time, and live time publication is a
  routed broadcast on tick 1 and each 20-tick boundary instead of an update
  attached to every selected-player simulation call.
- Ready-only local and dedicated drains do not poll jobs or advance scheduler,
  gameplay, physics, or day time.

Validation evidence:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server`: passed 388
  server tests, including two-player single-tick routing, independent drains,
  repeated side-effect-free drains, and 20-tick time publication.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p
  mclone-dedicated-server`: app-runtime passed 243 unit tests plus integration
  locks; dedicated passed 20 tests after updating the response-era time
  assertion to the new publication period.
- The protocol remains version `19` and the live dedicated connection remains
  command/response shaped; Slice 2 changes only that transport foundation.

### Slice 2: Native Duplex Transport Foundation — Complete

Status: completed 2026-07-13.

Make TCP capable of push before switching authority:

- split native client TCP writer and reader ownership around ordered bounded
  queues; the reader continuously accepts update publication frames;
- split dedicated connection read and write ownership; replace the per-command
  zero-capacity response sender with a connection-owned outbound sender;
- keep the current command-driven server capable of enqueueing one publication
  batch per command during this compatibility checkpoint;
- attach encoded length, queue age, read/decode time, and monotonic inbound
  frame sequence to queued updates; and
- add held-reader/held-writer tests proving command enqueue and update receipt
  progress independently.

Do not alter the client runtime pump. Do not rename all compatibility APIs in
this slice; first prove the new producer mechanics with the existing smokes.

Exit: native client and server can send in both directions independently, even
though the authoritative loop is not autonomous yet.

Implemented result:

- `NativeClientIoSession` now owns independent TCP writer and continuous
  reader threads over bounded ordered command and update-batch queues. Command
  enqueue uses a nonblocking bounded send, and inbound batches retain encoded
  lengths, queue age, read/decode time, and a monotonic publication-frame
  sequence under the temporary `response_sequence` compatibility name.
- Each dedicated connection now owns an independent reader and writer plus a
  bounded outbound publication sender. `DedicatedNetworkEvent::Command` no
  longer carries a zero-capacity response channel; the command-driven host
  enqueues its compatibility batch through the connection writer.
- Dedicated hosts explicitly disable the integrated local-player slot. This
  removes a ghost observer and prevents autonomous broadcasts from
  accumulating in an update queue that no client can drain.
- The shared app-runtime pump and the protocol version/bytes remain unchanged.

Validation evidence:

- `cargo test --manifest-path native/Cargo.toml -p mclone-net`: passed 17
  tests. Held-publication coverage proves a second command reaches the server;
  unsolicited-publication coverage proves an idle client reader receives a
  frame without sending a command.
- `cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server`:
  passed 21 tests, including a dedicated connection that accepts two commands
  before any paired publication and pushes an update before any command.
- `cargo test --manifest-path native/Cargo.toml -p mclone-server -p
  mclone-app-runtime`: passed 388 server and 243 app-runtime unit tests plus
  their integration/doc tests.
- `pnpm --silent native:dedicated:smoke`: passed the two-client fixture over
  the duplex transport. One response-era outbound-depth receipt now records a
  periodic cross-player publication waiting for the Slice 3 all-player drain.
- `pnpm --silent native:remote:smoke`: both native clients connected and
  rendered 166 sections, one remote player, two entities, and three actors.
  Both `/tmp` captures were inspected and are valid; the wrapper still exits
  after capture on the pre-existing screenshot-summary parser drift recorded
  in Slice 0.

### Slice 3: Autonomous Dedicated Host Cutover

Switch the dedicated authoritative loop as one behavior change:

- run the shared validated cadence at the normal 20/20/60 defaults;
- receive connection events until the next host boundary, then drain all ready
  commands in receive order;
- flush each session's buffered movement and mark one movement accounting tick
  boundary;
- advance global gameplay/physics once according to cadence;
- drain and publish ordered updates for every connected player;
- delete `wait_for_server_jobs`; snapshots publish when worker completion
  reaches the shared scheduler gate;
- replace per-command save with 6000-gameplay-tick autosave plus explicit
  shutdown flush; and
- ensure disconnect/removal routes final observer updates without another
  client command.

Use a deterministic clock seam for core cadence tests. Add wall-clock smoke
coverage only after deterministic tests establish the state machine.

Required proofs:

- world time advances with zero clients;
- zero-command connected clients receive periodic time publication;
- aggregate command rate from one/many clients does not change tick rate;
- `SetChunkView` command handling returns quickly and its snapshots arrive
  later without a polling command;
- an idle observer receives another player's movement/entity updates; and
- worldgen and persistence workers cannot stall the authority loop.

Exit: remote dedicated gameplay no longer has any command-as-clock or
command-as-poll dependency.

### Slice 4: Native Consumer Convergence And Response Cleanup

After the push cutover is proven:

- remove `pending_response_batches` from native runtime adapters;
- rename `response_sequence` to transport-neutral `inbound_frame_sequence` and
  replace response count metrics with frame/update queue facts;
- retire normal-path `drain_command_updates`/paired-response names while
  retaining only clearly quarantined test helpers if still useful;
- prove desktop flat, desktop XR, flat Android, and Android XR assemble the
  same `NativeClientIoSession`/`ClientConnection` implementation; and
- add source/purity gates preventing app-local socket, response, or update-pump
  policy from returning.

Exit: native platform differences are address/lifecycle/presentation wiring,
not networking semantics.

### Slice 5: Native WebSocket Host And Web Worker Remote

Bring web to the same production contract:

- attach WebSocket sessions directly to the shared dedicated connection/host
  registry; retire the one-WebSocket-to-one-loopback-TCP bridge;
- make browser production remote create a worker that owns the WebSocket,
  command encode/send, update receipt, and update-frame decode;
- hand decoded envelopes or transferable decoded buffers to the main runtime
  through the existing `ClientConnection` semantics;
- keep the inline integrated server and any main-thread WebSocket helper
  explicitly named as compatibility/smoke fallbacks; and
- preserve worker lifecycle, disconnect, and queue-pressure diagnostics across
  browser background/foreground transitions.

Transferable buffers are acceptable for the first cut. Do not wait for the
future shared-memory worker substrate, but do not expose a second runtime
interface that would block adopting `SharedArrayBuffer` later.

Exit: browser remote receives autonomous updates without a command and
production receipt/decode is worker-owned.

### Slice 6: Shared Conformance, Pressure, And Frame Evidence

Promote common tests and diagnostics after every production adapter exists:

- implement the cross-platform semantic cases listed above through reusable
  fixtures rather than copied app assertions;
- expose outbound/inbound depth and bytes, oldest inbound age, frames/updates
  received and applied, producer read/decode time, frame drain/apply time,
  deferred/stall counts, sequence numbers, and slow-consumer disconnects;
- run sustained chunk-view movement and multi-client idle-observer smokes;
- deliberately hold a client writer/reader to prove bounded memory and
  deterministic disconnect without authority-frame delay; and
- capture desktop, XR/Quest, and browser frame accounting showing no producer
  IO/decode/response wait charged to drawable work.

Exit: behavior and pressure are comparable across adapters through one schema;
no platform is accepted solely because it appears visually functional.

### Slice 7: Compatibility Removal And Documentation Closeout

- remove the retired response frame terminology and unused compatibility
  helpers from protocol/net/app-runtime/web/dedicated code;
- bump/update `docs/protocol.md`, hosting/platform docs, and smoke help text to
  describe publication frames and autonomous cadence;
- update the multiplayer topic's current-state evidence and next priority;
- mark Tactical 133's bus/pacing work complete or narrow its remaining terrain
  follow-up explicitly; and
- record exact commits, validation commands, queue bounds, protocol version,
  and known measured limitations here.

Exit: source and docs have one current push model; a future session lifecycle
slice can add profile/login/keepalive without first removing RPC scaffolding.

## Validation Matrix

Focused native gates evolve with the implementation but should include:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server
pnpm native:thin-adapters:purity
git diff --check
```

Run the existing dedicated multi-client and remote client/offscreen smokes at
each externally visible cutover. Use the current commands from
[`../platforms.md`](../platforms.md#validation-policy) rather than copying a
stale platform matrix into this tactical.

Web slices additionally require:

```bash
pnpm native:web:build
```

and the existing local-worker plus remote-WebSocket browser smoke lanes,
extended to assert unsolicited publication and worker ownership.

Final native device validation batches desktop XR/Quest after desktop TCP and
headset-free tests pass. Because this tactical changes data flow rather than
pixels, screenshot capture is not intrinsically required; if a validation lane
changes rendered behavior while proving streaming, follow the repository's
pixel-producing screenshot rule and inspect captures under `/tmp`.

## Stop Lines

This tactical does not authorize:

- login/auth/profile identity or player-record persistence;
- keepalive, latency display, or explicit disconnect protocol messages beyond
  the transport errors needed for bounded pressure;
- movement anti-cheat or sequenced input replay;
- compression, varint conversion, QUIC, WebRTC, or WebTransport;
- entity transform coalescing/priority lanes;
- arbitrary gameplay-rate changes or publication-lane product controls; or
- platform-local networking implementations.

Those remain later phases of the multiplayer topic. If the push conversion
uncovers a prerequisite in one of them, record the narrow dependency instead
of absorbing the adjacent feature.

## Completion Criteria

The tactical is complete only when all are true:

- dedicated simulation cadence is autonomous with zero/one/many clients;
- native TCP and production web remote receive ordered unsolicited updates;
- chunk/worldgen/entity/time publication has no polling-command dependency;
- authority, native/web producer work, and frame apply have bounded observable
  queues and never block the drawable thread;
- desktop flat/XR/Android use one native adapter and web uses the same semantic
  contract through worker-owned mechanics;
- one conformance suite proves ordering, pressure, disconnect, and budgeted
  deferral across the supported adapter families;
- response-pair compatibility state is gone from normal paths; and
- the living topic, architecture, protocol, hosting, and platform docs reflect
  the landed behavior and evidence.
