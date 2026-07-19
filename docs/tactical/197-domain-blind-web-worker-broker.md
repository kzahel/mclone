# Tactical 197: Domain-Blind Web Worker Broker

Status: autonomous high-value continuation approved 2026-07-19 after Slice 2
review. Drive worker-side render ownership, proven common Worker mechanics,
opaque integrated-server startup, bounded authority/session policy, and
Rust-authored catalog descriptors as atomic cuts. Then remeasure and stop for a
human decision before the browser-native long tail. This campaign must not
widen into a shared-Wasm-memory runtime.

Topic: `web-worker-runtime-ownership`

Workstream: native web/WASM browser adapter, browser-specific Rust worker
adapters, and TypeScript Worker mechanics. Shared Rust engine owners remain in
their existing crates; desktop/native channel transports remain unchanged.

## Result

Reduce native-web platform divergence by making browser Worker TypeScript
domain-blind while retaining the safety and failure containment of independent
Wasm heaps.

The first completed result should have:

- a whole-`www/` executable ownership inventory rather than a check limited to
  the main web app;
- worldgen and light-status Workers configured and dispatched by a
  worker-resident Rust actor rather than a TypeScript domain switch;
- a generic TypeScript mailbox that only loads Wasm, forwards opaque frames or
  explicit SAB handles, publishes completion/failure, and yields to the browser;
- unchanged external `SharedArrayBuffer` pools, codecs, copies, and metrics;
- no shared Wasm linear memory, new toolchain lane, or cross-worker Rust
  lifetime; and
- a proven path for moving the larger render compiler broker into
  browser-specific Rust next.

The tactical is an ownership refactor first. It does not claim a performance
win merely because TypeScript shrinks.

## Motivation

The completed scene-host work established one Rust policy owner, but the
browser adapter has continued to grow around it. Authored TypeScript now totals
7,417 lines across cadence, input, storage, server, render, and worker modules.
Much of that is legitimate browser mechanics. Some of it is increasingly rich
domain coordination:

- `mclone-server-job-worker.ts` selects worldgen versus light-status Rust
  entrypoints and owns the resident worldgen-session choice;
- `mclone-render-compiler-worker.ts` enumerates render and Far LOD fields,
  selects compiler methods, and owns per-world compiler sessions;
- `mclone-render-compiler-shared.ts` owns an active/standby broker, pending
  request maps, release queues, timeouts, and large hand-authored request
  schemas;
- `mclone-integrated-server-worker.ts` projects generation profile, topology,
  behavior, persistence, job-worker, cadence, and diagnostic facts; and
- `mclone-web-app.ts` relays compiler doorbells, promises, timing, asset-epoch
  swaps, and platform-operation completions.

Existing ownership tests prevent known duplicated terrain, scenario, and scene
policy. They do not yet express the broader rule that TypeScript should not
choose domain algorithms or maintain domain lifecycles simply because browser
workers need a JavaScript entry module.

## Current Transport Must Survive

The current production Workers use separate Wasm memories connected by
explicit byte transports:

```text
main Wasm private heap
  <-> explicit SharedArrayBuffer mailbox / postMessage envelope
worker Wasm private heap
```

The `shared-memory` label in runtime reports refers to the external mailbox,
not a shared Rust heap. The first slices keep:

- the stable `wasm32-unknown-unknown` build;
- current cross-origin isolation;
- resident SAB request/result arenas and bounded pools;
- message-transfer fallbacks where they are currently supported;
- all existing encode/copy/decode behavior;
- resident worldgen and render mirrors;
- WebGPU upload and presentation on the browser main thread; and
- explicit Worker restart/termination containment.

This deliberately preserves the safer isolation model while moving policy and
state-machine ownership toward Rust.

## Target Boundary

### Generic TypeScript worker broker

The broker may understand:

- Wasm JavaScript and `.wasm` URLs;
- one opaque actor initialization frame;
- request IDs and generic success/failure;
- `Uint8Array`, transfer lists, and explicit `SharedArrayBuffer` handles;
- mailbox control words needed to publish ready/complete/failure;
- Worker messages, browser errors, timers, and close; and
- generic byte/capacity/queue metrics.

The broker must not understand:

- worldgen versus light algorithms;
- generator profiles or topology variants;
- render sections versus Far LOD;
- active versus standby world priority;
- realm/dimension/scenario meaning;
- asset-selection policy;
- revisions, epochs, retries, or stale-result policy; or
- the fields of a domain request merely to reconstruct another domain request.

### Worker-resident Rust actor

The Rust actor owns:

- decoding its opaque initialization and request frames;
- the lane/domain variant and dispatch;
- resident caches and sessions;
- typed errors and shutdown state;
- domain metrics; and
- the encoded result frame.

Browser-specific Rust wrappers may live in `mclone-web-client`; engine behavior
and reusable state machines remain in their shared crates. Do not move browser
`Worker`, `JsValue`, IndexedDB, or WebGPU types into `mclone-server`,
`mclone-scene`, `mclone-render-session`, or other neutral owners.

## Invariants

1. `McloneSceneHost` remains the only scene/runtime policy host.
2. Native workers keep typed moved-value/channel paths; they do not adopt the
   browser byte ABI for cosmetic symmetry.
3. TypeScript remains responsible for browser event-loop mechanics and never
   blocks the main browser thread.
4. A Rust actor never receives DOM, WebGPU, or arbitrary `JsValue` handles as
   domain data.
5. Each production Worker retains its private Wasm heap through this tactical.
6. No shared Rust allocator, `Arc`, mutex, or pointer crosses Workers.
7. Existing codecs and payload reductions remain byte-compatible until a
   separately measured slice intentionally changes them.
