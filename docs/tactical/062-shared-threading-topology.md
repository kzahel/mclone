# 062: Shared Threading Topology

Status: in progress - native desktop/browser server runners, browser server
job workers, shared-memory runner/job frames, bounded shared-buffer pools, and
web shared-lane stress coverage landed

## Purpose

Make native desktop and browser/WASM converge on the same engine threading
topology:

- client/render/input work stays off the server tick path
- integrated singleplayer runs behind a server-runner boundary
- worldgen, lighting, and render-section compilation are worker jobs
- platform adapters choose native OS threads or browser Web Workers without
  changing the engine ownership model

This is now a correctness and pacing priority, not just an optimization. The web
version must not become a single-threaded reduced engine, and desktop should not
keep an inline integrated-server shortcut that diverges from both vanilla and
the target web architecture.

## Vanilla Reference Shape

Minecraft Java 1.17.1 singleplayer starts an integrated server on a separate
server thread:

- `Minecraft.doLoadLevel(...)` starts `new IntegratedServer(...)` through
  `MinecraftServer.spin(...)`.
- `MinecraftServer.spin(...)` creates a thread named `Server thread`, passes it
  into the server constructor, and starts `runServer()`.
- `MinecraftServer.runServer()` owns the server tick loop and advances
  `tickServer(...)` on the server thread.
- After the integrated server reports ready, the client connects through a
  local Netty memory channel:
  - `ServerConnectionListener.startMemoryChannel()`
  - `Connection.connectToLocalServer(...)`

Lighting is also explicitly threaded in vanilla's chunk pipeline:

- `ChunkMap` creates processor mailboxes for `worldgen` and `light`.
- `ThreadedLevelLightEngine` receives a light mailbox and a priority sorter
  mailbox.
- Light work is queued as `PRE_UPDATE` and `POST_UPDATE` tasks; direct calls to
  `runUpdates(...)` and `onBlockEmissionIncrease(...)` on the wrapper panic
  because the work is expected to run through the threaded path.
- `lightChunk(...)` returns a `CompletableFuture` that completes after queued
  light tasks run and the chunk is marked light-correct.

We do not need to clone Netty or Java's executor classes literally, but the
topology matters:

```text
client/game/render side
  -> local memory transport
  -> integrated server thread
      -> server tick loop
      -> priority scheduled chunk/world jobs
      -> threaded worldgen/light executors
```

## Current Mclone Shape

### Native Desktop

Desktop is only partially aligned:

- `WindowSceneRuntime` owns `Option<IntegratedServer>` directly.
- Local integrated commands go through `LocalTransport`, but command handling
  and server ticking are still called from `WindowSceneRuntime::poll()`.
- Server tick orchestration, scheduler polling, block ticks, fluid ticks, entity
  ticks, and update application can therefore consume app/frame time.
- Native worker threads already exist for:
  - worldgen feature jobs (`mclone-worldgen`)
  - light status jobs (`mclone-light-status`)
  - render-section compilation
- Native dedicated/remote TCP exists separately and is closer to the desired
  transport boundary than integrated singleplayer.

### Browser/WASM

Browser/WASM is further from the target server topology:

- `WebRuntime::local_integrated(...)` owns an in-process `IntegratedServer`.
- Gameplay commands and server updates use `LocalTransport` queues with protocol
  encode/decode roundtrips, but no socket, WebSocket, WebRTC, or server worker
  is involved.
- Render-section compilation uses a browser `Worker` and transferred packed
  section payloads.
- Threading smokes validate COOP/COEP, `SharedArrayBuffer`, shared Wasm memory,
  `Atomics`, and a module worker.
- Server worldgen and light mailboxes compile for WASM as inline backends.

That means browser rendering is meaningfully worker-backed today, but browser
integrated server work is not.

## Target Topology

The engine should present one logical topology with platform-specific execution
backends:

```text
desktop client adapter
  -> winit input/render/presentation
  -> native integrated transport or remote TCP transport
  -> shared client/runtime/render session

web client adapter
  -> DOM/canvas/input/fetch/WebGPU presentation
  -> web integrated transport or WebSocket/WebRTC transport
  -> shared client/runtime/render session

integrated server runner
  native: OS thread
  web: Web Worker
  owns IntegratedServer and the 20 TPS server loop
  receives ClientCommand frames
  publishes ServerUpdate frames
  owns server diagnostics and shutdown/readiness

server job workers
  native: OS threads or native worker pool
  web: Web Workers using shared Wasm memory/Atomics where practical
  worldgen feature jobs
  light status / level-light jobs
  future save/compression jobs

render compile workers
  native: OS thread or pool behind RenderSectionCompiler
  web: Web Worker compiler behind the same request/result lifecycle
```

