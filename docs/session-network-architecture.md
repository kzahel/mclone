# Session Network Architecture

Status: target architecture; Slice 1 send-only high-frequency command policy
and Slice 2 local integrated ordered update pump landed; Slice 3A batched dirty
intent and resident cache lookup landed; Slice 3B local integrated producer-side
decoded update queue landed; Slice 3C local integrated chunk-interest unload
hysteresis landed; Slice 3D resident cached-section dirty flags landed; revised
2026-07-03 to adopt the vanilla ordered-stream update model (thin apply plus a
frame-budget stall) and drop the earlier priority-class design; native remote
`SendOnly` and TCP-readiness fixes landed 2026-07-06; shared `ClientConnection`
plus focused remote inbound queue work completed 2026-07-07 under
[`tactical/151-remote-inbound-update-pipeline.md`](./tactical/151-remote-inbound-update-pipeline.md),
with autonomous dedicated tick, full-duplex server push, bounded connection
queues, and production web-worker transport now planned under
[`tactical/176-dedicated-autonomous-push-runtime.md`](./tactical/176-dedicated-autonomous-push-runtime.md)

## Purpose

Mclone should have one client/session topology across local integrated play,
native remote dedicated play, Android XR, flat Android, and web/WASM. The
logical protocol stays shared, but command sending and server-update
application must stop looking like a synchronous request/response helper inside
the render frame.

The original performance trigger was the Quest render-distance-7 frame pacing
work: high-frequency XR pose sync called a helper that sent a movement command
and then opportunistically drained/applied all pending integrated-server
updates. The shared update pump has removed that local and remote client-frame
coupling. The remaining architectural gap is producer-side: remote transports
still pair one update batch with every command, the dedicated world advances
once per received command, and production web remote does not yet prove
worker-owned socket/decode work.

This doc records the desired state so implementation slices do not drift into
one-off caps or platform-specific network paths.

## Reference Shape

Minecraft Java 1.17.1 separates command send from inbound packet handling:

- The local player sends movement packets through `Connection.send(...)`.
- The client receives packets on Netty or local-channel IO threads, and packet
  payloads are decoded there, not on the client thread.
- `PacketUtils.ensureRunningOnSameThread(...)` schedules packet handlers onto
  the Minecraft client thread.
- The client thread drains **all** queued packet handler tasks every render
  frame (`Minecraft.runTick` -> `runAllTasks()`), in receive order. There are
  no priority classes and no apply budget anywhere in the client.
- `ClientPacketListener.handleLevelChunk(...)` installs chunk data and marks
  render sections dirty on the client thread.
- `ChunkRenderDispatcher` then rebuilds dirty chunks asynchronously and queues
  uploads back to the render side.

The important lesson is not "chunk updates never touch the main thread." They
do, every frame, unbudgeted. The lesson is where vanilla lets pressure
accumulate when the server outruns the client:

- **Packet apply is thin.** `handleLevelChunk` parses the payload into a
  `LevelChunk` and calls `setSectionDirtyWithNeighbors`, which bottoms out in
  `ViewArea.setDirty` — an array index into a fixed resident grid of
  `RenderChunk` slots followed by a boolean store. No ordered-set churn, no
  allocation, no meshing, no upload.
- **Expensive work is pull-based with hard structural backpressure.** Mesh
  rebuilds are gated by a fixed pool of `ChunkBufferBuilderPack`s; when no
  builder is free, nothing is scheduled and the chunk simply stays dirty. The
  render traversal chooses which dirty chunks to compile (visible, nearest
  first), and the upload queue is bounded by the same pool.
- **The buffer between them is world state.** Chunks the client cannot mesh
  yet accumulate as installed chunk data plus dirty flags — passive memory
  bounded by view distance, not queued work that anything rescans per frame.
- **Arrival is smeared at the source.** Chunks are only sent when they finish
  the server generation/lighting pipeline, which is itself throttled (for
  example `ThreadedLevelLightEngine` processes a small task batch per tick),
  so packets arrive spread across ticks instead of as one dump.

Context from outside our reference target: even with this shape, desktop
vanilla of the 1.17 era can hitch during chunk storms, and Mojang's eventual
fix (1.20.2) was **server-side** chunk-send batching paced by client
acknowledgements — pacing at the source, not client-side reordering. We do not
port 1.20.2 systems, but the direction confirms where flow control belongs.

## Current Mclone Shape

Local integrated native already has most of the physical pieces:

