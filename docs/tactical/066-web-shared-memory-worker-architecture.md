# 066: Web Shared-Memory Worker Architecture

Status: Superseded by [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md).
The render-worker `SharedArrayBuffer` ABI and JS-shrink boundary are carried
forward into 067; this doc is retained for its landed history. Historical
status: high-priority architecture; render-section compiler transport
diagnostics, persistent worker asset/catalog state, and shared result arena
prototype landed; request-scoped shared input snapshots landed. Final resident
shared arenas/rings and the Rust-owned worker lifecycle are next.

## Purpose

Define the browser worker architecture we actually want for native web/WASM:
Rust-owned engine scheduling, explicit worker interfaces, and bulk data moved
through `SharedArrayBuffer`/`Atomics` instead of ad hoc `postMessage` payloads.

This document exists because the current web render compiler has the right
high-level worker shape but the wrong hot-path transport and ownership shape.
It runs off the browser animation-frame path, but it still behaves like a
full-request transaction across JavaScript, wasm-bindgen, a Web Worker, and
packed transferred buffers.

The architectural rule going forward:

- new hot browser engine worker paths must define their shared-memory interface
  up front
- transferred `Uint8Array`/object-message protocols are allowed only as
  explicit smoke/debug/fallback paths with diagnostics and removal criteria
- JavaScript may host workers and tiny control messages, but Rust owns engine
  scheduling, job identity, stale/revision checks, and compile scope decisions

## Current State

### Desktop Render Compile

Desktop render compilation is already close to the shared engine shape:

- `mclone-render-session::RenderSectionCompiler` exposes
  `submit`, `try_recv_completed`, and `pending_job_count`.
- `mclone-native-client::render_cache::RenderSectionCompileWorker` runs on a
  native OS thread behind that trait.
- The worker receives a `RenderSectionCompileRequest` with target section keys,
  submitted revisions, and actual client `ChunkSnapshot` data.
- The worker owns/clones the mesh catalog once at startup and builds
  `build_render_sections_from_snapshots(...)` over the submitted snapshots.
- The app drains completed native channel results, applies shared stale/revision
  acceptance, uploads bounded GPU work, and renders the current cache.

This path is not zero-copy internally, but it has the important boundaries:
Rust owns request construction, the worker does not regenerate a fake view, and
the frame loop is a submit/drain/render loop instead of a promise transaction.

### Browser Runner And Server Jobs

Browser integrated-server runner, worldgen, and light-status lanes already prove
that shared memory is viable in the web app:

- `mclone-server::wasm_job_worker` owns pooled `SharedArrayBuffer` request and
  response frames for worldgen/light jobs.
- `mclone-web-client::web_server_worker` owns pooled shared runner command and
  update frames.
- `mclone-server-job-worker.js` and `mclone-integrated-server-worker.js` accept
  shared frame messages, use `Atomics` for status/byte counters, and keep
  transferred-message fallbacks visible through metrics.
- Browser smokes assert cross-origin isolation, shared Wasm memory, `Atomics`,
  module workers, and shared-memory metrics on the normal runner/job paths.

That means the remaining render compiler issue is not "web cannot do shared
memory." It is a render-worker-specific architecture gap.

### Browser Render Compiler

The current browser render compiler still has a proof-path transport:

- `mclone-web-app.js::RenderSectionWorkerCompiler` owns a browser worker and a
  JS `pending` promise map.
- The worker has an `init-render-compiler` handshake and a resident
  `WebRenderCompilerSession`; the packed vanilla asset zip is transferred and
  parsed once at worker init.
- Normal compile requests now carry center/radius and `targetSections`, not
  the asset pack.
- Normal compile requests now include encoded main-session `ChunkSnapshot`
  input bytes. The page copies those bytes into a request-scoped
  `SharedArrayBuffer` input arena, publishes ready/byte/capacity control words,
  and the worker compiles through
  `WebRenderCompilerSession::compileSnapshotSectionsForTargets(...)`.