8. Production does not retain a long-lived old/new broker toggle. A slice may
   build a direct proof beside production, but cutover deletes the replaced
   owner atomically.
9. Worker crashes and forced termination remain reconstructable because no
   other Worker trusts their heap.
10. TypeScript line reduction is secondary to executable ownership and behavior
    evidence.

## Slice 0 — Whole-Worker Ownership And Copy Baseline

Status: complete 2026-07-19.

Add a new executable inventory, preferably
`scripts/check-web-worker-ownership.mjs`, and expose it as a package command.
Do not stretch the completed scene-host checker into a catch-all; keep its
historical scene-policy assertions intact and have the new checker call or
complement it.

The inventory covers every authored production/runtime `.ts` module under
`native/apps/mclone-web-client/www/` and records:

- source lines by responsibility family;
- Worker construction sites and worker entry modules;
- hand-authored inbound/outbound message interfaces;
- TypeScript domain selectors and state machines;
- Rust wasm-bindgen constructors/entrypoints each Worker invokes;
- external SAB ABI sources and their Rust lock tests;
- message-transfer fallbacks;
- resident worker state; and
- the current per-lane copy/codec path.

The checker should distinguish allowed browser mechanisms from domain-aware
debt. It should fail on new unregistered domain selectors or a new worker entry
module, not merely on any line-count increase. A small checked-in manifest of
approved worker modules/responsibilities is acceptable if the script verifies
it against the filesystem and source patterns.

Lock the following current facts:

- production engine Workers have private Wasm linear memories;
- the thread smoke is the only current shared-Wasm-memory proof;
- runner, worldgen, light, and render `shared-memory` metrics refer to external
  SAB mailboxes;
- server-job TypeScript currently selects `worldgen`/`light-status` and
  `WebWorldgenJobSession`;
- render TypeScript currently owns `RenderSectionWorkerCompiler`, the
  active/standby queue, per-world compiler sessions, and work-kind dispatch;
- integrated-server TypeScript currently owns browser cadence/IndexedDB/job
  servicing plus substantial descriptor projection; and
- the current 7,417-line authored TypeScript inventory matches the topic's
  responsibility table or is intentionally refreshed.

Add a compact before/after metric surface for later slices. Reuse existing
runtime reports instead of inventing parallel counters where possible:

- request/response bytes and frames by lane;
- shared-pool hit/miss/overflow/fallback counts;
- p50/p95 or sample ranges for worker round trip;
- maximum frame gap;
- worker init/restart/shutdown counts;
- resident mirror/session counts; and
- pending queues at settle.

### Slice 0 acceptance

- The new command passes on the current tree and produces machine-readable
  `--json` output.
- Corrupting one registered responsibility or adding a synthetic Worker module
  makes the checker fail with an actionable message.
- Existing scene-host, generator-profile, scenario, and SAB ABI locks still
  run unchanged.
- No production runtime behavior or transport changes.
- Baseline local Worker, chunk, movement, remote, and lobby scenario smokes
  remain green; evidence and screenshots stay under `/tmp`.

### Slice 0 review

Human review is useful here because the allowed-mechanism/debt classification
becomes the architectural contract. Implementation can proceed autonomously
after that classification is accepted.

### Slice 0 evidence

`pnpm native:web:worker-ownership` now inventories all 14 authored TypeScript
modules, six Worker entries, two TypeScript construction sites, 7,417 baseline
lines, registered domain debt, eight explicit copy sites, and the external SAB
ABI locks. Its self-test rejects an unregistered Worker entry, growth in a
registered domain selector, and production shared-Wasm-memory vocabulary.
Machine-readable output is available through `--json`.

The thin-adapter purity gate, web typecheck, local app, chunk, movement, and
lobby scenario probes passed. App, movement, and lobby captures were inspected
under `/tmp`. The remote WebSocket scenario reached normal shared-memory
rendering and settled its compiler queue, but its later block-break interaction
timed out twice on the unchanged baseline. Because Slice 0 changes only checks
and package wiring, this is recorded as a pre-existing red scenario rather than
attributed to the ownership gate. Slice 1 must preserve that exact boundary and
must not claim the remote scenario green without new evidence.

The baseline cleanup below later established that the old interaction report
proved command submission, not an authoritative mutation. This paragraph is a
historical record of the observed red run, not causal mutation evidence.

## Slice 1 — Server-Job Rust Actor Proof

Status: complete 2026-07-19; awaiting boundary review before Slice 2.

Use `mclone-server-job-worker.ts` as the first bounded proof. It is small, has
two domain variants, already uses isolated Wasm instances, and carries both SAB
and message-transfer mechanics. It exercises the intended boundary without
touching scene, WebGPU, IndexedDB, or authoritative tick cadence.

Add a browser-specific wasm-bindgen wrapper such as `WebServerJobActor` in
`mclone-web-client`:

```text
WebServerJobActor::new(opaque_init_frame)
WebServerJobActor::compute_frame(request_bytes) -> response_bytes
WebServerJobActor::diagnostics() -> typed/encoded diagnostics
WebServerJobActor::shutdown()
```

Exact names may change, but the ownership may not.

Main Rust creates the opaque initialization frame when it configures the
separate worldgen and light Workers. Worker Rust decodes the actor variant.
The actor owns the existing resident `WorldgenJobSession` for worldgen and the
light-status dispatch for the light worker. TypeScript calls one actor method
and does not branch on a lane kind.

After cutover, `mclone-server-job-worker.ts` may retain:

- dynamic bindgen module loading;
- generic initialization;
- SAB control/request/response views and atomic publication;
- message-transfer fallback;
- request-ID forwarding;
- generic error serialization; and
- Worker close.

It must no longer contain:

- `worldgen` or `light-status` algorithm selection;
- `WebWorldgenJobSession`;
- `mclone_web_compute_light_status_job_frame`;
- a domain result kind assembled from the request kind; or
- knowledge of the resident dependency mirror.

The existing SAB control-word ABI may remain TypeScript-visible because it is
transport mechanics and is already locked to Rust. Do not bundle an ABI
redesign or copy reduction into this slice.

### Slice 1 acceptance

- Worldgen and light worker selection is authored once in Rust.
- The TypeScript worker passes the new domain-blind ownership checker.
- `WorldgenJobSession` remains resident across jobs and its delta/desync tests
  remain byte-exact.
- Worldgen and light request/response bytes, transport kind, pool behavior,
  fallback probes, and mailbox-kind diagnostics match the Slice 0 baseline.
- Generated chunks, light payloads, and the deterministic browser canvas remain
  unchanged.
- Worker failure returns a typed Rust failure through the generic broker, marks
  the SAB failure word, and can be followed by a clean worker reconstruction.
- Native worldgen/light mailboxes and their channel transports are untouched.