```text
app/render thread
  -> command mpsc
integrated server thread
  -> update mpsc
app/render thread
  -> apply ServerUpdate
  -> mark render dirty
  -> feed render compile/upload pipeline
```

The bad coupling is API-level: `send_gameplay_command_timed` sends a command,
drains the update queue, and applies every drained update before returning
(fixed for pose sync by the Slice 1 `SendOnly` policy).

Slice 2 removed the high-frequency and view-change drain coupling in local
integrated play: pose sync and chunk view changes enqueue commands, while
`LocalSingleViewSceneRuntime::poll` applies received updates in strict receive
order under an elapsed-time budget, with unlimited drain paths reserved for
startup and idle waits.

Compared to the reference shape, local integrated play now has the thin
resident dirty-marking path for cached render sections:

- Slice 3A batches server-update dirty intent and uses a resident
  chunk-to-section-key cache index, so duplicate updates no longer multiply
  neighborhood fanout or full-cache scans.
- Slice 3D moved cached resident dirty state onto render-section slots.
  Resident snapshot and section-block updates flip slot-local booleans rather
  than inserting resident keys into ordered dirty sets.
- Ordered dirty sets remain as fallback/coordination state for new snapshot
  chunks before replica install, removals, nonresident/stale sections, and
  inflight compile tracking. Broader dirty-to-drawable lifecycle ownership is
  tracked by `tactical/128`.

Slice 3B moved local integrated update decode/conversion to the runner side:
`try_recv_update` now returns already-decoded `ServerUpdate` envelopes with
encoded byte metadata, so the local runtime pump no longer decodes update
payloads.

Slice 3C added local integrated source pacing at the shared player-chunk
tracking boundary. Dedicated/default views remain exact, while local
integrated Java-shaped views keep a one-chunk unload hysteresis margin so
boundary oscillation can retain the trailing edge instead of immediately
unloading and reloading it.

Native remote dedicated now uses the shared client connection boundary. Desktop,
Android, and Android XR remote adapters hold `NativeClientIoSession`, whose IO
actor owns the TCP stream, writes queued commands, blocks on paired responses,
decodes response batches producer-side, and publishes ordered update batches to
the runtime-facing queue. The app/runtime thread drains that queue through
`pump_client_connection_updates_report` under `RuntimeUpdatePumpBudget`. The
wire protocol is still one response batch per command, so
`pending_response_batches` remains adapter bookkeeping for response pairing, but
normal frame polling no longer owns socket reads or ready-batch decode.

Server-side native dedicated is already threaded: an accept thread spawns a
connection thread per client, the connection thread reads commands and waits on
the authoritative server loop for the response, then writes one update batch.
Server-push broadening is still future work owned by tactical 133; it is not
required for the current shared client ingress.

Web integrated inline/worker play and web remote WebSocket play also implement
the shared `ClientConnection` path. Browser worker or WebSocket callbacks remain
web adapter mechanics, but normal-frame Rust runtime polling enqueues commands
separately from draining already decoded queued updates.

## Target Topology

The client-facing shape should be a bus, not a request/response session:

```text
client app/render/runtime thread
  - samples input and XR/flat poses
  - enqueues outbound commands
  - drains inbound updates in receive order under a frame budget
  - applies client replica updates
  - marks render dirty incrementally
  - renders from already-paced state

session/network actor
  - owns connection or local runner bridge
  - writes outbound command frames
  - reads inbound update frames
  - decodes frames into logical updates
  - pushes updates to the inbound queue in receive order
  - exposes diagnostics/backpressure

authoritative host
  - local integrated server thread, native dedicated server, browser worker
    host, or future P2P host
  - consumes commands
  - publishes authoritative updates

render compile/upload actors
  - consume render dirty work after update application
  - compile, accept, upload, and publish drawable terrain under their own
    budgets
```

Native uses independent blocking reader and writer ownership rather than
forcing async through the engine. Production web uses a Web Worker to own the
WebSocket plus update-frame decode and feeds the same logical inbound queue;
browser-main-thread callbacks and inline server paths remain explicit
smoke/fallback implementations. The runtime stays frame-driven on all
platforms.

## Shared Boundary

The current shared contract lives in `mclone-app-runtime/src/client_connection.rs`:

