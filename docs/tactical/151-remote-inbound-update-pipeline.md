# 151: Remote Inbound Update Pipeline

Status: closed 2026-07-07; Slice 0 documentation baseline completed 2026-07-06;
Slice 1 shared `ClientConnection` runtime boundary completed 2026-07-06; Slice
2 native TCP client IO actor completed 2026-07-06; Slice 3 remote runtime budget
and diagnostics completed 2026-07-06; Slice 4 Quest remote chunk-view churn
rebaseline completed 2026-07-06; Slice 4A remote command enqueue decoupling
completed 2026-07-06. Slice 5 was re-planned 2026-07-07 into 5A-5D because the
native `ClientConnection` boundary still lived inside the single-view scene
module; Slice 5A shared contract promotion completed 2026-07-07; Slice 5B web
integrated shared ingress completed 2026-07-07; Slice 5C web remote shared
ingress completed 2026-07-07; Slice 5D close-out audit completed 2026-07-07.
This is the focused remote connection successor to tactical
[`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
and a convergence slice for tactical 133's shared bus shape. Architecture target:
[`../session-network-architecture.md`](../session-network-architecture.md).
Umbrella bus work: tactical
[`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md).

Workstream: native Rust shared session/runtime boundary, native TCP transport,
native web/WASM adapters, Android XR validation. Goal: introduce one shared
client-facing connection abstraction for local integrated, native remote, and
web transports, then make remote dedicated inbound updates arrive through that
same budgetable update boundary without making the runtime/render frame read and
decode TCP response batches.

## Problem

The 2026-07-06 remote fixes removed two older blockers:

- `SendOnly` commands no longer drain/apply remote response batches inside the
  command send path.
- Remote `poll()` no longer waits for a response that is not ready; it uses TCP
  readiness first.

The remaining Quest RD5 remote churn stall is different. One response batch was
ready, and the runtime frame read/decoded/applied that entire batch:

- `app_work_ms=268.630`
- `poll_total_ms=262.168`
- `drain_updates_ms=253.930`
- `apply_updates_ms=9.344`
- `updates=402`

This means the frame is no longer waiting for the host to finish work. It is
paying client-side receive/decode and batch construction for a large ready
response. That work must move behind a client inbound queue.

## Reference Shape

Minecraft Java 1.17.1 keeps local/integrated and remote/multiplayer shaped like
the same connection stack:

- `Connection.connectToLocalServer(...)` uses Netty `LocalChannel`, so
  integrated singleplayer bypasses physical TCP but not the packet/connection
  model.
- Remote multiplayer uses socket channels and a Netty worker group.
- `PacketDecoder.decode(...)` constructs packet objects on the Netty pipeline.
- `PacketUtils.ensureRunningOnSameThread(...)` schedules packet handling back
  onto the Minecraft client/server thread when needed.
- `ClientPacketListener.handleLevelChunk(...)` installs the chunk data and
  marks sections dirty on the client thread.
- `ChunkRenderDispatcher` handles expensive mesh rebuild work separately.

The target here follows that shape at the boundary level: receive/decode is
producer-side, ordered apply is client-runtime-side, and meshing remains in the
terrain compile/upload pipeline. We are not adding a chunk-meshing shortcut or
a speculative chunk-decode worker pool in this tactical.

## Current Mclone Shape

Local integrated already bypasses socket I/O. `NativeIntegratedServerRunner`
owns the integrated server on a runner thread and publishes decoded
`ServerUpdate` envelopes through a queue. The client runtime drains that queue
under a budget.

Native remote dedicated no longer has the normal runtime frame own the TCP
socket. As of Slice 2:

- `NativeClientIoSession` owns a background IO actor thread and keeps
  `NativeClientSession` as a low-level compatibility/test helper.
- The actor owns the primary `TcpStream`, writes outbound command frames, blocks
  reading paired response batches, decodes updates, and queues
  `NativeServerUpdateBatch` envelopes with byte/age/read/decode metadata.
- As of Slice 4A, runtime `SendOnly` enqueue returns once the command is accepted
  into the actor's ordered outbound queue. Physical write, flush, response read,
  decode, and disconnect reporting remain actor/adapter work surfaced through the
  later drain/poll/reconnect path.
- Desktop, Android, and Android XR normal remote session wrappers hold
  `NativeClientIoSession`, not `NativeClientSession`.
- `RemoteDedicatedSingleViewSceneRuntime` still has a temporary
  `pending_response_batches` counter inside its connection adapter because the
  wire protocol remains one response per command. That counter no longer means
  "socket read pending on the runtime thread"; it is adapter bookkeeping for
  ordered response completion.

Server-side dedicated already has an accept thread and a connection thread per
client. The remaining pieces are host-mode-honest diagnostics projection in
tactical 149 and web callback adoption behind the same client-facing connection
abstraction.

As of 2026-07-07, `ClientConnection`, `QueuedServerUpdate`,
`ClientConnectionDrainResult`, `ClientConnectionQueueMetrics`, and the shared
budgeted drain helper live in
`mclone-app-runtime/src/client_connection.rs`. Native local/remote adapters and
web integrated inline/worker adapters use that shared contract. Web remote
WebSocket callbacks now decode response batches into ordered queued updates and
normal-frame web remote polling drains through the same shared contract.

## Target Shape

Shared client-facing target:

```text
runtime/app thread
  -> ClientConnection::enqueue_command(ClientCommand, CommandQueuePolicy)
  -> ClientConnection::drain_update / drain_updates(RuntimeUpdatePumpBudget)
  -> apply decoded ServerUpdate envelopes in receive order

ClientConnection implementation
  -> owns local runner bridge, TCP IO actor, WebSocket callback bridge,
     worker bridge, or future P2P transport
  -> exposes one command queue, one ordered inbound update queue, and one
     diagnostics surface
```

First native remote implementation behind that boundary:

```text
runtime/app thread
  -> enqueue ClientCommand through ClientConnection
  -> drain decoded ServerUpdate envelopes through ClientConnection

native remote client IO actor
  -> owns TcpStream
  -> writes outbound command frames
  -> blocks reading response batches
  -> decodes batches into ordered ServerUpdate envelopes
  -> pushes envelopes plus byte/age/timing metadata to inbound queue

dedicated server connection thread
  -> unchanged first: one command in, one response batch out
```

The first implementation can keep the current one-response-per-command wire
protocol. Server-push broadening is useful later, but it is not required to
remove the Quest ready-batch stall from the runtime frame.

Web target:

```text
WebSocket message callback or Web Worker
  -> decode/queue ordered ServerUpdate envelopes
runtime frame
  -> drain/apply through ClientConnection
```

No native OS thread is required for the first web slice. The shared contract is
"one `ClientConnection` boundary with an ordered inbound queue plus budgeted
decode/apply accounting", not "all platforms spawn the same kind of thread".

## Threading And Drain Contract

This tactical should move Mclone closer to the reference shape at the boundary,
not by copying Netty or Java's exact task scheduler:

- Java integrated singleplayer uses `Connection` over Netty
  `LocalChannel` / `LocalServerChannel`; it is an in-process memory transport,
  not loopback TCP and not a separate client/server API.
- Java multiplayer uses socket channels and Netty worker groups.
- Java packet decode/object construction happens on the channel pipeline, then
  `PacketUtils.ensureRunningOnSameThread(...)` hands packet handling to the
  client/server thread when needed.
- Java's client drains all queued packet-handler tasks each frame in receive
  order. Mclone intentionally diverges with `RuntimeUpdatePumpBudget` for Quest
  frame pacing; `Unlimited` remains the parity escape hatch for startup,
  idle waits, and tests.
- Mclone local integrated may remain runner-thread plus Rust channels
  internally. Native remote should use a blocking TCP IO actor. Web should use
  browser callbacks or workers. Those are transport implementation details
  behind one `ClientConnection` contract.
- The shared runtime should stay frame-driven and synchronous at the boundary.
  Do not make the runtime async just because a web or TCP implementation is
  async/evented internally.

Non-goal for Slice 1: do not build a Netty clone, a literal Java packet-task
queue, or a second local-vs-remote runtime interface. The reference requirement
is one connection-shaped command/update surface with ordered producer-side
decode and client-runtime-side apply.

## Contract For Implementing Agents