- The generated `WebRuntime::local_integrated(SMOKE_SEED)` compiler path still
  exists as a fallback/smoke helper, but browser smokes now fail if the normal
  app hot path uses that generated-view fallback.
- The worker writes the packed `TexturedRenderSectionBuildReport` into a
  `SharedArrayBuffer` result arena on the normal path; transferred
  `Uint8Array` results remain only as explicit fallback.
- The main wasm instance still copies the shared typed-array view with
  `to_vec()`, decodes the packed report, applies shared acceptance/cache logic,
  then uploads GPU data.

Target-section-aware requests now bound the result section keys, and shared
input snapshots stop the worker from regenerating a fake view for normal
compiles. The result payload no longer travels as a multi-megabyte
`postMessage` transfer. The remaining gap is that both input and output are
still request-scoped packed byte transactions with wasm copies and a JS promise
owner, not the final Rust-owned shared worker ring/lifecycle.

## Why The Proof Path Exists

The render worker was useful because it moved mesh CPU work off
`requestAnimationFrame` and gave the native web app a real worker-backed
WebGPU render smoke. It was also much easier to package:

- wasm-bindgen exports can accept and return `Uint8Array` cleanly
- a transferred packed report works without defining a shared ABI first
- correctness, stale-result handling, and browser screenshots could land before
  the server runner/job worker shared-memory lanes existed

That tradeoff was reasonable for bring-up. It is not reasonable as the hot
streaming implementation now that the rest of the browser topology has
shared-buffer transports.

## Gaps From The Ideal

The render compiler gaps are concrete:

1. Worker request ownership is still partly JavaScript-owned.
   Rust decides target sections, but JS still owns promise orchestration and
   browser worker request lifetimes.
2. Output is still one packed report.
   The normal path now stores it in a shared result buffer, but main wasm still
   copies and decodes the whole report before cache/GPU work can be applied.
3. Worker input is now the main session's actual snapshot set, but it is still
   packed per request and copied into a request-scoped shared arena. The target
   architecture should keep resident shared input/state where practical and
   make section/neighbor readiness explicit.
4. Render compile uses only a partial shared-memory ABI.
   Result bytes and snapshot input bytes use shared arenas, but target-section
   data still crosses as JS message data, the JS promise map still owns request
   lifetimes, and the main wasm instance still copies/decodes packed reports.
5. Dirty planning is still too coarse for some movement cases.
   A move can dirty `8-9` chunks and expand loaded dirty chunks to `128-144`
   vertical section keys. The shared-memory worker ABI will reduce transport
   cost, but it does not replace section-level dirty planning.

## Two Shared-Memory Shapes

There are two browser shapes that both use `SharedArrayBuffer`.

### 1. Explicit Shared-Buffer Job ABI

This is the shape already used by runner/worldgen/light paths.

```text
main wasm/session
  -> writes request bytes or section inputs into shared slots
  -> sets atomic control header
  -> posts tiny worker wakeup/control message

worker wasm instance
  -> reads shared input slot
  -> writes response bytes/mesh output into shared slot or arena
  -> sets atomic status/byte counters
  -> posts tiny completion message or notifies

main wasm/session
  -> drains completed slots
  -> applies shared stale/revision acceptance
  -> uploads bounded GPU work
```

Pros:

- works with separate browser worker wasm instances
- keeps the interface narrow and testable
- avoids transferring multi-megabyte buffers through `postMessage`
- matches the existing server-job shared frame implementation
- lets us start with one-slot or small-ring implementations and grow from there

Costs:

- the ABI must be designed: headers, offsets, counts, status values, overflow,
  ownership, and versioning
- Rust object graphs are not automatically shared; section inputs and mesh
  outputs still need a compact shared representation
- some copy into and out of shared arenas remains unless later replaced by a
  true shared heap/thread runtime

This is the near-term target for render compilation.

### 2. Shared Wasm Memory Thread Runtime