```text
trait ClientConnection {
  fn send_command_only(&mut self, command: ClientCommand) -> Result<()>;
  fn drain_next_update(
    &mut self,
    mode: ConnectionUpdateDrainMode,
  ) -> Result<ClientConnectionDrainResult>;
  fn pending_update_metrics(&mut self) -> Result<ClientConnectionQueueMetrics>;
}

pump_client_connection_updates_report(
  core: &mut SingleViewRuntime,
  connection: &mut impl ClientConnection,
  budget: RuntimeUpdatePumpBudget,
  drain_mode: ConnectionUpdateDrainMode,
) -> Result<RuntimeUpdatePumpReport>
```

`ClientConnection` is the shared client-facing analogue to Java's
`Connection`: it owns transport/session mechanics and hands back decoded
updates in receive order. Local integrated, native TCP, WebSocket, and future
worker/P2P transports are implementation details behind this one boundary.
The app/runtime owns when drained updates are applied and how much frame budget
they may consume.

Adding this boundary is only successful if it removes runtime duplication.
Local integrated and remote dedicated may keep different producer adapters, but
normal frame polling, `SendOnly`, `DrainImmediately`, update budget checks,
ordering, queue diagnostics, and conservation accounting should live in one
shared runtime path instead of parallel local/remote pump implementations.

This intentionally follows the reference engine at the architectural level:
Minecraft uses one `Connection` model for multiplayer sockets and integrated
singleplayer memory channels. Mclone should not keep separate runtime-facing
interfaces for local integrated and remote dedicated play. Platform adapters
may be async internally; the shared runtime should not become async just
because one backend is.

The shared contract is semantic rather than a promise that every producer has
the same physical threads. Native TCP uses OS reader/writer threads, native
integrated play uses the server runner and Rust channels, browser remote uses a
Web Worker and browser WebSocket events, and browser integrated play uses a
server worker. Every producer must nevertheless expose the same ordered
command stream, ordered decoded-update stream, bounded-pressure state, and
ready-only runtime drain.

This is a boundary match, not a Netty clone. Java integrated singleplayer uses
`LocalChannel` / `LocalServerChannel` and drains all queued packet handlers on
the client thread each frame. Mclone may keep local integrated as a runner
thread plus Rust channels, native remote as a blocking TCP IO actor, and web as
callbacks/workers, as long as those implementations converge on this ordered
`ClientConnection` command/update surface. The deliberate divergence remains
the budgeted drain described below.

## Parity And Divergence

The reference-porting policy requires recording where we follow the Java shape
and where we deliberately leave it.

Parity (the default):

- Outbound commands and inbound updates are separate streams; a movement send
  never flushes inbound work.
- Updates are applied on the client/runtime thread at an owned pump point.
- Updates are applied in strict receive order. There are no priority classes,
  no cross-update reordering, and no coalescing for local play. Per-chunk
  snapshot/mutation/unload ordering follows automatically.
- Update payload decode happens on the producer/IO side (session actor,
  integrated runner thread), not on the render thread.
- Update application is thin: install state, flip resident dirty slots. All
  heavy compile/upload work stays pull-based behind the bounded terrain
  pipeline (`tactical/128`).
- Player position corrections are ordinary ordered updates, applied in stream
  order.

Deliberate divergence (narrow, recorded):

- The update pump has a per-frame apply budget. When the budget is exhausted
  mid-queue, application stalls and resumes next frame. This changes only
  *when* updates apply, never their order or content. Justification: vanilla
  drains everything each frame and tolerates occasional chunk-storm hitches on
  desktop; a 13.9 ms Quest frame cannot. Setting the budget to unlimited
  restores exact vanilla semantics, which keeps the parity path open.
- Recorded trade: a stall delays everything behind it, including corrections.
  A 30-update burst at 4 updates per frame is ~7 frames (~100 ms at 72 Hz) of
  worst-case added latency where vanilla would apply same-frame. Queue age
  must therefore be visible in diagnostics. If measured correction latency
  ever becomes a real problem, the recorded escalation is a narrow critical
  fast-lane for session lifecycle/corrections only — adopted with evidence,
  never for chunk streaming, and never as a default local-play mechanism.

Parked, not planned:

- Priority classes, superseding lanes, and entity-transform coalescing may
  return for remote high-rate entity snapshots (tactical `133` Slice 6
  territory, likely alongside WebRTC-style transports). Any such lane must
  define spawn/despawn ordering rules before it coalesces anything. It is not
  a local-play mechanism.

## Frame-Thread Work Boundary

The drawable/frame thread is allowed to:

- enqueue a command without waiting for physical write or queue capacity;
- drain only already-decoded ready updates;
- apply updates in receive order under `RuntimeUpdatePumpBudget`; and
- hand resident dirty facts to the bounded compile/upload pipeline.

It must not perform socket reads or writes, block for a response or queue slot,
decode a transport frame or chunk snapshot, wait for worldgen or persistence,
run connect/reconnect work, or synchronously compile terrain. An unlimited
startup drain changes only the update-application budget; producer IO and
decode remain off-frame.

Commands need explicit pressure semantics. Reliable ordered interactions,
inventory actions, movement records, and teleport accepts cannot be dropped or
reordered. A high-frequency command may be latest-wins only when its logical
contract explicitly permits replacement, such as a superseded view center.
No command path may silently turn bounded queue pressure into a frame wait.

## Ordering, Pacing, And Backpressure

Inbound updates form one logical stream:

- Apply in receive order at the runtime pump, under the frame budget, always
  applying at least one pending update per pump so a stall cannot become
  livelock.
- When the server outruns the client, pressure accumulates first as undrained
  frames in the inbound queue (observable depth, age, bytes), then as
  installed world state waiting on the pull-based mesh pipeline. Neither grows
  without bound: the inbound queue is limited by interest management and
  source pacing, and world state is bounded by view distance.
- Pacing beyond the budget stall belongs at the source: integrated-server
  send-side smearing and interest hysteresis (unload margin wider than load
  margin, so a path oscillating near a chunk boundary does not thrash full
  row swaps).

Outbound commands stay ordered command records, with care:

- Java-like movement packets are ordered command records; the vanilla movement
  path must never flush inbound updates.
- Future FPS-style predicted movement may require preserving every fixed-step
  command for replay. Do not blindly collapse that lane into "latest input."
- Some presentation or view-center commands can be latest-wins if the command's
  semantics explicitly allow it.
- Interactions, inventory changes, block actions, and teleport accepts remain
  reliable ordered commands.

Backpressure should be visible in diagnostics:

- outbound queue depth,
- inbound queue depth, oldest queued update age, and bytes queued,
- updates applied versus deferred per pump, and stall occurrences,
- producer-side decode time,
- time spent in apply, dirty marking, client apply, and render dirty handoff.

The first full-duplex remote implementation uses bounded queues and disconnects
a persistently slow core-stream consumer instead of dropping reliable updates.
Exact bounds are measured implementation constants, not platform-owned policy.
Dropping or coalescing entity transforms remains future measured work and must
first define spawn/despawn and correction ordering.

## Native Implementation Shape

Local integrated:

- Keep `NativeIntegratedServerRunner` as the authoritative server actor.
- Keep command and update channels.
- Remove update draining from command send helpers (`SendOnly` for pose sync
  landed in Slice 1; `set_chunk_view` landed in Slice 2).
- The runtime update pump drains and applies inbound updates in receive order
  under the frame budget. Undrained updates stay in the channel so queue depth
  remains observable and idle/bootstrap checks keep working.
- Update payload decode/conversion happens on the runner side; the pump applies
  already-decoded local integrated updates.

Native remote TCP:

- `NativeClientIoSession` owns independent TCP writer and reader lanes. The
  writer consumes the ordered bounded command queue; the reader continuously
  receives and decodes unsolicited update frames into the inbound queue.
- The app thread enqueues `ClientCommand` frames and drains decoded
  `ServerUpdate`s from queues.
- The existing TCP update-batch framing may remain initially, but a batch is a
  publication frame rather than a command response. Empty response batches and
  response-count bookkeeping disappear.
- Desktop flat, desktop XR, flat Android, and Android XR instantiate this same
  adapter. Their app crates do not wrap it with private drain or polling
  semantics.

Native dedicated server:

- connection readers enqueue commands and connection writers consume bounded
  outbound publication queues;
- one authoritative server thread drains commands at tick boundaries,
  advances global simulation once, and routes per-player updates;
- it never waits for worldgen or socket IO, and autosave is cadence/shutdown
  owned rather than command owned; and
- TCP and WebSocket sessions terminate at the same host/session boundary. The
  current one-WebSocket-to-one-loopback-TCP bridge is not the target topology.

Android XR and flat Android should consume the same native bus boundary. The
Android app crates should own platform lifecycle and launch arguments, not
private transport semantics.

## Web Implementation Shape

Web converges on the same bus semantics while preserving browser-owned async
mechanics:

- Production remote WebSocket ownership and update-frame decode live in a Web
  Worker. Its message/event callbacks push decoded updates or transferable
  decoded buffers onto the same logical inbound queue in receive order.
- A worker-integrated server publishes updates through the same logical queue.
- The Rust runtime drains updates on its normal frame/update cadence through
  `ClientConnection`.
- `SharedArrayBuffer`/atomics can replace transferable-buffer handoff later,
  but the policy remains the same as native: command enqueue and update drain
  are separate operations.
- Main-thread WebSocket callbacks and inline integrated servers are named
  compatibility/smoke fallbacks, not production-quality completion evidence.

This avoids a web-only engine architecture while still allowing browser APIs to
stay event-driven.

## Invariants

- Renderer-facing code never calls the authoritative server directly.
- Command send never performs inbound update application, bounded or not.
- Server updates are applied only at the owned runtime pump, in receive order,
  under an explicit frame budget; a pump pass always applies at least one
  pending update so a stall cannot become livelock.
- The pump runs every frame in every client lane. Queue depth and oldest
  queued update age are diagnostics so starvation is detectable rather than
  assumed absent.
- Frame paths use ready-only drains and nonblocking command enqueue. No
  platform adapter may hide a socket read, decode, promise wait, response wait,
  reconnect, or bounded-queue wait behind the shared call.
- Update application stays thin: producer-side decode, resident-slot dirty
  marking. New per-update work at the pump (payload decode, allocation,
  ordered-set churn) is a regression smell.
- Client player simulation does not free-run against missing chunks. Vanilla
  gates `LocalPlayer.tick()` on the player's chunk being present; the same
  gate matters here once a budget stall can defer the player's chunk snapshot.
- Local integrated and remote dedicated share the same client update
  application path.
- Web and native may differ in transport mechanics, not in command/update
  semantics.
- Native flat/XR/Android lanes use one native remote adapter. Cross-platform
  semantic parity is proven by one connection conformance suite over
  integrated, TCP, and web-worker/WebSocket implementations; XR adds frame
  accounting rather than a forked transport contract.
- Old render output remains visible until replacement compile/upload work is
  complete; network pacing feeds, not bypasses, the terrain lifecycle.

## Related Docs

- [`protocol.md`](./protocol.md) owns the logical `ClientCommand` and
  `ServerUpdate` enums.
- [`multiplayer-hosting.md`](./multiplayer-hosting.md) owns hosting modes and
  transport carriers.
- [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md)
  owns host tick and scheduler rules.
- [`player-movement-netcode.md`](./player-movement-netcode.md) records movement
  command/replay constraints.
- [`tactical/062-shared-threading-topology.md`](./tactical/062-shared-threading-topology.md)
  tracks native thread / web worker convergence.
- [`tactical/085-web-host-mode-convergence.md`](./tactical/085-web-host-mode-convergence.md)
  tracks shared host-mode policy versus web async mechanics.
- [`tactical/128-terrain-render-pipeline-coordination.md`](./tactical/128-terrain-render-pipeline-coordination.md)
  owns the dirty-to-drawable terrain pipeline that provides the pull-based
  backpressure this model relies on.
- [`tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md`](./tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md)
  records the measured Quest RD7 command/update tail that triggered this work
  and the dirty-mark cost diagnosis behind the thin-apply requirement.
- [`tactical/133-session-network-bus-and-update-pacing.md`](./tactical/133-session-network-bus-and-update-pacing.md)
  records the shared bus implementation plan and the local integrated slices
  already landed.
- [`tactical/149-remote-contrast-accounting-honesty.md`](./tactical/149-remote-contrast-accounting-honesty.md)
  records the remote `SendOnly` and readiness evidence that exposed the
  remaining ready-batch drain.
- [`tactical/151-remote-inbound-update-pipeline.md`](./tactical/151-remote-inbound-update-pipeline.md)
  closed the focused native/web remote inbound queue implementation.
- [`tactical/154-client-ingress-adapter-cleanup.md`](./tactical/154-client-ingress-adapter-cleanup.md)
  tracks post-151 cleanup that quarantines compatibility/probe helpers without
  changing the shared client ingress behavior.
- [`tactical/176-dedicated-autonomous-push-runtime.md`](./tactical/176-dedicated-autonomous-push-runtime.md)
  owns the coordinated autonomous dedicated tick, full-duplex remote push,
  bounded queues, production web-worker transport, and cross-adapter
  conformance implementation.
