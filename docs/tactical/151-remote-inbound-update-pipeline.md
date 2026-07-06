# 151: Remote Inbound Update Pipeline

Status: active; Slice 0 documentation baseline completed 2026-07-06; Slice 1
shared `ClientConnection` runtime seam completed 2026-07-06; Slice 2 native
TCP client IO actor completed 2026-07-06. Slice 3 is next: surface remote
producer/read/decode diagnostics through runtime accounting and re-tighten the
remote frame budget contract. This is the focused successor to tactical
[`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
for the remaining remote ready-batch stall. Architecture target:
[`../session-network-architecture.md`](../session-network-architecture.md).
Umbrella bus work: tactical
[`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md).

Workstream: native Rust shared session/runtime boundary, native TCP transport,
Android XR validation. Goal: introduce one shared client-facing connection
abstraction for local integrated, native remote, and web transports, then make
remote dedicated inbound updates arrive through that same budgetable update
boundary without making the runtime/render frame read and decode TCP response
batches.

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
- Desktop, Android, and Android XR normal remote session wrappers hold
  `NativeClientIoSession`, not `NativeClientSession`.
- `RemoteDedicatedSingleViewSceneRuntime` still has a temporary
  `pending_response_batches` counter inside its connection adapter because the
  wire protocol remains one response per command. That counter no longer means
  "socket read pending on the runtime thread"; it is adapter bookkeeping for
  ordered response completion.

Server-side dedicated already has an accept thread and a connection thread per
client. The remaining pieces are runtime diagnostics/projection cleanup, Quest
rebaseline, and web callback adoption behind the same client-facing connection
abstraction.

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

## Slice Rules

- One slice per session.
- At the end of each slice: update this document with status/evidence, run the
  named validation, commit, and state the next step.
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

## Slice 4: Quest Remote Churn Rebaseline

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

## Slice 5: Web Callback Adoption

Goal: make web remote feed the same inbound queue without requiring native OS
threads.

Deliverables:

- Route WebSocket `message` callbacks through the same ordered inbound envelope
  type and queue metadata.
- Keep browser lifecycle, promises, and JS glue in the web app/adapter.
- Add a web remote smoke proving command enqueue and inbound update application
  are separate runtime operations.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

## Out Of Scope

- Server-push protocol broadening. Keep it in tactical 133 Slice 6 after the
  client inbound boundary exists.
- Compression or binary protocol redesign.
- Chunk meshing, GPU upload, render admission, or terrain coordinator policy.
  Those remain in tactical 128 and related performance trackers.
- Multiplayer authority/correctness beyond preserving ordered command/update
  delivery.
- Priority/coalescing lanes for chunk streaming.

## Open Questions

- Should the first native IO actor queue decoded `ServerUpdate`s directly, or
  queue encoded per-update frames and decode under a producer budget on the IO
  actor? Default answer: decode on the IO actor first, because local integrated
  already queues decoded updates and the current stall is measured as
  runtime-side ready-batch drain/decode.
- Should the queue be bounded by bytes or by updates? Default answer: expose
  both; fail or disconnect only after a conservative byte cap is added with a
  clear error path.
- Does the response-paired wire protocol force head-of-line blocking that
  matters after the client IO actor lands? Measure first; server-push belongs
  to a later slice only if the new queue still shows source-shaped bursts.