Read this section, the active slice, the reference-shape section above,
[`../session-network-architecture.md`](../session-network-architecture.md),
tactical
[`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md),
and the relevant code before writing code. If context is compressed mid-task,
re-read this section before continuing.

1. **One client connection contract.** Local integrated, native remote TCP, web
   integrated worker/inline, web remote WebSocket, desktop, Android, Android XR,
   and future session shells converge on the same command/update boundary.
   Transport internals may differ; runtime-facing ingress must not.
2. **The contract is not view-owned.** `ClientConnection` and its update
   envelope are a session/app-runtime boundary, not a single-view, XR, desktop,
   web, or renderer detail. A new copy in an app crate or view runtime is a
   defect.
3. **Vanilla-shaped ownership.** Producer-side code owns transport, framing,
   packet/update decode, queue metadata, and disconnect/error reporting.
   Runtime/client-thread code drains ordered decoded updates and applies them.
   Rendering, dirty-section admission, meshing, and GPU upload remain downstream
   systems.
4. **Slice 5A is refactor-only.** Promoting the contract out of
   `local_single_view.rs` must not change wire protocol, queue ordering,
   budgets, default flags, worker counts, startup behavior, reconnect behavior,
   or diagnostics meaning.
5. **Receive order is binding.** Snapshot -> block update -> unload and paired
   remote responses must preserve receive order across budget stalls, startup
   drains, reconnect/resync, and web callback/promise boundaries.
6. **Normal frame polling is nonblocking.** Normal `poll()` may drain already
   queued decoded updates under `RuntimeUpdatePumpBudget`; it must not wait for
   socket reads, WebSocket promises, worker messages, response pairing, or
   ready-batch decode. `DrainImmediately`, startup, and idle waits are explicit
   exceptions.
7. **Budget semantics stay shared.** `RuntimeUpdatePumpBudget` is the only
   runtime update-apply pacing surface in this tactical. Web must adopt the
   shared drain semantics instead of adding a web-only update throttle.
8. **Diagnostics are honest.** Producer read/decode, queue depth, bytes, age,
   response sequence, stalls, and conservation fields keep their current
   meanings. Unavailable host-side counters are reported as unavailable, never
   zero.
9. **No diagnostics drift without a named detour.** Add counters only when a
   named gate or slice decision cannot be made from existing fields, record the
   missing evidence in this document, and name the consuming decision before
   adding the counter.
10. **No adjacent workstreams.** Do not take server-push protocol broadening,
    compression, binary protocol redesign, mesh/render admission, GPU upload
    policy, chunk streaming priority lanes, authority/correctness redesign, or
    platform lifecycle rewrites in this tactical.
11. **Web mechanics stay behind the adapter.** Browser promises, WebSocket
    callbacks, worker construction, `SharedArrayBuffer`, JS glue, and browser
    lifecycle code may remain web-specific. The runtime-facing command/update
    shape may not.
12. **No async infection of the shared runtime.** It is acceptable for a
    producer adapter to be async/evented internally. The shared runtime boundary
    remains frame-driven and synchronously drained.

## Slice 5 Preflight Gate Check

Run this before starting any Slice 5A-5D implementation. If a preflight gate is
already red, classify it before editing. Do not hide an existing failure inside
the slice diff.

```bash
git status --short
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
rg -n "NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

Preflight interpretation:

- `ClientConnection` hits in `local_single_view.rs` are expected before Slice
  5A and disallowed after Slice 5A except for adapter implementations/tests.
- `WebRuntimeHost` and `exchange_command` hits are expected before Slice 5B and
  Slice 5C. After web adoption, remaining hits must be explicit adapter,
  startup, compatibility, or test paths, not normal frame update polling.
- `NativeClientSession` and legacy drain hits are allowed only as low-level
  compatibility helpers, tests, or adapter internals. Normal desktop, Android,
  and Android XR remote sessions must stay on `NativeClientIoSession`.
- If `pnpm native:web:smoke` is already red, Slice 5B/5C cannot close until the
  failure is fixed or recorded as an unrelated blocker with user agreement.
- Quest rebaseline is not required for Slice 5A if the diff is a pure move and
  the native checks stay green. Re-run the Quest remote churn lane if native
  remote enqueue/drain semantics change.

Tripwires that stop the slice and require a document update before continuing:

- The implementation needs a second runtime-facing local-vs-remote or
  native-vs-web connection interface.
- A normal frame path must await a web promise, read a socket, or decode a
  ready response batch to make progress.
- The shared runtime boundary has to become async to compile.
- A counter changes from unavailable to zero, or producer time is double-counted
  between batch totals and per-update metadata.
- Web adoption requires a broad worker lifecycle or `SharedArrayBuffer`
  redesign before the shared ingress can be represented.
- A validation command fails after edits and cannot be explained by an
  explicitly recorded pre-existing failure.

## Slice Rules

- One slice per session.
- At the end of each slice: update this document with status/evidence, run the
  named validation, commit the slice files, and state the next step. If the
  worktree has unrelated dirty files, leave them untouched and call them out.
- Preserve receive order. Do not add priority lanes or coalescing for chunk
  streaming here.
- Do not add another runtime-facing local-vs-remote session interface. Local
  integrated, native remote TCP, web remote, and worker-backed local play
  should converge on `ClientConnection` even if their transport internals
  differ.
- Adding `ClientConnection` without deleting or moving the duplicate
  local-vs-remote runtime pump logic is not a completed slice. Transport
  adapters may differ; normal frame update draining must be shared.
- Keep startup, `DrainImmediately`, and `poll_until_idle` behavior explicit.
  Unlimited drains are allowed for those paths, but normal frame `poll()` must
  not read/decode socket batches.
- Report unavailable host-side counters as unavailable, not zero. Tactical 149
  owns the projection work after this stall is removed.

## Slice 0: Documentation Baseline

Status: completed 2026-07-06.

Deliverables:

- Update `session-network-architecture.md` with the current remote baseline:
  remote `SendOnly` and readiness landed, but ready batches still read/decode
  on the runtime path.
- Add this tactical as the focused tracker for the shared client connection
  boundary plus the remote inbound queue.
- Update the tactical index and stale tactical 133 wording so docs do not claim
  remote still ignores `SendOnly`.

Validation:

```bash
git diff --check
```

## Slice 1: Shared Client Connection Contract And Runtime Seam

Status: completed 2026-07-06.

Goal: create one runtime-facing `ClientConnection` boundary before moving the
remote socket. Local integrated and remote dedicated should stop presenting
different session shapes to the runtime.

Deliverables:

- Define the shared connection contract in the shared runtime/session layer.
  Exact names can change, but the shape must cover:
  - enqueue ordered `ClientCommand`s with a send/drain policy;
  - drain one ordered `QueuedServerUpdate` at a time;
  - drain under `RuntimeUpdatePumpBudget` for normal frame polling;
  - expose diagnostics for command depth, update depth/bytes/age,
    producer read/decode time, stalls, and conservation checks.
- Introduce a shared inbound update envelope that carries:
  - decoded `ServerUpdate`,
  - encoded byte length,
  - batch sequence or response sequence,
  - received/queued timestamp,
  - producer-side read/decode timing.
- Adapt local integrated to the same `ClientConnection` boundary using the
  existing `NativeIntegratedServerRunner` command/update channels. This should
  be mostly an adapter move, not a behavior change.
- Adapt remote dedicated runtime code to the same `ClientConnection` boundary.
  In Slice 1 it may still fill the queue from the existing direct
  `try_drain_command_updates` call, but "receive ready batch" and "apply queued
  updates" must be separate internal steps.
- Move the normal-frame drain/apply loop behind one shared runtime helper over
  `ClientConnection`. Local integrated and remote dedicated should call the
  same helper for `poll()` budget handling, queue depth/byte/age diagnostics,
  stall reporting, and apply timing.
- Delete or demote remote-only runtime state whose sole job is pending response
  draining. If `pending_response_batches` or equivalent state still exists in
  Slice 1, it must live inside the remote connection adapter as temporary
  transport state, not in the runtime pump.
- Remove direct normal-frame calls to remote transport drains from runtime and
  platform adapters. Calls such as `try_drain_command_updates` /
  `drain_command_updates` should be connection-adapter internals or explicit
  startup/compatibility helpers, not the app frame path.
- Add tests proving a queued snapshot -> section update -> unload sequence
  preserves order across budget stalls.
- Add regressions proving local integrated and remote-shaped connections both
  honor `SendOnly`, normal frame `poll()` applies from the shared queue under
  budget, and `DrainImmediately` remains explicit.

Non-goals: no new thread yet, no wire protocol change, no web adoption.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
rg -n "try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/apps/mclone-* || true
git diff --check
```

The `rg` audit is not expected to be zero in Slice 1, but every remaining hit
must be classified as one of: connection-adapter internals, explicit
startup/compatibility helper, or test. No remaining hit may be part of normal
frame runtime polling.

Implementation notes:

- Added a shared runtime-facing `ClientConnection` trait in
  `mclone-app-runtime` with command send, one-at-a-time update drain, and queue
  depth/byte metrics.
- Added `QueuedServerUpdate` as the shared inbound envelope carrying decoded
  `ServerUpdate`, encoded byte length, queue age, and transport-drained state.
  True producer read/decode timing and response sequencing remain explicitly
  unavailable until the Slice 2 native TCP IO actor owns the socket.
- Adapted local integrated through `LocalIntegratedConnection` over the
  existing `NativeIntegratedServerRunner` channels.
- Adapted remote dedicated through `RemoteDedicatedConnection`. Its temporary
  `pending_response_batches` and direct `try_drain_command_updates` /
  `drain_command_updates` calls now live inside that adapter, not in the
  runtime pump.
- Replaced the duplicate local runner pump and remote pending-batch pump with
  `pump_client_connection_updates_report`, shared by local and remote normal
  frame `poll()`, `DrainImmediately`, and remote `poll_until_idle`.
- Added a focused shared-pump test for receive-order preservation across a
  budget stall, and updated the remote `SendOnly` regression to assert through
  the connection adapter.

Validation completed 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
rg -n "try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/apps/mclone-* || true
git diff --check
```

`rg` classification:

- `mclone-app-runtime/src/local_single_view.rs`: remaining
  `pending_response_batches`, `drain_command_updates`, and
  `try_drain_command_updates` hits are temporary remote connection-adapter
  internals or tests.
- `mclone-app-runtime/src/host_mode.rs`: remaining drain hits are the
  compatibility trait and existing startup/test helpers.
- `native/apps/mclone-*`: remaining drain hits are `RemoteDedicatedServerSession`
  compatibility wrappers around `NativeClientSession`; Slice 2 should replace
  the normal native remote path with an IO actor behind the same
  `ClientConnection` boundary.
- `pump_pending_remote_update_batches_report`: no remaining hits.

## Slice 2: Native TCP Client IO Actor

Status: completed 2026-07-06.

Goal: move native remote socket read/decode off the runtime frame.

Deliverables:

- Add a native remote client IO actor/thread that owns `TcpStream`.
- Runtime/app side sends outbound `ClientCommand`s through a command channel.
- IO actor writes commands in order, reads the paired response batch with
  blocking socket I/O, decodes updates, and pushes ordered inbound envelopes to
  the `ClientConnection` inbound queue from Slice 1.
- Preserve the existing handshake and frame codec.
- Preserve reconnect/resync behavior, including clearing stale pending response
  state and issuing the resync command through the new actor.
- Expose actor diagnostics: outbound command depth, inbound update depth/bytes,
  read/decode max/total, disconnected/error state, and response sequence.
- Keep old `NativeClientSession::send_command` helpers for tests/tools if they
  are still useful, but remove them from the normal runtime frame path.

Non-goals: server push, compression, WebSocket rewrite.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
git diff --check
```

Implementation notes:

- Added `NativeClientIoSession` in `mclone-net`. It spawns
  `mclone-native-client-io`, keeps the existing native TCP handshake/frame
  codec, sends command frames from a command channel, blocks reading paired
  response batches on the actor thread, decodes updates there, and queues
  `NativeServerUpdateBatch` envelopes.
- Preserved `send_command_only` transport semantics with an actor write
  acknowledgment: callers wait until the command frame is written/flushed, but
  they do not wait for the response read/decode.
- Added actor diagnostics for outbound command depth, inbound response/update
  depth and bytes, read/decode total/max, response sequence, disconnected state,
  and last error.
- Added `RemoteCommandUpdateBatch` / `RemoteCommandUpdate` defaults to the
  app-runtime host session trait so actor-backed sessions can pass encoded
  lengths, queue age, response sequence, and producer timing into
  `RemoteDedicatedConnection` while legacy tests/tools still return plain
  `Vec<ServerUpdate>`.
- Switched desktop, Android, and Android XR normal remote session wrappers to
  `NativeClientIoSession`. `NativeClientSession` remains in `mclone-net` for
  low-level tests/tools and the standalone remote-player visual smoke helper.
- Updated desktop scene-runtime tests to keep `SendOnly` explicit and use
  `DrainImmediately` for the reconnect/resync assertion.

Validation completed 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client remote_session
rg -n "NativeClientSession|drain_command_updates|try_drain_command_updates|NativeClientIoSession|drain_update_batch|try_drain_update_batch|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

`rg` classification:

- `NativeClientSession`: remains as the low-level native TCP compatibility
  helper in `mclone-net`, its tests, `request_server_updates`, and the
  standalone desktop remote-player visual smoke helper. It is no longer held by
  the normal desktop, Android, or Android XR remote session wrappers.
- `drain_command_updates` / `try_drain_command_updates`: remain as legacy trait
  defaults, compatibility wrapper methods, and tests. The normal runtime
  connection adapter now calls `drain_command_update_batch` /
  `try_drain_command_update_batch`.
- `pending_response_batches`: remains only inside the remote
  `ClientConnection` adapter and tests as temporary response-paired protocol
  bookkeeping.
- `pump_pending_remote_update_batches_report`: no remaining hits.

Validation note: the app-crate check still reports existing Android XR
dead-code warnings for `ANDROID_REMOTE_ADDR_NONE_SENTINEL` and
`normalize_android_legacy_remote_addr`; they are unrelated to this slice.

## Slice 3: Remote Runtime Budget And Diagnostics

Status: completed 2026-07-06.

Goal: make remote frame `poll()` indistinguishable from local integrated at the
client update boundary.

Deliverables:

- Normal frame `poll()` drains decoded remote inbound updates one at a time
  under `RuntimeUpdatePumpBudget`, always applying at least one pending update
  for progress.
- `DrainImmediately`, startup, and `poll_until_idle` use explicit unlimited
  queue drains without reintroducing socket reads on the runtime thread.
- `RuntimePollTiming` and Android XR markers split:
  - producer read time,
  - producer decode time,
  - queued bytes/depth/age,
  - runtime apply time,
  - dirty mark/client apply time,
  - deferred/stalled counts.
- Remove or rename any "pending response batch" diagnostics that now really
  mean inbound queued updates.
- Add a desktop loopback remote smoke that asserts no runtime poll sample has
  socket read/decode attribution.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
pnpm native:accounting:smoke
git diff --check
```

Implementation notes:

- Carried producer-side read time, decode time, and response sequence through
  `RuntimeUpdatePumpReport`, `RuntimePollTiming`, and
  `RuntimePollDiagnostics`.
- Preserved native IO actor batch totals while avoiding double counting:
  `NativeServerUpdateBatch` still owns the full producer read/decode totals,
  per-update envelopes carry per-update parser samples, and
  `RemoteDedicatedConnection` attaches the batch producer total once to the
  first queued update from that batch. Legacy/fake sessions without batch
  totals still use per-update metadata.
- Made normal-frame remote `poll()` surface producer metadata from already
  queued decoded updates without making the runtime thread read or decode the
  socket response.
- Added desktop perf JSON fields for `poll_producer_read_ms`,
  `poll_producer_decode_ms`, and `poll_producer_response_sequence` in frame
  budget and startup streaming reports, including startup aggregate totals.
- Added Android XR runtime max markers for producer read/decode/sequence through
  `XrTerrainUploadSummary`.
- Added a desktop loopback regression that holds a remote response while the
  runtime polls, then asserts the held socket time appears as producer read
  time and not as runtime drain/poll wall time.

Validation completed 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client window_runtime_remote_poll_reports_io_actor_read_without_blocking -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
pnpm native:accounting:smoke
cargo fmt --manifest-path native/Cargo.toml --all --check
rg -n "pending response batch|pending_response_batch|pump_pending_remote_update_batches_report|try_drain_command_updates|drain_command_updates|NativeClientSession" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

Validation note: the app-crate check still reports the existing Android XR
dead-code warnings for `ANDROID_REMOTE_ADDR_NONE_SENTINEL` and
`normalize_android_legacy_remote_addr`; they are unrelated to this slice.

`rg` classification:

- `pending_response_batches` remains only as response-paired protocol
  bookkeeping inside the remote `ClientConnection` adapter and its tests.
- `drain_command_updates` / `try_drain_command_updates` remain as legacy trait
  defaults, compatibility wrappers, low-level native session helpers, and
  tests. Normal runtime polling uses the shared update queue.
- `NativeClientSession` remains as the low-level native TCP compatibility
  helper and the standalone remote-player visual smoke helper, not the normal
  desktop/Android/Android XR remote runtime session.

## Slice 4: Quest Remote Churn Rebaseline

Status: completed 2026-07-06.

Goal: prove the specific 253.930 ms ready-batch drain moved off the headset
runtime frame.

Deliverables:

- Build the Android XR APK.
- Run the same Quest RD5 remote chunk-view churn lane used in tactical 149:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

- Record whether max runtime poll is now dominated by apply/terrain lifecycle
  rather than socket read/decode.
- Update tactical 149 with the result and unblock its Slice 2 host-mode-honest
  projection if the remote client lane is now interpretable.

Validation:

```bash
pnpm native:android-xr:apk
git diff --check
```

Results:

- Built the Android XR release APK successfully:
  `android-xr/app/build/outputs/apk/release/app-release.apk`.
- Ran the documented Quest RD5 remote chunk-view churn lane through
  `--adb-reverse --start-server` against the local dedicated server. Output:
  - summary:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4-summary.txt`;
  - logcat:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4-logcat.txt`;
  - server log:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4-server.log`.
- Validation passed and the run emitted `MCLONE_ANDROID_XR_READY` plus
  `MCLONE_ANDROID_XR_PERF_SUMMARY`.
- Sample facts: `mode=chunk-view-churn`, `render_distance=5`,
  `render_compile_workers=2`, `sample_seconds=45.016`, `frames=2937`,
  `target_hz=72.0`, `budget_ms=13.889`, `conservation_violations=0`.
- The old ready-batch runtime drain is fixed for this lane:
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX` reported `poll_total_ms=2.127`,
  `drain_updates_ms=0.148`, `apply_updates_ms=2.112`,
  `dirty_mark_ms=1.890`, `client_apply_ms=2.061`, `updates=171`,
  `snapshot_updates=162`, `section_updates=23`, and `unload_updates=16`.
- Producer receive/decode work is now visible as off-frame producer work:
  `producer_read_ms=5766.112`, `producer_decode_ms=13.180`,
  `producer_response_sequence=20`.
- The remote lane is not yet clean enough to unblock tactical 149 host-mode
  projection. The worst frame moved to command send:
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` reported
  `max_total_ms=2770.917`, `max_send_ms=2770.910`,
  `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`, and zero
  updates. This means `SendOnly` no longer drains the response, but
  `send_command_only` can still wait for the IO actor to finish a previous
  response read before it writes/acks the next command.

Validation completed 2026-07-06:

```bash
pnpm native:android-xr:apk
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-remote-churn-rd5-slice4-summary.txt \
  --log /tmp/mclone-quest-openxr-remote-churn-rd5-slice4-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-remote-churn-rd5-slice4-server.log \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

## Slice 4A: Remote Command Send Queue Decoupling

Status: completed 2026-07-06.

Goal: make remote `SendOnly` and interest-command enqueue return without
waiting behind a previous response read.

Why: Slice 4 proved receive/decode moved out of runtime `poll()`, but the Quest
frame loop can still stall when `send_command_only` waits for the IO actor's
write acknowledgment while the actor is blocked reading the previous
one-response-per-command batch. This is still not reference-shaped enough: the
runtime should enqueue outbound commands and continue; transport flush and
paired response completion belong to the producer/adapter side.

Deliverables:

- Change the native remote command path so normal frame `SendOnly` does not
  wait for the IO actor to finish reading a prior response before returning.
  Either:
  - acknowledge once the command is accepted into the actor's ordered outbound
    queue, or
  - split the actor so outbound writes can progress while inbound reads block.
- Preserve command order and paired response accounting. `pending_response_batches`
  may remain adapter-local while the protocol is paired, but it must not force
  the app/runtime thread to wait for a prior response read.
- Preserve explicit failure behavior: disconnected/error state must surface on
  the next drain/poll/reconnect path instead of being silently dropped.
- Add a native loopback regression where command B is sent while command A's
  response is deliberately held; command B `SendOnly` must return quickly and
  no runtime poll sample may block on the held response.
- Re-run the Quest RD5 remote chunk-view churn lane or a narrowed equivalent
  and confirm `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` no longer
  reports multi-second `send_ms`.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client remote
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
git diff --check
```

Results:

- `NativeClientIoSession::send_command_only` now returns after accepting the
  command into the IO actor's ordered outbound queue. It no longer waits for a
  per-command write acknowledgment, so a prior response read cannot stall the
  runtime frame's interest-command send path.
- The IO actor still owns the `TcpStream`, preserves command/write order, reads
  the paired response batch after each write, queues decoded update batches, and
  marks disconnected on read/write/queue failure for the next drain/poll/reconnect
  path.
- Added
  `native_tcp_client_io_actor_queues_command_while_prior_response_is_held`, which
  deliberately holds command A's response, sends command B, verifies the second
  `SendOnly` returns in under `100ms`, and then drains both response batches in
  order.
- Updated
  `window_runtime_reuses_remote_session_for_interest_updates` to wait for the
  async actor/server handoff before teardown. The test now matches the new
  enqueue-only send contract instead of assuming synchronous socket write on
  return.
- Quest RD5 remote chunk-view churn rebaseline passed on Meta Quest 3. The Slice
  4 send-side stall is gone:
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` reported
  `max_total_ms=0.061`, `max_send_ms=0.021`,
  `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`, and zero updates.
- The receive/decode work remains producer-side:
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX` reported `poll_total_ms=2.134`,
  `drain_updates_ms=0.132`, `apply_updates_ms=2.091`,
  `producer_read_ms=6021.649`, `producer_decode_ms=13.114`,
  `updates=183`, `snapshot_updates=169`, `section_updates=22`, and
  `unload_updates=16`.
- Sample facts: `mode=chunk-view-churn`, `render_distance=5`,
  `render_compile_workers=2`, `sample_seconds=45.014`, `frames=3133`,
  `target_hz=72.0`, `budget_ms=13.889`, `conservation_violations=0`.
- Output:
  - summary:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-summary.txt`;
  - logcat:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-logcat.txt`;
  - server log:
    `/tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-server.log`.

Validation completed 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-native-client remote -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
pnpm native:android-xr:apk
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-summary.txt \
  --log /tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-remote-churn-rd5-slice4a-server.log \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

## Slice 5A: Promote ClientConnection To Shared App-Runtime Contract

Status: completed 2026-07-07.

Goal: make the native 151 seam a real shared contract before web adoption.
This is a move/refactor slice only.

Deliverables:

- Move `ClientConnection`, `QueuedServerUpdate`,
  `ClientConnectionDrainResult`, `ClientConnectionQueueMetrics`, drain mode,
  and the budgeted drain/apply helper out of `local_single_view.rs` into a
  wasm-compatible shared app-runtime module such as
  `mclone-app-runtime/src/client_connection.rs`.
- Make the API public or crate-visible exactly as needed for native scene
  runtimes and web adapters to use the same type/trait definitions. Do not make
  app crates define their own duplicate trait.
- Keep `LocalIntegratedConnection` and `RemoteDedicatedConnection` as native
  adapter implementations if that remains the smallest safe move; the contract
  and pump must no longer be private single-view helpers.
- Preserve all existing local integrated, native remote, desktop, Android, and
  Android XR behavior. No new queue, wire, reconnect, startup, or budget policy
  in this slice.
- Preserve and, if needed, relocate the existing tests for receive-order
  preservation across budget stalls and remote `SendOnly` behavior.
- Update this document with the exact module placement and the post-move audit
  classification.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
git diff --check
```

Exit criteria: shared contract module exists; `local_single_view.rs` no longer
owns the contract or pump; native local/remote behavior tests remain green;
web still compiles for wasm; remaining web divergence is classified for Slice
5B/5C.

Results:

- Added ungated shared module
  `native/crates/mclone-app-runtime/src/client_connection.rs`.
- Moved `ClientConnection`, `QueuedServerUpdate`,
  `ClientConnectionDrainResult`, `ClientConnectionQueueMetrics`,
  `ConnectionUpdateDrainMode`, and `pump_client_connection_updates_report` out
  of `local_single_view.rs`.
- Exposed the shared contract/pump through `mclone_app_runtime::client_connection`
  so future web adapters can implement the same trait instead of defining a
  web-local duplicate.
- Kept `LocalIntegratedConnection` and `RemoteDedicatedConnection` in
  `local_single_view.rs` as native adapter implementations. Their command,
  drain, startup, reconnect, diagnostics, budget, and pending-response
  behavior is unchanged.
- Relocated the receive-order/budget-stall pump regression to
  `client_connection::tests::shared_connection_pump_preserves_order_across_budget_stall`.

Preflight completed 2026-07-07 before editing:

```bash
git status --short
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
rg -n "NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

Preflight classification:

- Worktree was clean.
- `ClientConnection` and pump hits in `local_single_view.rs` were the expected
  pre-5A private single-view ownership.
- `WebRuntimeHost`, `exchange_command`, `try_recv_update`, and web
  `drain_updates` hits were the expected pre-5B/5C web divergence.
- `NativeClientSession` and legacy drain hits remained compatibility helpers,
  tests, app wrapper methods around `NativeClientIoSession`, and the standalone
  remote-player visual smoke helper.
- `pending_response_batches` remained remote connection-adapter response-paired
  protocol bookkeeping and tests.
- Existing warnings were unchanged: Android XR unused sentinel/helper in native
  app checks and wasm-only `with_unload_hysteresis_chunks` dead-code warning.

Post-move audit classification:

- `mclone-app-runtime/src/client_connection.rs`: owns the shared
  `ClientConnection` contract, queued update envelope, drain result/metrics,
  drain mode, pump helper, and the shared pump test.
- `mclone-app-runtime/src/local_single_view.rs`: remaining
  `ClientConnection`/`QueuedServerUpdate`/pump hits are native local/remote
  adapter implementations and runtime call sites using the shared pump.
- `mclone-app-runtime/src/local_single_view.rs`: remaining `try_recv_update`
  hit is the local integrated adapter receiving already decoded runner updates.
- `mclone-app-runtime/src/lib.rs`: remaining `drain_updates_ms` hits are
  diagnostics fields.
- `native/apps/mclone-web-client`: remaining `WebRuntimeHost`,
  `exchange_command`, `try_recv_update`, and `drain_updates` hits are the
  known web integrated/remote divergence for Slice 5B/5C.
- `mclone-net` / native app wrappers: `NativeClientSession` remains the
  low-level compatibility/test helper; normal desktop, Android, and Android XR
  remote wrappers remain on `NativeClientIoSession`.
- `pending_response_batches` remains remote connection-adapter
  response-paired protocol bookkeeping and tests.
- `pump_pending_remote_update_batches_report`: no remaining hits.

Validation completed 2026-07-07:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
git diff --check
```

## Slice 5B: Web Integrated Shared Ingress

Status: completed 2026-07-07.

Goal: make web integrated inline/worker play feed the shared
`ClientConnection` ingress without requiring native OS threads.

Deliverables:

- Adapt web integrated inline and worker/local-host paths so producer-side
  worker, inline host, or `SharedArrayBuffer` mechanics queue ordered
  `QueuedServerUpdate` envelopes.
- Route normal web integrated frame polling through the shared budgeted drain
  helper. Do not apply normal-frame updates directly from a returned
  `exchange_command` batch.
- Keep browser lifecycle, worker creation, JS glue, and any promise plumbing in
  the web adapter. The runtime-facing operation is enqueue command, then drain
  already queued decoded updates.
- Preserve startup and idle waits as explicit unlimited-drain paths.
- Add or update web integrated smoke/regression coverage proving command
  enqueue, producer queueing, and runtime update application are separate
  operations.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "WebRuntimeHost|exchange_command|ClientConnection|QueuedServerUpdate|drain_updates" native/apps/mclone-web-client native/crates/mclone-app-runtime
git diff --check
```

Exit criteria: web integrated normal-frame updates drain through the shared
connection path; remaining `exchange_command` hits are adapter/startup/test
helpers and are classified in this document.

Results:

- Implemented `ClientConnection` for `WebRuntimeHost`, with web inline and
  worker hosts queueing commands separately from draining decoded updates.
- Changed web integrated inline host to queue ordered `QueuedServerUpdate`
  envelopes with byte-depth diagnostics instead of returning command/update
  batches directly.
- Changed web integrated worker host to keep response frames as the producer
  queue, expose one-frame drains as `QueuedServerUpdate` envelopes, and report
  queued update bytes in diagnostics.
- Routed normal web integrated frame drains through
  `pump_client_connection_updates_report` using `RuntimeUpdatePumpBudget::default()`;
  removed the web-only `WEB_FRAME_UPDATE_DRAIN_BUDGET` count cap.
- Preserved startup/explicit immediate behavior by enqueueing commands, running
  an explicit unlimited shared pump, then applying command accounting with the
  final drained state.
- Kept web remote WebSocket behavior unchanged for Slice 5C.
- Made the shared pump timing wasm-safe with a wasm-only `js-sys::Date::now()`
  timing source, preserving elapsed-budget behavior without using
  `std::time::Instant` in the browser.
- Added
  `client_connection::tests::shared_connection_pump_preserves_drained_state_after_complete_response`
  so complete multi-update drains keep old batch-level drained semantics, while
  the existing budget-stall test still covers stalled drains.

Preflight completed 2026-07-07 before editing:

```bash
git status --short
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
rg -n "NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

Post-5B audit classification:

- `mclone-app-runtime/src/client_connection.rs`: owns the shared
  `ClientConnection` contract, `QueuedServerUpdate`, drain result/metrics, drain
  mode, wasm-safe shared pump timing, pump helper, and pump regression tests.
- `mclone-app-runtime/src/local_single_view.rs`: remaining
  `ClientConnection`/`QueuedServerUpdate`/pump hits are native local/remote
  adapter implementations and runtime call sites using the shared pump.
- `native/apps/mclone-web-client/src/lib.rs`: `WebRuntimeHost` is the web host
  adapter and now implements `ClientConnection`; `RemoteWebSocket` arms remain
  compatibility placeholders for Slice 5C.
- `native/apps/mclone-web-client/src/web_server_worker.rs`: remaining
  `exchange_command` hits are compatibility/probe helpers; normal web
  integrated runtime/frame draining uses queued updates and the shared pump.
- `native/apps/mclone-web-client/src/web_remote_session.rs`: remaining
  `exchange_command` is the unchanged web remote path for Slice 5C.
- Remaining `drain_updates` hits are app-runtime diagnostics fields, native
  local/remote runtime drain accounting, and the web worker `IntegratedServerRunner`
  compatibility method; normal web integrated frame polling no longer drains by
  applying an `exchange_command` batch.

Validation completed 2026-07-07:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "WebRuntimeHost|exchange_command|ClientConnection|QueuedServerUpdate|drain_updates" native/apps/mclone-web-client native/crates/mclone-app-runtime
git diff --check
```

Known unchanged warning: wasm `mclone-server` check/smoke still reports the
`with_unload_hysteresis_chunks` dead-code warning.

## Slice 5C: Web Remote Shared Ingress

Status: completed 2026-07-07.

Goal: make web remote WebSocket sessions feed the same shared ingress as native
remote TCP.

Deliverables:

- Adapt web remote WebSocket message/promise callbacks so producer-side code
  decodes or receives `ServerUpdate`s, attaches queue metadata where available,
  and queues ordered `QueuedServerUpdate` envelopes.
- Route normal web remote frame polling through the shared budgeted drain
  helper. A normal frame must not await a WebSocket response promise to apply
  updates.
- Preserve disconnect/error reporting and reconnect behavior. If a host-side
  metric is unavailable in web remote, report it as unavailable rather than
  zero.
- Keep the current wire protocol unless a blocker is recorded and user-reviewed;
  server-push broadening remains tactical 133.
- Add or update web remote smoke/regression coverage for command enqueue,
  producer queueing, ordered apply, and a held/delayed response that does not
  block normal frame polling.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "WebRuntimeHost|exchange_command|ClientConnection|QueuedServerUpdate|drain_updates" native/apps/mclone-web-client native/crates/mclone-app-runtime
git diff --check
```

Exit criteria: web remote normal-frame updates drain through the shared
connection path; no normal frame path awaits response decode/apply; remaining
web compatibility helpers are classified.

Results:

- Refactored `WebSocketServerSession` so WebSocket message callbacks decode
  server update batches producer-side, track response sequence/queue metrics, and
  queue ordered `QueuedServerUpdate` envelopes for the shared app-runtime pump.
- Split web remote command sending into deferred `queue_command` and explicit
  `send_command_acknowledged` paths. Immediate commands still wait for their
  paired response before the unlimited shared drain, preserving startup and
  reconnect/resync behavior.
- Routed `WebRuntimeHost::RemoteWebSocket` through the shared
  `ClientConnection` enqueue/drain/metrics implementation instead of returning a
  command-response `WebSocketExchange` batch.
- Changed the normal web remote streaming frame path to enqueue chunk-view
  interest and drain already queued updates through
  `pump_client_connection_updates_report`; normal frames no longer await a
  WebSocket response promise to apply updates.
- Preserved the current WebSocket wire protocol, response pairing, disconnect
  and error reporting, diagnostics queue-depth meanings, and explicit remote
  reconnect/resync path.
- Added
  `client_connection::tests::shared_connection_pump_reports_pending_response_without_blocking_ready_only_drain`
  to cover the held/delayed response case: ready-only frame polling reports a
  pending response without blocking or applying out-of-order data.
- Fixed a pre-existing `mclone-dedicated-server` diagnostics compile error by
  removing `Copy`/`Eq` from `DedicatedSessionDiagnostics` and cloning the
  non-`Copy` publication diagnostics value. This was required to run the extra
  remote WebSocket app-loop smoke and does not change runtime behavior.

Preflight completed 2026-07-07 before editing:

```bash
git status --short
rg -n "trait ClientConnection|struct QueuedServerUpdate|pump_client_connection_updates_report|WebRuntimeHost|exchange_command|try_recv_update|drain_updates" native/crates/mclone-app-runtime native/apps/mclone-web-client
rg -n "NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

Post-5C audit classification:

- `mclone-app-runtime/src/client_connection.rs`: owns the shared
  `ClientConnection` contract, `QueuedServerUpdate`, drain result/metrics, drain
  mode, wasm-safe shared pump timing, pump helper, and pump regression tests.
- `mclone-app-runtime/src/local_single_view.rs`: remaining
  `ClientConnection`/`QueuedServerUpdate`/pump hits are native local/remote
  adapter implementations and runtime call sites using the shared pump.
- `native/apps/mclone-web-client/src/lib.rs`: `WebRuntimeHost` is the web host
  adapter and all host variants, including `RemoteWebSocket`, implement the
  shared `ClientConnection` boundary.
- `native/apps/mclone-web-client/src/web_remote_session.rs`: no remaining
  `exchange_command`/`WebSocketExchange` path; message callbacks queue decoded
  updates and normal drains expose `ClientConnectionDrainResult`.
- `native/apps/mclone-web-client/src/web_canvas.rs`: normal remote streaming
  frame polling enqueues chunk-view interest and drains queued updates; it does
  not await the WebSocket response promise.
- `native/apps/mclone-web-client/src/web_server_worker.rs`: remaining
  `exchange_command`/`drain_updates` hits are compatibility/probe helpers for
  the web integrated worker adapter, not normal runtime frame polling.
- Remaining `drain_updates` hits are app-runtime diagnostics fields, native
  local/remote runtime drain accounting, and compatibility/test helpers.

Validation completed 2026-07-07:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-dedicated-server
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "WebRuntimeHost|exchange_command|ClientConnection|QueuedServerUpdate|drain_updates" native/apps/mclone-web-client native/crates/mclone-app-runtime
git diff --check
```

Known unchanged warning: wasm `mclone-server` check/smoke still reports the
`with_unload_hysteresis_chunks` dead-code warning.

Extra validation note: `pnpm native:web:remote-smoke` is not a named Slice 5C
gate, but it was attempted after edits. After the dedicated diagnostics compile
fix, the current worktree and a temporary pre-Slice-5C worktree at `5c5e6c38`
with the same diagnostics fix both failed the full remote app-loop probe at the
same place-interaction target miss (`place: miss`, `commandSent=false`). This is
classified as a pre-existing remote app-loop probe issue, not a Slice 5C shared
ingress regression.

## Slice 5D: Close-Out Audit

Status: completed 2026-07-07.

Goal: close tactical 151 once native and web all use one runtime-facing
connection ingress.

Deliverables:

- Run the full audit for duplicate normal-frame connection shapes:
  `ClientConnection`, `QueuedServerUpdate`, `WebRuntimeHost`,
  `exchange_command`, `NativeClientSession`, legacy drain helpers, and
  `pending_response_batches`.
- Confirm every remaining duplicate-looking hit is adapter internals, startup,
  compatibility, or test code. No normal frame runtime pump may use a
  host-specific command-response apply path.
- Update `session-network-architecture.md`, tactical 133, tactical 149 if
  needed, this tactical, and the tactical index with the final state.
- Decide whether any remaining response-paired protocol head-of-line blocking
  belongs in tactical 133, not this tactical.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "ClientConnection|QueuedServerUpdate|WebRuntimeHost|exchange_command|NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-web-client native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

Exit criteria: tactical 151 status marked closed with evidence; one shared
client connection ingress is used by native local, native remote, web
integrated, and web remote normal-frame paths; remaining server-push or protocol
broadening ideas are handed to tactical 133 or a new tactical.

Results:

- Ran the full duplicate-shape audit across shared app-runtime, native net,
  native desktop/Android/Android XR wrappers, and web adapters.
- Confirmed one normal-frame runtime pump:
  `pump_client_connection_updates_report` in
  `mclone-app-runtime/src/client_connection.rs`.
- Confirmed native local, native remote TCP, web integrated inline/worker, and
  web remote WebSocket normal-frame paths all implement and drain through
  `ClientConnection`.
- Confirmed desktop, Android, and Android XR remote app wrappers hold
  `NativeClientIoSession`; `NativeClientSession` remains only the low-level
  native-net compatibility/test helper and the standalone native remote-player
  visual smoke helper.
- Confirmed remaining web `exchange_command` hits are
  `WebIntegratedServerRunner` compatibility/probe helpers. Normal web runtime
  frame polling uses queued updates through `WebRuntimeHost`'s
  `ClientConnection` implementation.
- Confirmed remaining `pending_response_batches` is adapter bookkeeping for
  the current one-response-per-command wire protocol, not runtime-thread socket
  ownership.
- Updated `session-network-architecture.md`, tactical 133, tactical 149, this
  tactical, and the tactical index with the final state.
- Classified remaining response-paired protocol head-of-line/server-push
  broadening as tactical 133 Slice 6 work, not tactical 151 work.

Final audit classification:

- `mclone-app-runtime/src/client_connection.rs`: canonical shared
  `ClientConnection`, `QueuedServerUpdate`, drain result/metrics, drain mode,
  wasm-safe pump timing, shared budgeted pump helper, and pump tests.
- `mclone-app-runtime/src/local_single_view.rs`: native local and remote
  adapter implementations plus normal runtime call sites using the shared pump.
  `try_recv_update` is local integrated adapter receive of already decoded
  runner updates.
- `mclone-net`: `NativeClientIoSession` is the normal native TCP client IO
  actor; `NativeClientSession` and legacy drain helpers remain low-level
  compatibility/test helpers.
- Native desktop, Android, and Android XR app wrappers: remote sessions hold
  `NativeClientIoSession` and implement the shared remote-session trait used by
  app-runtime's `RemoteDedicatedConnection`.
- `native/apps/mclone-web-client/src/lib.rs`: `WebRuntimeHost` is the web host
  adapter and implements `ClientConnection` for inline, worker, and remote
  WebSocket hosts.
- `native/apps/mclone-web-client/src/web_remote_session.rs`: WebSocket callback
  adapter queues decoded updates and exposes drain metrics; no
  `exchange_command`/`WebSocketExchange` normal path remains.
- `native/apps/mclone-web-client/src/web_server_worker.rs`: remaining
  `exchange_command`, `try_recv_update`, and `drain_updates` hits are worker
  compatibility/probe helpers, not normal runtime frame polling.
- `pump_pending_remote_update_batches_report`: no remaining hits.

Validation completed 2026-07-07:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
rg -n "ClientConnection|QueuedServerUpdate|WebRuntimeHost|exchange_command|NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-web-client native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

Known unchanged warnings: native app checks still report the existing Android
XR unused sentinel/helper warnings; wasm web check/smoke still reports the
existing `with_unload_hysteresis_chunks` dead-code warning.

## Out Of Scope

- Server-push protocol broadening. Keep it in tactical 133 Slice 6 after the
  client inbound boundary exists.
- Post-closeout adapter naming/quarantine cleanup. Keep it in tactical
  [`154-client-ingress-adapter-cleanup.md`](154-client-ingress-adapter-cleanup.md)
  so this closed tactical remains the implementation/evidence record.
- Compression or binary protocol redesign.
- Chunk meshing, GPU upload, render admission, or terrain coordinator policy.
  Those remain in tactical 128 and related performance trackers.
- Multiplayer authority/correctness beyond preserving ordered command/update
  delivery.
- Priority/coalescing lanes for chunk streaming.

## Open Questions

- Native IO actor queue shape: resolved in this tactical by queuing decoded
  updates with byte/age/read/decode metadata before the runtime pump.
- Queue bounding: depth and bytes are exposed; hard caps remain future work and
  should land only with a clear disconnect/error path.
- Response-paired wire protocol: handed to tactical 133 Slice 6. Tactical 151
  intentionally stopped at a shared client ingress over the current wire
  protocol.
