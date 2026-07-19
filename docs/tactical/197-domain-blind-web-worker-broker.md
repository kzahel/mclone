# Tactical 197: Domain-Blind Web Worker Broker

Status: planned 2026-07-19. Slices 0 and 1 are implementation-ready. Slice 2
has a concrete ownership target but begins only after the small server-job
actor proves the broker contract. Later integrated-server consolidation remains
provisional and must not widen this tactical into a shared-Wasm-memory runtime.

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

Status: ready.

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

## Slice 1 — Server-Job Rust Actor Proof

Status: ready after Slice 0.

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

### Slice 1 review

No product-design review should be necessary if byte, timing, failure, and
pixel evidence remain equivalent. Review the Rust/TypeScript boundary itself
before using it as the render template.

## Slice 2 — Rust-Owned Main-Side Render Broker

Status: concrete target; start after Slice 1 boundary review.

Move the main-side `RenderSectionWorkerCompiler` lifecycle and queue from
`mclone-render-compiler-shared.ts` into browser-specific Rust while leaving the
worker-side compiler session and current SAB wire shape unchanged.

Extend or wrap the existing Rust `WebRenderSectionCompiler`, which already
owns the resident input/result arenas, mirror tracking, request IDs, revisions,
and completion polling, so it also owns:

- browser `Worker` creation and error callbacks;
- worker readiness and asset initialization;
- the active/standby admission queue;
- request-to-world association and world release ordering;
- timeouts, failures, and restart state;
- compiler wake posting; and
- compile timing/diagnostic merge.

This removes the current `js_sys::Function` compiler wake callback and
domain-rich doorbell relay from `WebSceneRuntimeService`. Rust may still build
a JavaScript `Object` to call `Worker.postMessage`; the important change is
that authored TypeScript no longer interprets or reconstructs the domain
request.

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

### Slice 2 acceptance

- One Rust owner performs render worker admission and request association.
- No JavaScript callback crosses into `WebRenderCompilerWakeSink`.
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

## Slice 3 — Worker-Side Render Rust Actor

Status: provisional after Slice 2.

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

## Slice 4 — Integrated-Server Responsibility Split

Status: decision checkpoint, not yet implementation-ready.

`mclone-integrated-server-worker.ts` is the largest and most entangled worker.
It combines legitimate browser mechanisms—timers, IndexedDB requests,
job-worker servicing, SAB views—with descriptor projection and lifecycle
coordination. Do not port it wholesale or create one giant Rust browser actor.

After Slices 1-3, inventory it against the proven broker contracts and split it
into:

1. Rust authority/session actor policy;
2. generic runner mailbox mechanics;
3. typed IndexedDB platform operations; and
4. browser cadence/yield/shutdown mechanics.

Draft a focused follow-up tactical if this cannot be expressed as one bounded
cut. IndexedDB schema/migration mechanics may remain TypeScript; generation,
topology, realm, profile, and behavior defaults may not.

## Slice 5 — Remeasure And Stop Or Escalate

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

Shared Wasm linear memory is not Slice 6. If the
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
pnpm native:web:remote-smoke
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Run focused subsets for small preparatory commits; run the complete applicable
matrix before each production owner cutover. Any slice that can change pixels
must capture to `/tmp` and inspect the image before continuing.

Native regression coverage remains required where a shared trait or engine
owner changes. Browser-only broker mechanics do not justify rebuilding every
physical platform, but changes to public scene/session/compiler contracts must
follow the platform validation policy in `docs/platforms.md`.

## Human Review Gates

The workstream can be driven autonomously within these stops:

1. Review Slice 0's allowed browser-mechanism versus domain-debt
   classification.
2. Review Slice 1's actor/broker contract before applying it to render.
3. Inspect render and lobby captures after Slice 2; performance/pixel gates are
   load-bearing even when the refactor is intended to be invisible.
4. Review the integrated-server decomposition before Slice 4.
5. Require a new explicit decision before any shared Wasm linear-memory work.

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