This is the pthread/Rayon-style shape: workers share one
`WebAssembly.Memory` whose backing buffer is a `SharedArrayBuffer`, and Rust
jobs operate closer to normal threads.

Pros:

- can reuse more native thread-pool mental model
- can avoid manually serializing some data if the runtime and ownership model
  permit shared heap access
- may eventually reduce separate ABI code for some worker classes

Costs:

- still requires explicit thread-safe ownership and synchronization
- does not remove the need to define which data the render worker may read,
  mutate, or publish
- packaging, wasm-bindgen integration, stack/heap sizing, panic/error handling,
  and browser feature constraints are more demanding
- WebGPU upload/presentation still remains on the main render adapter
- complex engine state cannot become "willy-nilly shared" without recreating
  desktop-style data races in the browser

This may be useful later, but it should not block the render compiler SAB ABI.
Even if we adopt a shared Wasm thread runtime later, the explicit worker
interface remains the design contract.

## Target Render Compiler Shape

The Rust-facing surface stays the same:

```text
RenderSectionCompiler
  submit(RenderSectionCompileRequest)
  try_recv_completed() -> Vec<RenderSectionCompileResult>
  pending_job_count()
```

Platform backends differ underneath:

```text
desktop backend
  native thread/channel
  worker owns mesh catalog
  request carries target sections, revisions, and snapshots

web backend
  persistent worker or worker pool
  shared control ring plus input/output arenas
  worker owns wasm module and resident mesh catalog
  request carries target sections, revisions, and explicit snapshot inputs
```

JavaScript should shrink to:

- create/terminate browser `Worker` instances
- pass bindgen URLs and initial shared buffers during worker initialization
- post tiny wakeup/control messages when Rust asks it to
- surface worker errors and diagnostics

JavaScript should not own:

- compile coalescing
- target section choice
- stale/revision acceptance
- asset pack copying per compile
- large result payload ownership
- dirty-section policy

## Render Shared ABI

The first render ABI can be deliberately small. It does not need to solve a
general ECS or arbitrary Rust object sharing problem.

### Initialization

Worker init message:

- bindgen JS/WASM URLs
- packed asset bytes once, or a shared asset buffer once
- shared control buffer
- shared input arena
- shared output arena
- ABI version and capacity diagnostics

Worker init result:

- ok/error
- transport kind: `shared-memory`
- mesh catalog load count
- asset byte count
- arena capacities

The worker must not parse the asset pack per compile request.

### Job Header

Each job entry should include:

- request id
- compile generation id
- target section count
- input byte offset/length
- output byte offset/capacity
- flags
- status: empty, pending, running, complete, stale, failed, overflow
- submitted section revision hash or per-section revision table offset

Atomics own status and byte/count publication. The exact header layout should
live in Rust constants and be mirrored in JS only where browser worker glue must
touch it.

### Input Payload

Input should describe the actual main-session data needed to mesh the requested
sections:

- target `RenderSectionKey` list
- submitted revisions for every target section
- section block palettes or compact block-state ids for target sections
- neighbor section facts needed for face culling, ambient occlusion, and light
- light payloads if the target section reads them
- mesh catalog version id

The worker should not request a new center/radius view from a fresh
`WebRuntime`. If the main session did not provide a section or neighbor, the
ABI should say whether that means "not ready," "air boundary," or "defer."

### Output Payload

Output should be section-addressed, not a whole-view transaction:

- section key
- submitted revision or revision hash
- status per section: complete, empty, stale, failed, overflow
- visibility set bits
- vertex/index counts
- vertex/index output offsets
- build stats, including visibility-graph timing
- error code or compact error string offset for failed jobs

The main session applies the existing shared stale/revision acceptance before
mutating the CPU render cache. GPU upload remains main-thread render-adapter
work.

### Overflow And Backpressure

Variable-size mesh output is the main hard part. The ABI needs explicit
behavior instead of silently falling back to a giant transfer:

- split jobs by section count when the output arena is tight
- mark a job `overflow` with required byte count so the main side can grow or
  retry a slot
- keep a hard diagnostic if a fallback transferred result is used
- expose pool hits/misses/drops, overflow retries, max live capacity, and
  pending high-water counts

## Implementation Sequence

### 1. Render Worker Transport Diagnostics

Status: completed first pass.

Add render-compiler frame metrics equivalent to runner/job metrics:

- transport kind
- shared-memory availability
- transferred request/response byte counts
- shared request/response byte counts
- asset load count
- worker wasm init count
- worker compile count
- overflow/fallback counts

Acceptance:

- movement perf reports render compiler transport kind
- normal shared runner/job paths remain unchanged
- current transferred render compiler is visibly labeled as temporary

Landed on 2026-06-22:

- `mclone-render-compiler-worker.js` now labels render compile responses as
  `message-transfer`, reports shared-memory availability, worker wasm init
  count, worker compile count, request asset-pack bytes, target-section bytes,
  and transferred response bytes.
- `mclone-web-app.js` and `mclone-web-smoke.js` now track app-side render
  compiler metrics: worker construction count, compile count, asset-pack send
  count, per-request transfer byte lengths, and cumulative transferred
  request/response byte totals.
- Public web compile timings now include both quick top-level fields such as
  `renderCompilerTransportKind`, `renderCompilerRequestAssetPackByteLength`,
  `renderCompilerTransferredRequestByteLength`, and
  `renderCompilerTransferredResponseByteLength`, plus a nested
  `renderCompilerMetrics` object.
- Browser smoke assertions now require accepted compile timings and direct
  render-worker results to expose the temporary `message-transfer` path and
  nonzero asset/result transfer bytes.

Observed local movement perf after this slice:

- movement compile transport kind was `message-transfer`
- each render compile still transferred the packed asset zip:
  `5,828,345` request bytes per compile
- the final full movement compile transferred an `11,623,508` byte packed
  response
- cumulative render compiler request transfer reached `23,313,380` bytes after
  four worker compiles in the movement perf run
- runner, worldgen, and light-status lanes remained `shared-memory`, confirming
  the divergence is render-compiler-specific

### 2. Persistent Render Worker Assets

Status: completed first pass.

Before changing result transport, remove the worst lifecycle mistake:

- initialize the worker once
- parse/load the asset pack once
- keep the mesh catalog resident in the worker
- stop sending `assetPack.slice()` per compile

Acceptance:

- tiny targeted compiles no longer pay asset-pack copy/parse time
- diagnostics prove asset load count stays at `1` per worker
- transferred result fallback still works while the SAB ABI is built

Landed on 2026-06-22:

- Added `WebRenderCompilerSession` in `mclone-web-client` as a worker-callable
  wasm-bindgen class that owns loaded terrain mesh assets and compiles generated
  render sections through the resident catalog.
- `mclone-render-compiler-worker.js` now has an explicit
  `init-render-compiler` handshake. The asset pack is transferred once during
  worker init, parsed once, and retained in the worker session.
- `mclone-web-app.js` and `mclone-web-smoke.js` now wait for worker readiness
  before compile submission and no longer send `assetPack.slice()` in normal
  compile requests.
- Browser smoke assertions now require `persistentAssetCatalog: true`,
  `assetPackSendCount: 1`, nonzero `workerAssetPackInitByteLength`, nonzero
  `workerAssetLoadCount`, and zero per-compile
  `requestAssetPackByteLength`.

Observed local movement perf after this slice:

- movement compile transport kind is still `message-transfer`
- worker asset load count stayed at `1`
- asset pack send count stayed at `1` across four worker compiles
- per-compile asset request bytes dropped from `5,828,345` to `0`
- one-time worker asset init transfer was `5,828,345` bytes
- the final full movement compile still transferred an `11,623,508` byte
  packed response
- worker round trips still stayed high, `858.1-883.9ms` for movement compiles,
  which points to fresh worker-side runtime/view generation and packed response
  transfer rather than per-compile asset parsing