The transport can be local, TCP, WebSocket, or WebRTC, but the client-facing
shape should stay the same: commands are sent to a host, updates are drained or
pushed back, and the client replica applies updates through shared engine
helpers.

## Boundary Rules

- Do not let client frame/render code call `IntegratedServer::tick`,
  `try_simulation_tick_report`, `try_poll`, or command handlers directly in the
  default desktop or web app paths.
- Inline server execution is allowed only as an explicit test/smoke fallback
  behind the same runner interface.
- Native and web should expose the same runner/job counters even when the
  backend mechanism differs.
- Browser workers are part of the target architecture. `wasm32-unknown-unknown`
  remains the browser target, but it is not permission to remove worker
  topology.
- Lighting jobs must remain off the client/render frame path. Native currently
  has a light-status worker; web needs an equivalent worker-backed mailbox.
- GPU upload and presentation stay client/render-adapter owned. Workers produce
  CPU-side command/update/job payloads, not direct WebGPU surface operations.
- Remote browser transport is a later transport backend, not a reason to delay
  the integrated server-runner boundary.

## Proposed Shared Interfaces

Exact names can change during implementation.

### Server Runner

```text
trait IntegratedServerRunner {
  fn kind(&self) -> ServerRunnerKind;
  fn send_command(&mut self, command: ClientCommand) -> Result<()>;
  fn drain_updates(&mut self) -> Result<Vec<ServerUpdate>>;
  fn poll_diagnostics(&mut self) -> ServerRunnerDiagnostics;
  fn request_shutdown(&mut self);
  fn join_shutdown(&mut self) -> Result<()>;
}

enum ServerRunnerKind {
  InlineFallback,
  NativeThread,
  WebWorker,
}
```

The runner owns:

- `IntegratedServer`
- startup/readiness
- server tick cadence
- command ingress
- update egress
- server diagnostics
- shutdown and panic/error propagation

The client/runtime adapter owns:

- client replica
- render-session dirtying
- GPU upload/presentation
- input
- selected transport backend

### Local Integrated Transport

The local integrated transport should become a real channel boundary:

```text
ClientCommand -> encoded frame -> runner inbox
runner updates -> encoded frames -> client update queue
```

Native can start with `std::sync::mpsc` or crossbeam-style channels if already
available. Browser can start with `postMessage` and transferred `Uint8Array`
frames, then move hot paths to `SharedArrayBuffer` ring buffers once the worker
runner is stable.

Protocol encode/decode should remain in the local path so local integrated,
remote TCP, WebSocket, and WebRTC all exercise the same command/update wire
shape.

### Server Job Workers

Worldgen and lighting should sit behind backend-neutral mailboxes:

```text
trait ServerJobMailbox<Request, Response> {
  fn kind(&self) -> ServerJobMailboxKind;
  fn submit(&mut self, request: Request) -> Result<()>;
  fn drain_completed(&mut self) -> Result<Vec<Response>>;
  fn pending_count(&self) -> usize;
}

enum ServerJobMailboxKind {
  InlineFallback,
  NativeThread,
  WebWorker,
}
```

Native already has thread-backed worldgen and light-status mailboxes. WASM needs
worker-backed equivalents so worldgen/lighting do not run inline on the browser
main thread or the server worker's tick lane.

## Implementation Sequence

### 1. Native Integrated Server Runner

Move desktop local integrated mode behind a native server runner thread.

Acceptance:

- `WindowSceneRuntime` no longer owns `IntegratedServer` directly.
- Desktop local integrated commands are sent to a runner thread.
- The runner owns the 20 TPS tick loop and publishes updates independently of
  the render frame loop.
- Existing native remote TCP behavior is unchanged.
- Existing native worldgen, light-status, and render compile workers remain
  worker-backed.
- Desktop debug/runtime stats expose runner kind, server tick timing, pending
  server jobs, pending publications, command queue depth, and update queue
  depth.
- Movement smoke, timedemo smoke, native screenshot validation, and relevant
  unit tests stay green.

Landed in the first native slice:

- `mclone-server` now exposes `IntegratedServerRunner`,
  `ServerRunnerKind`, runner diagnostics, and a native
  `NativeIntegratedServerRunner`.
- The native runner owns `IntegratedServer` on an OS thread, receives encoded
  `ClientCommand` frames, publishes encoded `ServerUpdate` frames, runs the
  server tick loop on the runner thread, and joins cleanly on shutdown.
