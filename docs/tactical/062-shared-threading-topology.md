# 062: Shared Threading Topology

Status: in progress - native desktop runner landed

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

Next step: port the same runner boundary to browser/WASM with a Web Worker
backend, keeping the `IntegratedServerRunner` client-facing shape and encoded
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

## First Implementation Slice

Start with the native desktop integrated server runner:

1. Introduce the shared runner interface and diagnostics.
2. Implement `NativeIntegratedServerRunner` as an OS thread owning
   `IntegratedServer`.
3. Update `WindowSceneRuntime` local integrated mode to use the runner instead
   of owning `IntegratedServer`.
4. Preserve remote TCP behavior and existing native worker mailboxes.
5. Add tests for command/update crossing, diagnostics, and shutdown.
6. Validate with native tests, movement/timedemo smoke, and a native screenshot.

Once that lands, repeat the same boundary in browser/WASM with a Web Worker
runner instead of inventing a separate web-only runtime shape.
