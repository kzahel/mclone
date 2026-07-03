# Session Network Architecture

Status: target architecture; Slice 1 send-only high-frequency command policy
landed for local integrated play; remaining implementation tracked in
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
- The client receives packets on Netty or local-channel IO threads.
- `PacketUtils.ensureRunningOnSameThread(...)` schedules packet handlers onto
  the Minecraft client thread.
- `ClientPacketListener.handleLevelChunk(...)` installs chunk data and marks
  render sections dirty on the client thread.
- `ChunkRenderDispatcher` then rebuilds dirty chunks asynchronously and queues
  uploads back to the render side.

The important lesson is not "chunk updates never touch the main thread." They
do. The lesson is that a movement packet send does not synchronously flush
inbound chunk streaming work.

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
drains the update queue, and applies every drained update before returning.

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
  - drains inbound updates under priority and frame budget
  - applies client replica updates
  - marks render dirty incrementally
  - renders from already-paced state

session/network actor
  - owns connection or local runner bridge
  - writes outbound command frames
  - reads inbound update frames
  - decodes frames and classifies update priority
  - pushes updates to inbound queues
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

The bus owns transport/session mechanics. The app/runtime owns when drained
updates are applied and how much frame budget they may consume.

Local integrated, native TCP, WebSocket, WebRTC, and worker channels should all
fit behind this boundary. Platform adapters may be async internally; the shared
runtime should not become async just because one backend is.

## Priority And Backpressure

Inbound updates need explicit priority because not every update has the same
frame-time or correctness consequence.

| Class | Examples | Apply policy |
| --- | --- | --- |
| Critical ordered | disconnect/errors, session lifecycle, `PlayerPosition` correction, teleport acknowledgement state | Apply promptly before lower-priority work. Do not delay behind chunk streaming. |
| Gameplay ordered | inventory/container state, carried item, interaction results, local block action confirmations | Preserve ordering for gameplay semantics. Small batches can usually apply in the same frame. |
| Realtime superseding | remote-player/entity transform snapshots, presentation-only pose updates | Coalesce by entity/player when newer state replaces older state. Do not let stale transforms grow an unbounded queue. |
| World streaming paced | `ChunkSnapshot`, `ChunkUnload`, broad chunk light/content payloads | Apply under an explicit per-frame budget. Maintain per-chunk ordering and avoid starving nearby visible chunks. |
| Section/block mutation | `SectionBlockUpdates`, local edit echo, light deltas | Ordered per affected section/chunk. Local-player-visible edits may deserve a higher coherence lane than background streaming. |

Outbound commands need similar policy, but with care:

- Java-like movement packets are ordered command records; the current vanilla
  movement path should not accidentally flush inbound updates.
- Future FPS-style predicted movement may require preserving every fixed-step
  command for replay. Do not blindly collapse that lane into "latest input."
- Some presentation or view-center commands can be latest-wins if the command's
  semantics explicitly allow it.
- Interactions, inventory changes, block actions, and teleport accepts remain
  reliable ordered commands.

Backpressure should be visible in diagnostics:

- outbound queue depth by command class,
- inbound queue depth by update class,
- bytes queued,
- applied updates by class per frame,
- deferred chunk snapshots/unloads,
- coalesced realtime updates,
- time spent in drain, dirty marking, client apply, and render dirty handoff.

## Native Implementation Shape

Local integrated:

- Keep `NativeIntegratedServerRunner` as the authoritative server actor.
- Keep command and update channels.
- Remove update draining from high-frequency command send helpers.
- Let the runtime update pump drain/apply inbound updates with budget and
  priority.

Native remote TCP:

- Move `TcpStream` ownership to a session/network thread.
- The app thread enqueues `ClientCommand` frames and drains decoded
  `ServerUpdate` frames from queues.
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

- WebSocket `message` callbacks or WebRTC data-channel callbacks push binary
  frames into an inbound queue.
- A worker-integrated server publishes updates through the same logical queue.
- The Rust runtime drains updates on its normal frame/update cadence.
- Hot paths can later use `SharedArrayBuffer`/atomics, but the policy remains
  the same as native: command enqueue and update drain are separate operations.

This avoids a web-only engine architecture while still allowing browser APIs to
stay event-driven.

## Invariants

- Renderer-facing code never calls the authoritative server directly.
- Movement/pose command send never performs unbounded inbound update apply.
- Client replica mutation and render dirty marking happen at explicit update
  pump points, with priority and budget.
- Local integrated and remote dedicated share the same client update
  application path.
- Web and native may differ in transport mechanics, not in command/update
  semantics.
- Chunk streaming cannot starve corrections, lifecycle updates, or interaction
  feedback.
- Old render output remains visible until replacement compile/upload work is
  complete; network pacing should feed, not bypass, the terrain lifecycle.

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
  owns the dirty-to-drawable terrain pipeline.
- [`tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md`](./tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md)
  records the measured Quest RD7 command/update tail that triggered this work.