### Slice 1 validation

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:thread-smoke
pnpm native:web:chunk-smoke
pnpm native:web:movement-perf
pnpm native:web:app-smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
```

Inspect the chunk/app browser capture under `/tmp` because the worker output
feeds rendered terrain, even though the intended change is ownership-only.

### Slice 1 evidence

Main Rust now selects `ServerJobActorKind`, encodes a strict versioned nine-byte
initialization frame, and gives it to each isolated job Worker. Worker Rust
decodes that frame into `ServerJobActor`, owns the resident
`WorldgenJobSession` or light-status dispatch, counts completed/failed frames,
exports opaque diagnostics, and has explicit shutdown state. Tests cover bad
magic/version/kind, warm worldgen session reuse, byte-identical light output,
typed failure after shutdown, and clean actor reconstruction.

`mclone-server-job-worker.ts` now has one `WebServerJobActor` and one
`computeFrame` call. It has no worldgen/light selector, domain constructor,
light entrypoint, or domain-derived result kind. The ownership inventory records
all four server-job debt counts at zero. Authored TypeScript fell from the
7,417-line baseline to 7,410 lines; the eight-copy ledger, external SAB ABI,
isolated Wasm heaps, transfer fallback, and native channel mailboxes are
unchanged.

The shared server suite passed 512 tests; the web client tests, wasm target
check, generated-bindgen TypeScript check, thread/chunk/app/movement browser
probes, native movement/timedemo probes, and lobby A-to-B-to-A scenario passed.
The representative movement probe retained four worldgen frames and 2,886,884
request bytes, 37,230,612 response bytes, all pooled responses, and zero pending
jobs. Its maximum browser frame gap was 10.325 ms. Chunk, app, movement, and
lobby-return captures were inspected under `/tmp`.

The remote probe again made the authoritative block mutation successfully but
timed out waiting for an additional mesh build, matching the Slice 0 baseline
failure. All other remote interaction checks and the compiler settled state
were green. This remains tracked as pre-existing validation debt, not as Slice
1 evidence of a green remote path.

The baseline cleanup below supersedes that interpretation. The then-current
probe called successful command enqueue `changed` and could associate unrelated
updates or compiler work with the interaction; it did not prove an
authoritative mutation. Repeated causal runs exposed a real intermittent
WebSocket disconnect.

### Slice 1 review

No product-design review should be necessary if byte, timing, failure, and
pixel evidence remain equivalent. Review the Rust/TypeScript boundary itself
before using it as the render template.

## Validation Baseline Cleanup

Status: complete 2026-07-19; no Slice 2 work included.

The Slice 0/1 review stop first required resolving every supposedly known red
baseline. Investigation found two coupled problems:

- gameplay command APIs returned a `changed` boolean even though successful
  local and remote dispatch only proved submission; updates drained near the
  command were not a command-specific acknowledgement; and
- the dedicated WebSocket adapter treated `WouldBlock` from a nonblocking
  Tungstenite send/flush as fatal. Under kernel backpressure the authoritative
  server could apply a command, disconnect before publishing the update, and
  leave the browser waiting indefinitely.

The runtime now returns a `GameplayCommandSubmission` receipt with timing only.
Native, Android, XR, offscreen, and web callers report submission rather than
claiming a world change. Browser interaction reports no longer synthesize a
result block state or update count from enqueue success.

The block interaction smoke now records the exact candidate states before
submission, waits for the expected authoritative block state in the client
replica and a newer section-block update, then waits for an accepted remesh and
an idle compiler. It invokes the explicit web interaction adapter so pointer
gesture thresholds cannot masquerade as authority failures; physical input is
covered separately by the app input probes.

The WebSocket writer now retains a pending-flush state when Tungstenite reports
`WouldBlock` and retries its already-buffered frame on a later connection-loop
turn. A deterministic mock stream forces one blocked write and proves that the
connection survives and the frame is subsequently written. The pre-existing
`rustfmt` drift in the composable-world presentation contract was normalized as
part of restoring a genuinely green baseline.

### Cleanup evidence

- `cargo fmt --all --check` passed.
- the app-runtime, scene, and web-client suites passed; the dedicated-server
  suite passed all 42 tests, including the forced-`WouldBlock` regression;
- `cargo check --workspace`, the Wasm build/typecheck, Worker ownership gate,
  and scene-host adoption gate passed;
- the local block-edit proof observed dirt state `5`, a section update and
  remesh, followed by air state `0`, another section update and remesh;
- five consecutive fresh remote WebSocket smokes passed the same authoritative
  placement/break and remesh contract with settled compiler queues; and
- the local block-edit and final remote canvas captures were inspected under
  `/tmp` and remained visually correct.

Authored TypeScript is now 7,413 lines. The three-line increase from the Slice
1 result is the clearer submitted/not-submitted interaction status; the four
server-job domain-debt counters remain zero and the copy ledger is unchanged.
The baseline is green, but the existing human review gate still stops the
workstream before Slice 2.

## Slice 2 — Rust-Owned Single Render-Worker Coordinator

Status: complete 2026-07-19; stopped at the required review gate.

Move the main-side `RenderSectionWorkerCompiler` policy and queue from
`mclone-render-compiler-shared.ts` into Rust while leaving the worker-side
compiler session and current SAB wire shape unchanged. “Single render-worker
coordinator” means one main-thread coordinator arbitrates the existing ordinary
render Worker across active and standby worlds. It does not mean the browser
`SharedWorker` API, shared Wasm linear memory, or one Worker per world.

The browser boundary is a generic, polled TypeScript transport:

```text
post(opaque message, optional transfer handles)
poll() -> opaque message | browser error | empty
terminate()
```

The transport may construct an ordinary browser `Worker`, retain opaque event
data until Rust polls it, forward transfer lists and SAB handles, and terminate
the Worker. It must not interpret render work, worlds, priorities, epochs,
retries, releases, or compiler sessions. Rust polls at a known frame/runtime
boundary rather than accepting asynchronous JavaScript callbacks into an
arbitrary mutable Rust owner.

Add a Rust coordinator around the existing `WebRenderSectionCompiler`, which
already owns resident input/result arenas, mirror tracking, local request IDs,
revisions, and completion polling. The coordinator owns:

- worker generation and readiness;
- active/standby admission and the single-in-flight queue;
- broker request identity and request-to-world association;
- stale completion and failure rejection;
- quiescent world release ordering;
- asset-epoch candidate/activation/rollback state;
- timeout and reconstruction policy; and
- compile timing and diagnostic semantics.

Keep the transport mechanism distinct from coordinator policy. Native may use
typed moved-value/channel transports while web uses opaque frames and explicit
SAB handles. Share a deterministic Rust coordinator/state machine wherever the
semantics genuinely match; do not make native adopt the browser ABI for
cosmetic symmetry.

`mclone-web-app.ts` should stop owning:

- the `RenderSectionWorkerCompiler` instance;
- `wakeRenderCompiler` and compile promise maps;
- render request/world timing association;
- compiler release messages;
- active/standby broker policy; and
- compiler asset-epoch rollback decisions already represented by Rust host
  state.

It may continue to fetch browser assets and pass bytes/URLs into Rust service
construction. This slice does not yet require the worker entry module itself
to be domain-blind.

### Slice 2 implementation sequence

1. Lock this contract and the source ownership rules.
2. Add and directly prove the generic polled Worker transport without changing
   production render ownership.
3. Add the deterministic Rust coordinator and fake-transport traces for
   active/standby order, stale completion, worker failure, world release, and
   asset replacement.
4. Cut production over atomically and delete the TypeScript queue, pending
   maps, promises, timeouts, world-release policy, asset-compiler swap policy,
   and compiler-wake relay in the same ownership commit.
5. Run the complete applicable browser/native matrix, compare performance and
   transport metrics, inspect captures, update evidence, and stop before Slice
   3.

Preparatory contracts and direct proofs may land separately. Production must
not retain joint Rust/TypeScript request ownership or a long-lived old/new
toggle after cutover.

### Slice 2 acceptance

- One Rust owner performs render worker admission and request association.
- One generic TypeScript transport owns only Worker construction, opaque
  post/poll, browser errors, transfer handles, and termination.
- No JavaScript callback crosses into `WebRenderCompilerWakeSink`; the Rust
  owner polls transport events at an explicit runtime boundary.
- Local, IndexedDB, remote, and two-world lobby modes use the same broker.
- Active-world compilation remains preferred over standby work without a
  TypeScript policy queue.
- World release and asset-epoch replacement cannot complete against a stale
  worker/session.
- Render input/result bytes, single-in-flight budget, round-trip range, maximum
  frame gap, mirror counts, overflow, and fallback metrics match baseline.
- Far LOD, movement, hidden/resume, mobile throttling, and lobby A-to-B-to-A
  smokes remain green.
- Browser captures are inspected at the first drawable cutover and after asset
  replacement/lobby composition.

This is the first materially risky slice. It should cut over atomically after a
direct proof and delete the TypeScript owner in the same commit.

### Slice 2 autonomous stop conditions

Stop for a new decision rather than broadening this slice if implementation
requires shared Wasm linear memory, a general browser threading runtime, a new
SAB ABI or copy strategy, changed mesh algorithms, changed native scheduling
for browser symmetry, more than one production render Worker, a long-lived
dual implementation, or domain fields in the generic TypeScript transport.

Also stop on evidence if stale completions cannot be rejected unambiguously,
worker failure leaves requests/worlds pending, asset rollback cannot be made
atomic, active work loses priority, pixels change unexpectedly, or frame gap,
memory, Worker count, copies, overflow, and retained-session metrics regress
materially. Otherwise drive the complete main-side cutover autonomously,
commit coherent stages, and stop after Slice 2 for human review before any
worker-side Slice 3 work.

### Slice 2 evidence

The implementation landed as a reviewable sequence:

- `d60d0267` added the 47-line generic polled Worker transport and direct
  transport proof without changing production ownership;
- `9cc974a4` added deterministic Rust coordinator and asset-swap state with
  fake traces for priority, stale completion, failure, release, and rollback;
- `24138036` cut production over atomically and removed the TypeScript broker,
  wake relay, promises, timing maps, release policy, and asset-worker swap;
- `499eeee1` made the source ownership gate reject any return of those
  TypeScript owners; and
- `c8071a5c` repaired stale Far LOD browser-probe assumptions, retained its
  real coverage assertion, and added hidden/resume coverage to the mobile lane.

`WebSceneHost` now owns one `WebRenderWorkerCoordinator`. Scene runtimes hold
world-qualified handles and set active/standby priority through Rust. The
coordinator owns worker generations/readiness, global broker request IDs,
single-in-flight admission, stale/failure/timeout handling, quiescent world
release, candidate asset generations, activation/rollback, reconstruction,
and public timing/worker metrics. Its monotonic timeout clock is internal, so
browser callers cannot mix `Date.now()` and `performance.now()` domains.

Production TypeScript no longer owns `RenderSectionWorkerCompiler`, compiler
promises, request/world timing association, queue priority, wake callbacks,
release messages, or asset-worker replacement. The retained
`mclone-render-compiler-shared.ts` is a 32-line compatibility export over the
generic transport. Authored TypeScript moved from the 7,417-line Slice 0
baseline and 7,413-line pre-Slice-2 tree to 6,484 lines across 15 modules:

| Family | Before | After | Delta |
|---|---:|---:|---:|
| render workers | 1,349 | 536 | -813 |
| web app | 2,650 | 2,487 | -163 |
| generic Worker transport | 0 | 47 | +47 |
| all authored TypeScript | 7,417 | 6,484 | -933 |

The ownership checker reports six Worker entries, two TypeScript construction
sites, zero server-job and main-side render ownership debts, and the unchanged
eight-site copy ledger. Production still has private Wasm heaps, one ordinary
render Worker, external SAB input/result arenas, zero transferred response
bytes, and the worker-side TypeScript render actor reserved for Slice 3.

The complete Rust workspace format, check, and test gates passed, including
the 293-test app-runtime, 512-test server, and 21-test web-client suites. All
three required Wasm target checks, generated-bindgen TypeScript check, Worker
ownership self-test, thread, app, chunk, movement, desktop/mobile lobby, asset
replacement, and IndexedDB reload lanes passed. Five consecutive fresh remote
WebSocket interaction runs also passed authoritative placement/break,
accepted remesh, and zero pending work.

Representative movement evidence retained one Worker initialization, 453
accepted compiles, shared-result-buffer transport, zero result overflow, zero
transferred response bytes, and zero pending work. The retained 16-sample
round-trip range was 14.9–25.7 ms (19.1 ms average), while the browser movement
lane's maximum frame gap was 10.3 ms. The desktop lobby used one Worker across
three qualified world identities and two resident compiler sessions; the
standby world retained 81 chunks, was switchable, and the A→B→A path settled.
Asset replacement advanced from epoch 0/generation 1 to epoch 1/generation 2
with two Worker initializations, two asset-pack sends, active state, no result
overflow, and no pending compile jobs.

App, chunk, asset-replacement, IndexedDB, desktop/mobile lobby, movement, and
Far LOD on/off captures under `/tmp` were inspected. Terrain, HUD, touch UI,
embedded preview, and asset replacement remained visually correct. The Far LOD
enabled capture visibly added the coarse synthetic shell without corrupting
the nearby authoritative terrain.

Three broader harness failures were reproduced on the exact pre-cutover
`d8004e77` control and therefore remain explicit baseline debt rather than
weakened Slice 2 acceptance:

- the lobby lifecycle fixture requires returned actor IDs `1`/`2` after live
  simulation, while both trees return later valid actors;
- the browser Far LOD flight settles at exactly 936 desired, 899 visible, and
  1,225 resident tiles with no pending work on both trees, but advances
  `suppressed_without_replacement` by ten, correctly failing C5; and
- the mobile smoke's startup ledger records hold Y `112` and minimum
  pre-admission Y `93.62` on both trees. Before that unchanged assertion fires,
  the new hidden/resume leg advances background saves from zero to one and
  records one hidden-frame skip.

No gameplay, Far LOD, actor, or startup policy was changed to make the
coordinator appear green. Slice 2 is complete and remains stopped here for
human review.

## Slice 3 — Worker-Side Render Rust Actor

Status: complete 2026-07-19.

Once main-side ownership is Rust, move worker-side domain dispatch into a
resident Rust actor. The actor should own:

- selected asset/catalog template state;
- per-world compiler sessions;
- release-world handling;
- render-section versus Far LOD dispatch;
- target normalization and typed validation;
- worker compile counters and domain diagnostics; and
- typed encoded completion/failure.

The residual TypeScript Worker should load Wasm, forward opaque actor frames
and explicit SAB handles, publish generic status, and report browser failures.
Do not force it into the exact server-job broker abstraction if the asset and
multi-world lifetime shape is genuinely different. Extract common mechanics
only after the two brokers demonstrate the same contract.

Acceptance is the Slice 2 matrix plus a source gate that removes render/Far LOD,
world priority, per-world session, and asset-selection vocabulary from the
worker TypeScript.

After the actor lands, compare the server-job and render Worker shells. Extract
only common Wasm loading, opaque forwarding, SAB publication, browser failure,
yield, and close mechanics that are actually identical. Do not introduce a
general browser-thread runtime or force unlike asset/session protocols through
one abstraction merely to reduce line count.

### Slice 3 execution and evidence

`WebRenderWorkerActor` now owns the selected asset template, per-world session
map, release notifications, render-section/Far-LOD selection, target
normalization, worker counters, typed reports, shared-input validation, result
overflow/failure status, and atomic notification. The external SAB constants
and atomic helpers are shared by main and worker Rust through
`web_render_compiler_abi.rs` and remain locked to the one JavaScript ABI copy.

The production TypeScript Worker fell from 481 to 69 lines. It imports and
initializes the independent Wasm instance, constructs the actor from the first
opaque frame, forwards later frames, posts actor reports, and retains a
last-resort browser/FFI failure envelope. It does not inspect work kind, asset
selection, world priority, compiler methods, or world-session lifecycle. All
three registered worker-side render debts are now locked at zero.

The actor also removed the temporary JavaScript output array from the live
path. Compiler internals return packed `Vec<u8>` values, report summaries read
those bytes directly, and worker Rust copies once into the result SAB. The
explicit production copy ledger therefore fell from eight sites to seven;
input and main-side result copies, independent Wasm heaps, and the external SAB
transport are otherwise unchanged.

Focused Rust/Wasm checks, the ABI and scenario ownership locks, generated
bindgen TypeScript checking, and the Worker ownership self-test passed. The
complete Rust workspace check and test suite passed. The desktop lobby scenario
also passed with one render Worker, one Worker-Wasm initialization, one asset
load, three qualified worlds, two resident compiler sessions, zero result
overflows, and no transferred response bytes. Its embedded preview capture was
inspected and remained coherent. The ordinary browser smoke settled 22 compiles
with one Worker-Wasm initialization, one asset load, zero transferred response
bytes, and zero result overflows. A
fresh movement run settled 450 compiles and 225 rendered movement frames; its
retained 16-sample worker round-trip range was 15.0-32.3 ms (20.1 ms average),
with a 20.3 ms maximum frame gap and no overflow or transferred response.

The dedicated Far LOD probe reached its Rust-authored `far-lod` worker proof,
rendered and captured the enabled multi-level shell, and then failed at the
already-recorded movement C5 coverage debt rather than at actor dispatch or
mailbox publication. The ordinary and Far LOD browser captures under `/tmp`
were inspected and remained visually coherent. The baseline coverage assertion
was not weakened.

The broader lobby lifecycle probe again reached the already-recorded fixture
failure: it waited specifically for actor IDs `1` and `2` after the returned
world contained later valid cow and chicken actors. Before that assertion, the
new actor completed 628 compiles with two resident world sessions, no overflow,
and no transferred response bytes. This is the same pre-cutover debt and the
assertion remains unchanged.

### Post-Slice-3 common-mechanics decision

The required render/server-job comparison found no useful shared broker yet.
Their identical code is limited to a one-shot dynamic Wasm import and error
stringification. The 69-line render shell forwards structured-clone frames to
Rust, whose actor validates inputs and publishes the result SAB. The 157-line
server-job shell still has a materially different dual transport: it selects
message transfer versus SAB, validates request buffers, publishes response
buffers, rings atomics, and contains failure wakeups around an opaque Rust job
actor.

Extracting the small import helper would add a generic type and another module
without removing domain knowledge or copy sites. Sharing SAB publication now
would move proven Rust render ownership back into JavaScript or obscure the
job-specific overflow contract. The approved “common mechanics only after
proof” step is therefore complete with a negative result: keep both shells
small and specialized, and revisit only after another Rust actor removes the
server-job transport policy.

## Slice 4 — Integrated-Server Responsibility Split

Status: complete 2026-07-19.

`mclone-integrated-server-worker.ts` is the largest and most entangled worker.
It combines legitimate browser mechanisms—timers, IndexedDB requests,
job-worker servicing, SAB views—with descriptor projection and lifecycle
coordination. Do not port it wholesale or create one giant Rust browser actor.

After Slices 1-3, inventory it against the proven broker contracts and preserve
this split:

1. Rust authority/session actor policy;
2. generic runner mailbox mechanics;
3. typed IndexedDB platform operations; and
4. browser cadence/yield/shutdown mechanics.

Implement it in this order:

1. Replace generation, topology, realm, behavior, and authority defaults with
   one strict versioned opaque startup frame authored and decoded by Rust.
2. Move authority/session admission, state transitions, job policy, typed
   failures, and graceful domain shutdown into a worker-resident Rust actor.
3. Keep `setTimeout`, Worker messages, SAB typed views, IndexedDB requests and
   transactions, browser yielding, and forced termination in a domain-blind
   TypeScript shell.

Each production transfer must delete its previous TypeScript owner atomically.
If the authority cut cannot remain bounded, draft a focused follow-up tactical
and stop that sub-slice rather than creating one giant browser actor.
IndexedDB schema/migration mechanics may remain TypeScript; generation,
topology, realm, profile, and behavior defaults may not.

### Slice 4.1 execution — opaque authority startup

Main-side Rust now encodes one strict `MCSI` version-1 startup frame. Its codec
owns the seed, generation profile, horizontal topology, behavior profile,
scheduled-fluid policy, passive and auxiliary debug policy, light batch size,
observer role, and local player identity. The decoder rejects bad magic,
unknown versions and flags, unsupported or invalid topology, truncation,
oversized or invalid player names, and trailing bytes. Pure Rust tests cover a
complete round trip and each failure family.

Worker Rust decodes that frame into `WebIntegratedServerStartup`. It constructs
the transient or external-load IndexedDB `LocalRealmSession`, applies stored
metadata precedence, initializes new metadata, configures player versus
observer authority, and applies all domain settings before returning the live
server object. TypeScript only loads Wasm, performs the selected browser
IndexedDB operations, passes their records to Rust, selects the browser runner
transport, and arms the Rust-authored tick interval. The old bulk-record
constructor fallback was unreachable with the current Wasm API and has been
deleted rather than retained as a second production path.

`mclone-integrated-server-worker.ts` fell from 1,202 to 1,039 lines. Authored
TypeScript is now 5,909 lines, and all registered integrated-server startup
projection debts are locked at zero. The external SAB contracts, private Wasm
heaps, IndexedDB schema, cadence, and worker count are unchanged.

The Wasm build and generated TypeScript gate passed. The ordinary browser smoke
passed both shared-memory and message-transfer runner stress, including active
worldgen and light job mailboxes. IndexedDB placement/reload passed with its
persisted mutation visible after reload. The desktop lobby passed protected
break/place denial, warm standby, embedded preview, actor motion, and A-to-B-to-A
activation. The IndexedDB capture was inspected and remained visually coherent.

The next bounded sub-slice is the resident authority/session actor. It may take
ownership of admission, command/tick/poll transitions, job-pending policy,
typed failures, and graceful domain shutdown. Timer, yield, IndexedDB
transactions, SAB view mechanics, and forced Worker termination remain browser
mechanics unless that cut proves a smaller neutral boundary.

### Slice 4.2 execution — resident authority/session actor

`WebIntegratedServerActor` now owns the live Rust session returned by startup
and admits exactly one domain operation at a time. It decodes the browser
message kind, selects command, flush, observer promotion, player demotion, or
graceful shutdown, authors ready/completion/failure envelopes, and decides
whether an empty tick response should be posted. It also owns the 60,000-poll
command-job limit and the exact pending-job predicate. Tick, flush, role, and
shutdown operations preserve their prior non-draining behavior; only commands
perform the pending-job drain, matching the pre-cutover cadence.

The TypeScript shell now has only two browser-owned message cases: initial
startup and runner-SAB slot release. All other messages and timer ticks enter
the resident actor. The shell serializes asynchronous browser callbacks,
executes and batches IndexedDB requests, yields with `setTimeout(0)`, extracts
or publishes external SAB bytes, posts the Rust-authored report, and performs
the final forced Worker close. Its operation-in-flight flag is an event-loop
exclusion guard; Rust independently rejects a second admitted domain operation.

This reduced `mclone-integrated-server-worker.ts` from 1,039 to 919 lines and
the authored TypeScript inventory from 5,909 to 5,789 lines. Source locks now
forbid TypeScript pending-job predicates, direct command/role dispatch, and
command completion policy. No copy site, Wasm heap, IndexedDB schema, timer
cadence, worker count, or SAB ABI changed.

The Wasm build, generated TypeScript gate, focused ownership tests, and Worker
inventory self-test passed. Ordinary smoke again passed shared-memory and
message-transfer runner stress and graceful shutdown. IndexedDB mutation/reload
passed through batched external loads. The desktop lobby passed protected
interaction, observer/player role transitions, warm standby, actor motion, and
A-to-B-to-A activation. No browser fixture assertion was weakened.

## Slice 5 — Rust-Authored Catalog Descriptors

Status: complete 2026-07-19.

Keep IndexedDB CRUD, request/transaction mechanics, browser settings storage,
and DOM-facing file acquisition in TypeScript. Remove its duplicated
generation-profile, topology, behavior, and persisted-world unions by having
Rust own one versioned opaque descriptor and one Rust-authored UI projection.
The catalog adapter may index stable opaque IDs and browser timestamps; it must
not reconstruct domain defaults or validate domain variants independently.

Acceptance requires create/list/open/reopen/delete coverage for plane and
periodic worlds, existing generator-profile fixtures, managed-world isolation,
and unchanged stored-record compatibility. Do not bundle an IndexedDB schema
migration unless the opaque descriptor cannot fit the current record envelope.

### Slice 5 execution and evidence

Rust now owns a strict `MCWC` version-1 descriptor for every ordinary browser
catalog row. The current IndexedDB version and `worlds` key path are unchanged;
new records have only the stable clear `id` plus opaque descriptor bytes. The
descriptor encodes the complete current `LocalWorldSummary`, recomputes
compatibility on decode, rejects invalid magic/version/flags, malformed UTF-8,
truncation, and trailing bytes, and preserves full-width integers internally.

Create, open, and play-recording policy now return a Rust-authored mutation
plan containing the opaque storage record and a separate UI projection. List
returns only Rust-authored projections. The TypeScript adapter stores the
record without reconstructing it and exposes only `id` in its summary type;
the duplicated generation-profile union and all clear-summary production
writes are source-locked at zero. Legacy clear-field records are accepted and
rewritten to the descriptor envelope on open or play-recording, so no startup
migration or database-version increment was needed.

Topology and behavior do not belong to `LocalWorldSummary` today. Their
existing Rust owners remain the dimension record, realm metadata, and opaque
integrated-server startup frame. The periodic acceptance therefore combines
ordinary catalog CRUD with the existing cylinder IndexedDB reopen rather than
inventing a second topology field in the catalog. The managed scenario store
also remains independent from the ordinary catalog.

The web-client suite passed 28 unit tests, including exact descriptor
round-trip and strict rejection cases. The Wasm check, generated-bindgen
TypeScript check, Worker ownership self-test, ordinary browser smoke, and
catalog UI create/list/open/delete probe passed. The ordinary smoke explicitly
inserted a legacy clear record, listed it, opened it, verified its in-place
opaque rewrite, and deleted it. The Flat Grass cylinder IndexedDB probe saved
and reopened 121 chunks, two entity chunks, its dimension descriptor, and the
same canonical/lifted dirt edit. The managed-storage probe repaired partial and
incompatible content, refused corruption, reopened both roles, and retained
zero ordinary catalog rows. Catalog and cylinder captures under `/tmp` were
inspected and remained visually coherent.

One catalog UI run transiently observed chunk records after deletion before a
clean retry passed. The broader catalog-lobby lane reached successful catalog
selection, warm startup, actor observation/motion, A-to-B-to-A activation, and
recency updates, then failed an unchanged remote-player walk-distance direction
assertion; two earlier exact-seed experiments selected a different destination
and were discarded. No assertion or gameplay rule was weakened for this cut.

Authored TypeScript is now 5,778 lines, eleven below Slice 4.2 and 1,639 below
the original 7,417-line inventory. `mclone-web-world-catalog.ts` is 776 lines
versus its 784-line baseline, `mclone-web-app.ts` is 2,484 lines versus 2,650,
all registered catalog ownership debts are zero, and the seven explicit
production copy sites are unchanged.

## Slice 6 — Remeasure And Stop Or Escalate

Status: required closeout.

After the isolated-actor conversions, remeasure the actual residual copies and
duplicate memory. The expected default is to stop with isolated heaps.

Record per lane:

- serialized and copied bytes;
- encode/decode/copy versus worker-compute time;
- user-felt chunk-load and frame latency;
- duplicate resident catalog/snapshot memory;
- TypeScript responsibility and line-count change; and
- worker restart/failure containment.

Shared Wasm linear memory is not Slice 7. If the
[`web-worker-runtime-ownership`](../topics/web-worker-runtime-ownership.md)
revisit gates are satisfied, create a separate tactical with a current
toolchain proof, allocator and memory-growth instrumentation, cooperative
shutdown, forced-crash recovery, and explicit human approval.

## Validation Matrix

Every production cutover keeps these core gates green:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo test --manifest-path native/Cargo.toml
cargo check --manifest-path native/Cargo.toml \
  -p mclone-app-runtime --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:thread-smoke
pnpm native:web:app-smoke
pnpm native:web:chunk-smoke
pnpm native:web:movement-perf
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke
for attempt in 1 2 3 4 5; do
  pnpm native:web:remote-smoke
done
```