- `WindowSceneRuntime` local integrated mode no longer stores or ticks
  `IntegratedServer`; it sends commands to the runner and drains runner updates
  during client polling. Remote TCP still uses `RemoteServerSession`.
- Existing native worldgen, light-status, and render-section compile workers
  remain worker-backed; the runner only moves server ownership and tick pacing.
- Desktop runtime/debug stats now expose runner kind, command queue depth,
  update queue depth, server tick timing, pending server jobs, and pending
  publications.
- Tests cover command/update crossing over runner frames, native-thread
  diagnostics, clean shutdown, and a default desktop local-mode regression
  against inline fallback.

### 2. Shared Runner Diagnostics And Tests

Make runner topology visible and testable before adding the web runner.

Acceptance:

- Shared diagnostics distinguish `InlineFallback`, `NativeThread`, and
  `WebWorker`.
- Native unit tests prove commands cross the runner boundary, updates drain on
  the client side, and shutdown joins the runner.
- A regression test or smoke assertion fails if default desktop integrated mode
  falls back to inline execution.

### 3. WASM Integrated Server Worker

Move browser local integrated mode behind a Web Worker runner.

Port the same runner boundary to browser/WASM with a Web Worker backend,
keeping the `IntegratedServerRunner` client-facing shape and encoded
command/update frames. The web slice should also assert `WebWorker` runner kind
in `native:web:app-smoke` before adding WebSocket/WebRTC transport work.

Acceptance:

- The browser app no longer constructs or ticks `IntegratedServer` on the main
  WASM instance.
- The server worker owns `IntegratedServer`, readiness, command ingress,
  update egress, and tick cadence.
- Main-thread web runtime communicates with the server worker through encoded
  command/update frames.
- Initial implementation may use `postMessage` transfer for correctness, but
  the interface must preserve the path to shared-memory queues.
- `native:web:app-smoke` reports runner kind `WebWorker`, proves input,
  movement, correction handling, block interaction, chunk streaming, sky,
  actors, worker render compiles, and zero pending jobs after settle.
- Playwright screenshots remain part of validation.

Landed in the browser worker slice:

- The browser app now constructs `WebRuntime` with a `WebIntegratedServerRunner`
  instead of the inline loopback host. The raw wasm runtime smoke keeps the
  explicit inline fallback so it can still run through a direct wasm export
  without browser `Worker` plumbing.
- `mclone-integrated-server-worker.js` loads the bindgen module inside a module
  worker, owns `IntegratedServer`, posts readiness, runs a 20 TPS tick timer,
  receives encoded `ClientCommand` frames, and transfers encoded
  `ServerUpdate` frames back to the main wasm instance.
- The wasm app session exposes async command-crossing methods for chunk view,
  movement/correction sync, and block interaction. JS app code serializes
  session access so async worker exchanges do not overlap unsafe mutable
  wasm-bindgen borrows.
- Web render reports now include runner kind, command/update queue depths,
  pending server jobs, pending publications, and last server simulation tick.
  `native:web:smoke` and `native:web:app-smoke` assert `web-worker` runner kind
  and settled queues/jobs. The standard web smoke also calls explicit session
  shutdown and verifies the worker shutdown report.
- Existing browser render-section compilation remains worker-backed. The
  existing raw wasm runtime smoke and render compiler smoke fallback are kept
  as temporary compatibility paths until worker-backed worldgen/light mailboxes
  replace WASM inline server job execution.

### 4. WASM Server Job Workers For Lighting And Worldgen

Replace WASM inline server mailboxes with Web Worker-backed job mailboxes.

Acceptance:

- WASM worldgen feature jobs run behind a worker mailbox or worker pool.
- WASM light-status / level-light jobs run behind a worker mailbox or worker
  pool.
- Browser reports expose mailbox kinds and pending/completed counts for
  worldgen and lighting.
- The default browser app path does not run worldgen or lighting inline on the
  client frame path.
- Lighting remains authoritative server data; render mesh workers consume
  published light facts rather than recomputing lighting locally.

Landed in the browser server-job worker slice:

- `mclone-server` now has a binary job-frame boundary for worldgen feature jobs
  and light-status batches. The frame tests cover request/response crossing for
  generated chunks, retained dependency buffers, light inputs, completed light
  sections, and timing/cache diagnostics.
- WASM `WorldgenMailbox` and `LightStatusMailbox` keep the explicit inline
  fallback but use browser `Worker` backends when the web integrated server
  supplies a `WasmServerJobWorkerConfig`. Native desktop still uses the existing
  `mclone-worldgen` and `mclone-light-status` OS threads.