### 3. Shared Result Arena Prototype

Status: completed first pass.

Move worker output from transferred packed reports to a shared result slot:

- one inflight compile is enough for the first slice
- write packed report bytes or section-addressed output bytes into a shared
  response arena
- publish byte counts/status with `Atomics`
- keep transferred result as explicit fallback

Acceptance:

- render compiler reports `shared-result-buffer` on cross-origin-isolated
  browsers
- movement perf fails if it unexpectedly uses `message-transfer` on the normal
  path
- decode/apply behavior remains unchanged initially

Landed on 2026-06-22:

- `RenderSectionWorkerCompiler` in both the app and web smoke page now allocates
  a per-compile shared control header and 16 MiB shared response arena, sends
  those buffers to `mclone-render-compiler-worker.js`, and reads the packed
  result from a shared typed-array view.
- The worker writes packed report bytes into the shared result arena, publishes
  byte count/capacity/status with `Atomics`, and posts only metadata plus the
  shared-buffer reference. If the arena is too small, the worker allocates an
  overflow `SharedArrayBuffer` and reports the overflow instead of transferring
  the packed bytes.
- Browser diagnostics now separate packed byte length, transferred response
  bytes, shared result bytes, shared result capacity, response count, and
  overflow count.
- Browser smoke assertions now require `shared-result-buffer`,
  `sharedResultBufferUsed: true`, and zero transferred render-compiler response
  bytes on the normal path.

Observed local movement perf after this slice:

- initial compile: `144` submitted sections, `11,310,892` packed/shared result
  bytes, `0` transferred response bytes, `887.7ms` worker round trip
- movement compile: `129` submitted sections, `9,740,708` packed/shared result
  bytes, `0` transferred response bytes, `866.0ms` worker round trip
- tiny stale movement compile: `2` submitted sections, `36` packed/shared
  result bytes, `0` transferred response bytes, `830.1ms` worker round trip
- later full movement compile: `144` submitted sections, `11,623,508`
  packed/shared result bytes, `0` transferred response bytes, `890.1ms`
  worker round trip
- no shared result overflow occurred with the 16 MiB arena

This confirms the remaining web/desktop divergence is no longer large
`postMessage` response transfer. At this point the remaining cost was
worker-side compile and generated-view setup, plus coarse dirty planning that
still asked for `128-144` sections in full movement cases. The next slice below
removed generated-view setup from the normal worker path.

### 4. Shared Input Arena With Actual Snapshots

Status: completed first pass.

Replace worker-local view regeneration:

- main session packs target sections and required neighbors into shared input
- worker compiles from those inputs
- remove the fresh `WebRuntime::local_integrated(...)` path from normal worker
  compiles
- keep a generated-view smoke fallback if still useful, but route it outside
  the app hot path

Acceptance:

- worker compiles exactly the main session's submitted target sections
- worker cannot publish data for sections the main session did not request
- stale/revision tests still pass
- movement perf separates worker mesh cost from server/chunk-view generation

Landed on 2026-06-22:

- `WebChunkRenderSession` now collects the main session's current
  `ChunkSnapshot`s for each compile request and submits them through the shared
  `RenderSectionCompileRequest` path instead of passing an empty snapshot list.
- The web compile request exposes encoded snapshot input bytes,
  `snapshotInputByteLength`, and `snapshotInputChunkCount`.
- App and smoke worker clients copy that snapshot input into a request-scoped
  `SharedArrayBuffer` input arena, publish ready/byte/capacity control words
  with `Atomics`, and send only SAB handles plus section metadata to
  `mclone-render-compiler-worker.js`.
- The worker reads the shared input and calls
  `WebRenderCompilerSession::compileSnapshotSectionsForTargets(...)`. Generated
  center/radius compilation remains available only as fallback/smoke behavior.
