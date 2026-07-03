# Session Network Architecture

Status: target architecture; Slice 1 send-only high-frequency command policy
and Slice 2 local integrated ordered update pump landed; Slice 3A batched dirty
intent and resident cache lookup landed; Slice 3B local integrated producer-side
decoded update queue landed; Slice 3C local integrated chunk-interest unload
hysteresis landed; revised 2026-07-03 to adopt the vanilla ordered-stream
update model (thin apply plus a frame-budget stall) and drop the earlier
priority-class design; remaining implementation tracked in
[`tactical/133-session-network-bus-and-update-pacing.md`](./tactical/133-session-network-bus-and-update-pacing.md)

## Purpose

Mclone should have one client/session topology across local integrated play,
native remote dedicated play, Android XR, flat Android, and web/WASM. The
logical protocol stays shared, but command sending and server-update
application must stop looking like a synchronous request/response helper inside
the render frame.

The immediate performance trigger is the Quest render-distance-7 frame pacing
work: high-frequency XR pose sync currently calls a helper that sends a
movement command and then opportunistically drains/applies all pending
integrated-server updates. When the pending updates are chunk snapshots and
unloads, a cheap pose command can turn into multi-ms chunk dirty marking on the
render frame.

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

Compared to the reference shape, local integrated play still has one costly
gap:

- Slice 3A now batches server-update dirty intent and uses a resident
  chunk-to-section-key cache index, so duplicate updates no longer multiply
  neighborhood fanout or full-cache scans. Render dirty state is still
  ordered-set backed, where vanilla flips a boolean on a resident
  render-section slot.

Slice 3B moved local integrated update decode/conversion to the runner side:
`try_recv_update` now returns already-decoded `ServerUpdate` envelopes with
encoded byte metadata, so the local runtime pump no longer decodes update
payloads.

Slice 3C added local integrated source pacing at the shared player-chunk
tracking boundary. Dedicated/default views remain exact, while local
integrated Java-shaped views keep a one-chunk unload hysteresis margin so
boundary oscillation can retain the trailing edge instead of immediately
unloading and reloading it.

Native remote dedicated is even more request/response-shaped today:
`NativeClientSession::send_command` writes one command and blocks reading one
server-update batch on the caller thread.

Web remote uses browser WebSocket callbacks underneath, but the Rust-facing
session still presents command exchange as "send frame, await response." Web
worker integrated play is closer to the desired queue shape, but its async
mechanics are still app-local instead of a shared session bus contract.

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

Native should start with a dedicated blocking-IO thread for TCP rather than
forcing async through the engine. Web should use browser WebSocket/WebRTC
callbacks or a worker to feed the same inbound queue. The runtime stays
frame-driven on all platforms.

## Shared Boundary

Exact names can change, but the shared contract should look like this:

```text
trait ClientSessionBus {
  fn enqueue_command(&mut self, command: ClientCommand, policy: CommandQueuePolicy) -> Result<()>;
  fn drain_updates(&mut self, budget: UpdateDrainBudget) -> Result<SessionUpdateDrain>;
  fn diagnostics(&self) -> SessionBusDiagnostics;
}
```

The bus owns transport/session mechanics and hands back decoded updates in
receive order. The app/runtime owns when drained updates are applied and how
much frame budget they may consume.

Local integrated, native TCP, WebSocket, WebRTC, and worker channels should all
fit behind this boundary. Platform adapters may be async internally; the shared
runtime should not become async just because one backend is.

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

- Move `TcpStream` ownership to a session/network thread; that thread owns
  payload decode.
- The app thread enqueues `ClientCommand` frames and drains decoded
  `ServerUpdate`s from queues.
- The first implementation can keep the current wire codec and blocking socket
  reads in the IO thread.
- Server-push wire changes can come later; the app/runtime boundary should be
  ready before the wire protocol is broadened.

Android XR and flat Android should consume the same native bus boundary. The
Android app crates should own platform lifecycle and launch arguments, not
private transport semantics.

## Web Implementation Shape

Web should converge on the same bus semantics while preserving browser-owned
async mechanics:

- WebSocket `message` callbacks or WebRTC data-channel callbacks push decoded
  updates onto the same inbound queue in receive order.
- A worker-integrated server publishes updates through the same logical queue.
- The Rust runtime drains updates on its normal frame/update cadence.
- Hot paths can later use `SharedArrayBuffer`/atomics, but the policy remains
  the same as native: command enqueue and update drain are separate operations.

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