- `mclone-integrated-server-worker.js` now starts the Rust integrated server
  with job-worker URLs, posts server job frames to `mclone-server-job-worker.js`,
  and waits asynchronously for pending server jobs/publications to drain instead
  of spinning inside the Rust server worker.
- Web render reports and smokes now expose/assert `worldgenMailboxKind` and
  `lightStatusMailboxKind` as `web-worker`, with settled worldgen/light pending
  counts. Existing render-section compilation remains worker-backed.
- That slice originally used message/transfer transport. The follow-up shared
  server-job frame slice below moves the worldgen/light job payloads onto
  `SharedArrayBuffer` while keeping message transfer as the fallback.

### 5. Worker Queue Upgrade To Shared Memory

After the message-based worker topology is correct, move hot web paths to
`SharedArrayBuffer`/Atomics queues where measurement shows transfer overhead or
latency matters.

Acceptance:

- COOP/COEP remains enforced by web smoke servers.
- Shared Wasm memory and Atomics are still required by thread smokes.
- Queue metrics distinguish transferred-byte paths from shared-memory paths.
- Fallback message transport is explicit and not the default for performance
  lanes once shared queues are available.

Landed in the queue-measurement slice:

- Shared runner diagnostics now include `WorkerFrameMetrics` for the integrated
  server runner, worldgen job workers, and light-status job workers. Metrics
  report transport kind, request/response frame counts, transferred byte
  counts, pending-frame high water, and observed request latency.
- Browser integrated server command/update frames and browser worldgen/light
  server-job frames record `message-transfer` metrics while keeping native
  desktop runner and native job-worker diagnostics labeled as `none` for this
  browser-specific transport.
- Web render reports and app state expose `runnerFrameMetrics`,
  `worldgenJobFrameMetrics`, and `lightStatusJobFrameMetrics`; the standard
  web smoke asserts all three message-transfer paths are active.
- Native tests cover metric accumulation and preserve native-thread diagnostics
  as non-message-transfer paths. This gives the next slice a regression guard
  when one path moves to shared memory and the fallback remains available.

Landed in the first shared-memory server-job slice:

- Browser worldgen and light-status server-job workers now prefer
  `SharedArrayBuffer` request and response frames when `SharedArrayBuffer` and
  the required `Atomics` operations are available in the integrated-server
  worker.
- The existing transferred-`Uint8Array` server-job transport remains as the
  explicit fallback for non-isolated browsers or runtimes without shared memory.
- `mclone-server-job-worker.js` accepts both frame shapes. The shared-memory
  path uses an atomic control header for status and byte counts, then returns
  the large response payload through a shared response buffer instead of a
  transferred array buffer.
- Web render reports and smokes now keep integrated runner command/update
  frames labeled `message-transfer`, while asserting worldgen and light-status
  job-frame metrics report `shared-memory` with nonzero request/response
  counts and byte totals.

Landed in the shared-buffer pool slice:

- WASM server-job workers now reuse bounded shared-buffer slots for
  worldgen/light-status requests and responses instead of allocating fresh
  `SharedArrayBuffer`s for every frame.
- Each slot carries a reusable atomic control header, request buffer, and
  response buffer. Slots are owned by the Rust-side worker wrapper while
  inflight, then returned to a small pool when the browser worker replies.
- Oversized responses keep an explicit fallback path: the browser worker
  allocates a one-off shared response buffer, reports the fallback, and the
  Rust-side slot grows before re-entering the pool.
- `WorkerFrameMetrics` now reports pool hits, misses, drops, live/max shared
  capacity, and pooled/fallback response counts. Web render reports and smoke
  tests assert pooled shared responses with no fallback on the standard
  worldgen/light-status path.

Landed in the shared runner frame slice:

- The browser integrated-server runner now prefers `SharedArrayBuffer` command
  frames and packed update-response frames when shared memory and the required
  `Atomics` operations are available.
- The existing transferred-`Uint8Array` command/update path remains the
  fallback for non-isolated browsers or runtimes without shared memory.
- Main-thread Rust owns a bounded runner shared-slot pool for command/result
  exchanges; the integrated-server worker can also publish tick updates through
  worker-owned shared response buffers and releases them after the main thread
  copies the packed frames.
- Web runner diagnostics now report `shared-memory` transport with pool
  hit/miss/drop, capacity, pooled response, and fallback response counters. The
  standard web smoke asserts the runner, worldgen, and light-status lanes are
  all shared-memory-backed with pooled responses on the normal path.