- Diagnostics now include request snapshot input bytes, shared input bytes,
  shared input capacity, snapshot input chunk count,
  `snapshotInputCompileUsed`, and `generatedViewFallbackUsed`. Browser smokes
  require the normal path to use shared input snapshots and reject generated
  fallback use.

Remaining gaps after this first pass:

- the main wasm instance still serializes `ChunkSnapshot`s into a packed input
  frame and JS copies that frame into the request SAB;
- the worker wasm instance still copies the SAB-backed `Uint8Array` into wasm
  memory before decoding;
- target-section data is still a JS `Int32Array` message field;
- output is still a packed build report in a request-scoped shared result
  buffer, not section-addressed resident mesh output;
- JS still owns the worker promise map instead of a Rust `RenderSectionCompiler`
  backend.

Observed local movement perf after this slice:

- normal accepted compile timings all reported `snapshotInputCompileUsed: true`
  and `generatedViewFallbackUsed: false`;
- radius-1 snapshot input frames were about `206-215 KB`, used the shared input
  arena, and transferred `0` request bytes per compile;
- worker round trips dropped from the earlier `~0.87-0.89s` generated-view path
  to roughly `34-66ms`;
- large movement cases still submitted `96-144` sections and produced
  `7.9-11.6 MB` packed/shared result buffers;
- total movement compile windows were still roughly `262-474ms`, so the next
  work is dirty-scope reduction plus replacing request-scoped packed input and
  packed output with the final resident shared compiler backend.

### 5. Section-Level Dirty Planning

Reduce full dirty plans after the transport is no longer hiding the cost:

- split chunk-level loaded-view dirtying from section-level neighbor dirtying
- when a removed neighbor chunk affects a boundary, rebuild only boundary
  sections that actually need that neighbor fact
- avoid expanding every dirty chunk to all vertical sections unless the whole
  chunk snapshot changed

Acceptance:

- one-chunk movement over already-loaded terrain submits materially fewer than
  `128-144` sections when only boundaries changed
- full-chunk updates still rebuild all affected sections
- desktop and web use the same dirty planning logic

## Boundary Rules

- Hot browser worker paths must not introduce new multi-megabyte
  `postMessage` transfers unless the tactical labels them fallback/debug only.
- Every worker path needs an explicit interface:
  job identity, input ownership, output ownership, synchronization, lifetime,
  cancellation/staleness, memory bounds, fallback behavior, and metrics.
- Shared memory is not permission for unstructured shared mutable engine state.
  The shared contract must say exactly which thread may read/write each region.
- The Rust session/runtime layer owns scheduling policy. JS owns browser
  worker construction, wakeups, and DOM/WebGPU presentation glue.
- WebGPU upload and presentation stay on the main render adapter.
- `wasm32-unknown-unknown` remains the browser target unless a tactical
  explicitly changes it.

## Validation

The render-worker shared-memory slices should keep the existing gates:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/www/mclone-web-smoke.js
node --check native/apps/mclone-web-client/www/mclone-render-compiler-worker.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Rendered browser slices still need screenshot inspection under `/tmp`.

Perf assertions now require the normal render-compiler path to use
`shared-result-buffer`, shared input snapshots, and no generated-view fallback.
The harness should keep recording:

- render compiler transport kind
- asset load count
- request/response bytes by transport
- snapshot input byte count and input chunk count
- shared input/result arena capacities
- whether snapshot input compile or generated-view fallback ran
- worker init count
- per-section compile counts
- overflow/fallback counts

## Relationship To Other Tacticals

- `061-shared-engine-web-adapter-refactor.md` owns the shared Rust
  render-session boundary.
- `062-shared-threading-topology.md` owns the parent native-thread/Web-Worker
  topology and existing runner/job shared-memory lanes.
- `065-native-web-mobile-streaming-performance.md` owns the immediate movement
  performance measurements and dirty-scope reductions.
- This tactical owns the render-worker shared-memory ABI and the policy that
  future web worker hot paths must not default to bulk transferred buffers.