Run focused subsets for small preparatory commits; run the complete applicable
matrix before each production owner cutover. Any slice that can change pixels
must capture to `/tmp` and inspect the image before continuing.

Remote interaction validation is intentionally repeated. One green run does
not establish transport stability after the intermittent nonblocking-write
failure found during the baseline cleanup.

Native regression coverage remains required where a shared trait or engine
owner changes. Browser-only broker mechanics do not justify rebuilding every
physical platform, but changes to public scene/session/compiler contracts must
follow the platform validation policy in `docs/platforms.md`.

## Human Review Gates

The 2026-07-19 continuation decision accepts the Slice 2 evidence and
authorizes Slices 3-6 autonomously at the high-value ownership boundary. Commit
each atomic proof/cutover and preserve exact pre-cutover controls for any red
lane. Stop early for a new decision if a slice requires:

- a new SAB ABI or copy strategy;
- a persistence schema or authority cadence change;
- more production Workers, shared Wasm memory, or a general thread runtime;
- a long-lived old/new implementation;
- DOM, rAF, input, WebSocket, or IndexedDB mechanics moving into Rust;
- weaker gameplay, pixel, lifecycle, performance, or failure assertions; or
- a material frame-time, memory, copy, Worker-count, or visual regression.

Otherwise stop after Slice 6 with a residual ownership/copy/performance
decision package. Require a new explicit human decision before browser-native
long-tail reduction, integrated storage migration, or any shared Wasm
linear-memory work.