Landed in the shared-lane stress slice:

- The web smoke now runs an explicit shared-topology stress export after the
  normal render/session shutdown path. The stress path starts a fresh
  integrated-server worker with shared runner transport forced on and a small
  initial response buffer so oversized update responses exercise the fallback
  buffer path and grow the reusable slot before it re-enters the pool.
- The stress path posts multiple chunk-view commands before yielding to the
  browser event loop, proving multiple in-flight runner frames, runner pool
  misses, bounded-pool drops, and max-pending-frame diagnostics. A reuse phase
  then asserts pooled shared slots are hit after overflow.
- The same stress report includes worldgen and light-status job metrics from
  the stressed runner, so the browser smoke continues to prove those server-job
  lanes stay shared-memory-backed while the runner is under contention.
- A second forced `message-transfer` runner probe keeps the fallback transport
  observable and asserts it has command/update frame activity without shared
  pool counters.

Next implementation slice:

- Add first-class browser remote transport by introducing a WebSocket client
  adapter to a native hosted server, while keeping the same client runtime and
  command/update application path as local integrated mode. Do not add WebRTC
  until the WebSocket adapter proves the transport boundary.

### 6. Browser Remote Transport

Add browser remote-client transport after local integrated threading is not
inline anymore.

Likely order:

1. WebSocket client transport to a native hosted server.
2. WebRTC data-channel transport if browser-to-browser or lower-latency NAT
   traversal becomes a product target.

Acceptance:

- Browser remote mode uses the same client runtime/update application path as
  local integrated.
- Native hosted server can serve both native TCP clients and browser clients
  through transport adapters.
- Remote players and entity snapshots continue to flow through protocol
  updates, not special browser-only state.

## Validation

Threading slices should keep the existing native and web validation lanes:

```text
cargo test --manifest-path native/Cargo.toml
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
pnpm --silent native:web:build
pnpm --silent native:web:thread-smoke
pnpm --silent native:web:app-smoke
```

Rendered-output slices still need screenshot inspection under `/tmp`, including
native and Playwright browser captures.

New validation should include:

- runner-kind assertions for desktop and web
- mailbox-kind assertions for worldgen/light/render compile workers
- queue-depth and pending-job counters in smokes/debug stats
- a stress path that moves chunk interest while server jobs are pending and
  proves the client frame loop still presents
- shutdown tests for native runner threads and browser worker termination

## Non-Goals

- No legacy TypeScript engine edits.
- No Android, Quest, or OpenXR scaffolding in this tactical.
- No broad renderer rewrite.
- No browser remote WebSocket/WebRTC transport before local integrated worker
  topology is in place.
- No direct WebGPU work from server workers.

## Completed Implementation Slices

1. Native desktop integrated server runner:
   shared runner interface, native OS-thread runner, `WindowSceneRuntime`
   routing, diagnostics, shutdown tests, native smokes, and screenshot
   validation.
2. Browser/WASM integrated server worker:
   module-worker runner, encoded command/update frame crossing, async web app
   session routing, runner diagnostics in web reports, worker-kind and settled
   queue assertions, explicit browser worker shutdown smoke, and Playwright
   screenshot validation.
3. Browser/WASM server job workers:
   worker-backed worldgen and light-status mailboxes for the browser integrated
   server, binary job-frame codecs, mailbox-kind diagnostics in web reports,
   native/web tests and smokes, and screenshot validation.
4. Browser/WASM worker frame metrics:
   shared frame metric diagnostics, browser runner/job-worker transferred-byte
   and frame counters, web report/app exposure, web smoke assertions for active
   message-transfer paths, native metric tests, and screenshot validation.
5. Browser/WASM shared-memory server-job frames:
   shared request/response buffers for worldgen and light-status job workers,
   atomic status/byte-count headers, transferred-message fallback, web smoke
   assertions for shared-memory job metrics, and screenshot validation.
6. Browser/WASM shared-buffer pools and runner frames:
   bounded shared-buffer pools for server-job and integrated-runner frames,
   shared runner command/update payloads, pooled/fallback response counters,
   normal-path smoke assertions, contention/backpressure stress coverage,
   forced transfer fallback coverage, and Playwright screenshot validation.

## Next Implementation Slice

Start browser remote transport with a WebSocket client adapter to the native
hosted server. The good next chunk is to define the browser-side transport
adapter behind the existing command/update runtime boundary, add a native
hosted server smoke path that a browser can connect to, and assert runner kind /
transport diagnostics without changing local integrated mode. WebRTC remains a
later transport slice after WebSocket proves the shape.