## Non-Goals

- No shared Wasm linear memory or Rust pointers crossing Workers.
- No `std::thread::spawn` emulation or general browser thread runtime.
- No common allocator across Workers.
- No desktop serialization or browser ABI on native channels.
- No worldgen, lighting, render, scenario, topology, or persistence behavior
  change.
- No WebGPU work from CPU Workers.
- No removal of browser-required IndexedDB, Worker, WebSocket, promise, DOM,
  input, or presentation code merely to reduce line count.
- No long-lived alternate runtime or hidden fallback that can drift.
- No revival of the retired TypeScript engine.

## Relationship To Existing Work

- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) created
  the current native-thread/Web-Worker topology and explicit SAB mailboxes.
- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  established the shared compiler contract, resident render state, and
  delta-only input while leaving a TypeScript doorbell/lifecycle broker.
- [`068-web-zero-copy-worker-lane-investigation.md`](068-web-zero-copy-worker-lane-investigation.md)
  measured and declined shared Wasm linear memory.
- [`069-web-worldgen-lane-payload-reduction.md`](069-web-worldgen-lane-payload-reduction.md)
  proved resident isolated Rust state can remove the dominant payload without a
  shared heap.
- [`070-web-glue-typing-and-abi-hardening.md`](070-web-glue-typing-and-abi-hardening.md)
  and [`071-native-web-typescript-glue-graduation.md`](071-native-web-typescript-glue-graduation.md)
  hardened and typed the existing glue; this tactical changes ownership rather
  than undoing that safety work.
- [`170-web-scene-host-adoption.md`](170-web-scene-host-adoption.md) and
  [`../topics/web-scene-host-adoption.md`](../topics/web-scene-host-adoption.md)
  established the nonblocking shared host and typed platform-operation model
  this work must preserve.
