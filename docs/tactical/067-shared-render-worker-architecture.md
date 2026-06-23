# 067: Shared Render-Worker Architecture

Status: active high-priority architecture parent. Stage 0 (baselines), Stage 1
(resident SAB arenas), Stage 2 (web `RenderSectionCompiler` over the resident
ring), and **all of Stage 3** — the keystone (per-frame
`sync_render_sections_with_budget` streaming: the shared loop, web
`syncCameraRenderFrame`, mega-job eliminated) plus the legacy-deletion tail
(overview smoke converted to a `syncOverviewRenderFrame` pump-to-idle; the
begin/finish/submit/poll/request compile exports + their internals, the
`RenderViewCompileQueue`/`loaded_center`/`compile_requests`/`shared_compile`
fields, and the non-SAB `message-transfer` fallback deleted) — are landed.
**Stage 4** — the render-compile worker now holds a resident snapshot mirror, so
the per-frame SAB input is a delta (changed columns + evictions) against that
mirror instead of the whole loaded world: input dropped from ~206 KB/compile to
24 bytes–122 KB (header-only when the target column is already resident) — is also
landed. Stage 5 (JS dedup + single-source ABI constants) remains. Supersedes
[`065-native-web-mobile-streaming-performance.md`](065-native-web-mobile-streaming-performance.md)
and [`066-web-shared-memory-worker-architecture.md`](066-web-shared-memory-worker-architecture.md),
folding their streaming-perf goals and the render-worker `SharedArrayBuffer`
ABI into one direction. The shared render-session boundary stays owned by
[`061-shared-engine-web-adapter-refactor.md`](061-shared-engine-web-adapter-refactor.md);
the parent native-thread/Web-Worker topology and the server-runner / worldgen /
light / transport lanes stay owned by
[`062-shared-threading-topology.md`](062-shared-threading-topology.md). This
doc owns only the render-compile worker.

## Purpose

Make the desktop native app and the native web/WASM app run the **same render
compile topology** with close performance characteristics, without:

- making desktop code convoluted because web needs `SharedArrayBuffer`,
- growing duplicate JavaScript orchestration, or
- paying web-specific serialization costs on the desktop hot path just for
  architectural symmetry.

The render-session policy is already shared in Rust. The remaining divergence is
in *who drives the compile loop* and *at what granularity*, not just in the
worker transport. This tactical defines the target so the next implementation
slices have one direction instead of three render-worker docs.

## What Is Already Unified (do not re-do)

Both platforms already share the policy core in `mclone-render-session`:

- dirty state, ready planning, chunk budgeting, and removal classification
  (`RenderSectionDirtyState`, `plan_ready_render_sections`,
  `prepare_render_section_sync_plan`),
- request identity, submitted revisions, and stale acceptance
  (`RenderSectionCompileRequest`, `RenderSectionCompileResult`,
  `partition_by_revision`),
- cache merge after a completed compile (`apply_finished_compile_report`,
  `drain_completed_compile_updates`).

The web worker output is already routed back through that same acceptance path
via `compile_result_from_worker_report` in
`native/apps/mclone-web-client/src/web_canvas.rs`. So **request identity,
revisions, stale acceptance, and cache merge are Rust-owned on both sides
today.** JavaScript does not own staleness or coalescing policy. Implementers
should not rebuild any of that; this tactical changes the loop and the
transport around it.

## Current State

### Desktop render compile (the reference shape)

`native/apps/mclone-native-client/src/scene_runtime.rs` and
`render_cache.rs`:

- `RenderSectionCompileWorker` runs on an OS thread and implements
  `mclone_render_session::RenderSectionCompiler`
  (`submit` / `try_recv_completed` / `pending_job_count`)
  (`render_cache.rs:125`).
- The request is a `RenderSectionCompileRequest { target_sections,
  section_revisions, snapshots: Vec<ChunkSnapshot> }` moved over `mpsc` with
  **zero serialization**; the result is a `TexturedRenderSectionBuildReport`
  moved back over `mpsc`.
- The frame loop is continuous **incremental streaming**:
  `sync_render_sections_with_budget(camera, DEFAULT_RENDER_CHUNK_MESH_BUDGET)`
  with the budget set to `1` chunk per frame (`scene_runtime.rs:39, 600`),
  distance-sorted, `RenderSectionRemovalMode::ApplyImmediately`. Each frame
  drains completed jobs, merges the cache, uploads bounded GPU work, and renders
  the current cache. One job is in flight at a time but jobs are tiny.

### Native-web render compile (the divergent shape)

`native/apps/mclone-web-client/src/web_canvas.rs` and `www/*.js`:

- Web does **not** implement `RenderSectionCompiler`. It drives the worker
  round-trip from JavaScript through bespoke wasm-bindgen exports
  (`beginChunkRenderCompileRequest` / `finishChunkRenderCompileRequest` /
  `requestCameraRenderCompile` / `takeQueuedCameraRenderCompile`).
- The loop is a one-shot **whole-view transaction**. `prepare_chunk_render_plan`
  builds the entire ready plan in a single request with `chunk_budget =
  usize::MAX` (`web_canvas.rs:2291`), `ReadyWithNeighbors` for every section,
  and `RenderSectionRemovalMode::Defer`. `mclone-web-app.js::compileCameraView`
  owns the `begin -> worker promise -> finish` sequence and the `pendingCompile`
  busy flag (`mclone-web-app.js:378-429`).
- Coalescing uses `RenderViewCompileQueue<String>` + `loaded_center`, which are
  **web-only** constructs (desktop is purely dirty-driven and references
  neither).
- Transport is the full request-scoped encode/copy/decode chain: every request
  re-encodes *all* loaded snapshots
  (`client().chunk_snapshots().cloned().collect()` ->
  `encode_web_render_compile_input`, `web_canvas.rs:1728-1735`), copies into a
  per-request `SharedArrayBuffer` input arena, the worker copies the SAB view
  into worker wasm memory and decodes, builds a packed report, writes it into a
  per-request SAB result arena, and main wasm `to_vec()`s + decodes it back.

### Why this is the real problem

Because web compiles the **entire ready plan as one job** (`usize::MAX`) instead
of a per-frame increment, every boundary crossing submits 96-144 sections and
produces an up-to-11.6 MB result. This is a scheduling-granularity divergence,
not a transport bug: even with perfect zero-copy SAB, a whole-view job is still
a whole-view job. The transport chain makes it worse; the budget is the root
cause.

Measured after the latest landed slice (066, request-scoped shared input
arena):

- snapshot input ~206-215 KB per radius-1 request,
- packed/shared result still up to ~11.6 MB,
- worker round trips ~34-66 ms (down from ~0.87-0.89 s once the worker stopped
  regenerating a local view),
- total movement compile windows still ~262-474 ms,
- dirty planning still submits 96-144 sections for movement, often only 44-68
  non-empty.

### Two secondary issues

1. **Request-scoped input is the whole world.** The worker holds a resident
   mesh catalog (`WebRenderCompilerSession`) but no resident snapshot mirror, so
   movement re-ships chunks it shipped last frame.
2. **SAB arenas are allocated per request.** `createRenderCompilerSharedInputBuffer`
   / `createRenderCompilerSharedResultBuffer` (`mclone-web-app.js:1538-1578`)
   `new SharedArrayBuffer(...)` on every compile, with realloc-on-overflow.
   Nothing is a resident ring.

### JavaScript duplication

`www/` has **zero ES-module structure** (no `import`/`export` anywhere).
`mclone-web-app.js` (2327 lines) and `mclone-web-smoke.js` (1103 lines)
copy-paste ~35 shared symbols: the entire render-compiler worker protocol
constants, the `RenderSectionWorkerCompiler` class, SAB buffer creation,
shared-memory support detection, metrics helpers, and worker URLs. The ABI
constants are hand-duplicated a third time in
`mclone-render-compiler-worker.js`.

## Target Architecture

**One shared logical `RenderSectionCompiler` contract; platform-specific memory
mechanics behind it. Web adopts desktop's streaming loop.**

1. **Web implements `RenderSectionCompiler`** over a resident SAB ring, exactly
   the seam desktop uses over `mpsc`. `submit` serializes the request into the
   input arena and rings a doorbell; `try_recv_completed` polls the result
   control word via `Atomics.load` **from main wasm** and decodes — no
   JavaScript in the data path, no `await`.
2. **Web drives the same `sync_render_sections_with_budget` loop as desktop**,
   with budget ~1 (tunable; could be a small worker pool). The 96-144-section
   mega-job is replaced by per-frame increments that fill progressively while
   the current cache stays visible.
3. **JavaScript shrinks to a doorbell + lifecycle shim**: construct the Worker,
   hand it bindgen URLs + the resident shared buffers at init, post tiny wakeup
   messages, surface errors/diagnostics. It owns no scheduling, no busy flag, no
   payload, no promise chain.
4. **The worker holds resident snapshot state**, so request-scoped input is the
   *delta* (target keys + revisions + the few changed columns).

**Desktop transport does not change.** Desktop keeps `Vec<ChunkSnapshot>` +
channels by move. The symmetry lives in the *trait*, not in a shared byte ABI.
Forcing desktop through a serialized arena ABI for symmetry would tax the
desktop hot path for zero benefit. We choose a shared logical trait, **not** a
common arena ABI across both platforms.

### Interface boundaries

```text
shared Rust render-session policy  (mclone-render-session)
  dirty state · ready plan · revisions · stale acceptance · cache merge
  sync_render_sections_with_budget(...)  <- lift here; both platforms call it
  trait RenderSectionCompiler { submit; try_recv_completed; pending_job_count }

platform worker backend  (Rust, behind the trait)
  desktop: RenderSectionCompileWorker   -> thread + mpsc, request by move
  web:     WebRenderSectionCompiler      -> resident SAB ring, Atomics doorbell

JS worker glue  (web only — thin)
  Worker construction · init buffers · wakeup postMessage · error/diagnostics

wasm-bindgen exports  (web only — shrinks)
  advance_frame(input,dt) · sync_render() -> small status/diagnostics struct
  (NO packed bytes cross this boundary; result lives in the SAB ring)

GPU upload / presentation  (platform render adapter, unchanged)
  read accepted sections from shared cache · bounded per-frame upload
```

### Resident vs request-scoped state

- **Resident, worker-readable:** mesh catalog/atlas (already resident), the SAB
  control + input + output ring (allocate once at init; today per-request), and
  a worker-side snapshot mirror.
- **Request-scoped:** only the per-frame delta — target `RenderSectionKey`s,
  their submitted revisions, and the columns that changed since the last
  compile.

## Render Shared ABI (carried forward from 066)

The first render ABI can stay small. It does not need to solve general Rust
object sharing. It uses the explicit shared-buffer job shape already proven by
the runner / worldgen / light lanes in 062, not a shared Wasm linear-memory
thread runtime (that remains a later, separate option and must not block this).

- **Initialization (once):** bindgen URLs; asset bytes once; shared control
  buffer; shared input arena; shared output arena; ABI version + capacities.
  The worker must not parse the asset pack per request.
- **Job header:** request id; compile generation id; target section count;
  input offset/length; output offset/capacity; flags; status (empty, pending,
  running, complete, stale, failed, overflow); per-section revision table
  offset. Atomics own status/byte publication. Header layout lives in Rust
  constants, mirrored in JS only where the worker glue must touch it.
- **Input payload:** target key list; submitted revisions; compact block-state
  ids for target sections; neighbor section facts needed for culling/AO/light;
  light payloads if read; catalog version. The worker does not request a fresh
  center/radius view.
- **Output payload:** section-addressed, not a whole-view transaction — section
  key; revision; per-section status; visibility bits; vertex/index counts and
  offsets; build stats. Main wasm applies the existing shared stale/revision
  acceptance before mutating the CPU cache. GPU upload stays main-thread.
- **Overflow/backpressure:** split jobs when the output arena is tight; mark a
  job `overflow` with the required byte count; keep any transferred fallback as
  a hard-diagnostic, labeled path; expose pool hits/misses/drops, overflow
  retries, max live capacity, and pending high-water.

## What To Extract / Delete

**Extract / add:**

- Lift the desktop `sync_render_sections_with_budget` streaming loop into a
  shared `EngineRenderSession` method both platforms call (parametrized by
  compiler, budget, ordering, readiness, removal mode).
- Add `WebRenderSectionCompiler: RenderSectionCompiler` in the web client crate,
  over a resident SAB ring.
- One ES module for the residual JS glue; emit the ABI constants from Rust so
  there is one source instead of three hand-synced copies.

**Delete as later stages land (not up front):**

- Web `begin/finish/take` wasm-bindgen exports; the JS `compileCameraView`
  begin->worker->finish chain; the `RenderSectionWorkerCompiler` class.
- Per-request SAB allocation (`createRenderCompiler*Buffer`) -> resident ring.
- `encode_web_render_compile_input` over *all* snapshots -> delta-only against
  the worker's resident mirror.
- `RenderViewCompileQueue<String>` + `loaded_center` — these exist only to
  coalesce JS-driven whole-view requests; under per-frame streaming, coalescing
  is implicit in dirty state and they collapse away. They are the current
  correctness guard against stale-center swaps, so retire them only once the
  streaming loop subsumes that guard.
- The ~35 duplicated symbols across `mclone-web-app.js` / `mclone-web-smoke.js`.

## Risks And Tradeoffs

- **Bounded ring backpressure.** `mpsc` is unbounded; a SAB ring is not. With
  budget ~1 and single-in-flight, pressure is low, but the overflow policy above
  is required.
- **Still not zero-copy.** One SAB->worker-wasm copy in and one out remains (the
  worker reads SAB views, not shared wasm linear memory). That is far cheaper
  than today's encode-all/decode-all. Full zero-copy is the deferred shared Wasm
  thread runtime — out of scope here.
- **Budget tuning on web.** At ~34-66 ms round trips, budget=1 single-in-flight
  catches up a ~12-chunk burst over ~0.5-1 s, but progressively, non-blocking,
  with the cache visible (the goal) versus today's ~262-474 ms semi-blocking
  window. If catch-up feels slow, raise the budget or add a 2-3 worker pool; the
  trait makes that a backend detail.
- **Removal-mode / readiness convergence.** Web currently uses `Defer` +
  `ReadyWithNeighbors`-for-all + coordinate sort; desktop uses
  `ApplyImmediately` + distance readiness + distance sort. Converging the loop
  forces choosing one (desktop's is more mature). This is a behavior change to
  validate, not a mechanical one.
- **Sequencing risk.** The keystone (per-frame streaming, Stage 3) is the
  riskiest change, so it must not be first.

## Implementation Sequence

### Stage 0 — Lock baselines

Capture current movement metrics (submitted sections, packed bytes, window ms,
frame gaps) as a regression fence using the existing instrumentation. No engine
change.

**Captured.** `pnpm native:web:movement-perf` on HEAD `efef4b7` (mobile
viewport, no-clip traverse across 3 chunk boundaries, 2 movement compiles):

| Metric | Baseline |
|---|---|
| submitted sections / compile | 96 and 144 |
| accepted sections / compile | 96 and 144 |
| packed = shared result bytes | 7.69 MB and 11.35 MB |
| shared input bytes / compile | ~210 KB |
| input SAB capacity | exact-fit (~210 KB, realloc per compile) |
| worker round-trip | 34.4–55.1 ms |
| total compile window | 263.7–491.9 ms |
| max frame gap | 12.3–21.1 ms |
| frames / renders advanced during traverse | 91 / 93 |

These match the figures quoted in Current State above. Artifacts saved to
`/tmp/stage0-baseline-movement-perf.json` plus canvas/page PNGs as the
regression fence for later stages (never committed).

### Stage 1 — First safe next step: resident SAB arenas

Allocate input / result / control buffers once at session init and reuse them;
keep the existing begin/finish flow otherwise untouched. Isolated, mechanical,
reversible, and a prerequisite for the ring. Removes per-request SAB churn
immediately.

**Landed.** `RenderSectionWorkerCompiler` in both `mclone-web-app.js` and
`mclone-web-smoke.js` now allocates the render-compiler input, result, and
control `SharedArrayBuffer`s **once** in its constructor
(`createResidentRenderCompilerShared{Input,Result}Buffer`) and re-arms them on
every compile (`armShared{Input,Result}Arena`) instead of
`new SharedArrayBuffer(...)` per request. Re-arm resets the control word
(status/bytes/capacity) and, for input, copies that compile's snapshot bytes
into the resident buffer. The result arena stays a resident 16 MB; the worker's
one-off allocation remains the oversized-result overflow path (unchanged). The
input arena is a resident 1 MB; oversized snapshots grow it in place (realloc +
`console.warn` + `sharedInputGrowCount`). Single-in-flight — the `pendingCompile`
busy flag in the app, sequential `await compiler.compile(...)` in the smoke —
makes reuse race-free. The begin/finish flow, worker protocol
(`mclone-render-compiler-worker.js`), and ABI constants are otherwise untouched.

Validated against the Stage-0 baseline: submitted/accepted sections, packed
bytes, and shared input/result bytes are **byte-identical**; the only metric
delta is input SAB capacity moving from per-request exact-fit (~210 KB,
reallocated every compile) to a stable resident 1 MB, and the movement run
logged **zero** input-arena grows. Worker round-trip / window / frame-gap
unchanged within jitter. Render output identical (movement-perf canvas
pixel-identical to baseline; app-smoke terrain verified). Green:
`cargo test` (668 passed), `cargo check` wasm, `node --check` on all four JS
files, `native:web:build`, `native:web:app-smoke`, `native:web:movement-perf`.

### Stage 2 — Web `RenderSectionCompiler` over the resident ring

`submit` fills the input arena + doorbell; `try_recv_completed` polls the result
control word from main wasm. JavaScript becomes doorbell-only. Keep the
whole-view budget here to isolate the transport change from the scheduling
change.

**Landed.** `WebRenderSectionCompiler` in `web_canvas.rs` now implements
`mclone_render_session::RenderSectionCompiler` over a **Rust-owned** resident SAB
ring (input control+data and result control+data buffers allocated once via
`SharedArrayBuffer::new`, mirroring the 062 `RunnerSharedSlot` pattern). `submit`
encodes the request snapshots straight into the resident input arena and arms the
input + result control words via `js_sys::Atomics`. `try_recv_completed` polls the
result control word from main wasm with `Atomics::load`, reads the packed bytes
back through a `Uint8Array` view of the resident result buffer, and decodes them
in place into a `RenderSectionCompileResult` — **no JavaScript in the result data
path**. The decode of unreported sections reuses the same missing-key→stale rule
as the legacy worker path (`render_section_compile_result_from_report`).

New wasm-bindgen exports drive it: `renderCompilerSharedSupported`,
`submitCameraRenderCompile` (initial), `submitDeferredCameraRenderCompile`
(movement, preserving the runner-settle/center-loaded wait), and
`pollCameraRenderCompile` (poll → finish → render). Per 067's target,
**JavaScript posts the tiny doorbell**: the submit export returns a descriptor
carrying the request id, target sections, and the resident shared buffers; the JS
`RenderSectionWorkerCompiler.compileWithDoorbell` relays it to the worker (which
writes the result into the same buffers main wasm polls) and resolves only the
worker **metrics** report. The app's `compileCameraView` branches on
`sharedCompileSupported()`: submit → post doorbell → `pollSharedCompileToCompletion`
loop (yields a frame per poll so presentation continues during the compile).

The budget stays `usize::MAX` (whole-view), so this is a pure transport swap. The
result on `pnpm native:web:movement-perf` is **byte-identical** to the Stage-0
baseline: per-compile 96/144 submitted = accepted sections, 7.87 MB / 11.62 MB
packed = shared-result bytes, ~215 KB shared input, zero overflow,
`shared-result-buffer` transport, snapshot-input compile (no generated-view
fallback). Frames and renders advance during each compile (frameΔ 92 / renderΔ
94 over a 3-boundary traverse). `native:web:app-smoke` is green with the same
shared-result-buffer diagnostics and terrain capture.

Fallbacks stay labeled. The legacy begin/finish exports + JS `compile()` path are
retained for non-cross-origin-isolated browsers without `SharedArrayBuffer`
(`message-transfer`) and also still back the deterministic overview begin/finish
smoke. A worker result that overflows the resident 16 MB buffer is a labeled
hard-diagnostic: `try_recv_completed` records `sharedResultOverflowCount`, grows
the resident buffer for the retry, and fails that compile so the sections
requeue (dormant at the ~11.6 MB worst case). Known asymmetry to reconcile in
Stage 5: only `mclone-web-app.js` gained `compileWithDoorbell`; the duplicated
`mclone-web-smoke.js` class keeps the legacy `compile()` only, since the standard
smoke exercises the overview path.

Green: `cargo test` (no failures), `cargo check` wasm, `node --check` on the four
JS files, `native:web:build`, `native:web:app-smoke`, `native:web:movement-perf`.

### Stage 3 — Keystone: web drives `sync_render_sections_with_budget` (budget ~1)

**Landed (keystone).** The desktop streaming loop is lifted into a shared
`EngineRenderSession::sync_render_sections_with_budget<C: RenderSectionCompiler>`
method (parametrized by compiler, budget, ordering closures, readiness, removal
mode, and a snapshot collector) plus a shared `has_pending_render_work` idle gate.
The distance ordering + near-camera readiness helpers
(`render_section_neighbor_readiness`, `sort_chunk_positions_by_distance`,
`sort_dirty_section_chunks_by_distance`, `render_section_center`) moved into
`mclone-render-session` so both platforms run byte-identical policy. Desktop
`scene_runtime.rs` now delegates to the shared method (its local copies deleted);
`cargo test`, `native:movement:smoke`, and `native:timedemo:smoke` stay green.

Web drives the same loop at budget 1 over the Stage-2 `WebRenderSectionCompiler`.
A single new export, `syncCameraRenderFrame(radius)`, runs one frame: update the
deferred interest center, drain runner updates (now `EngineServerUpdateDirtyPolicy::ALL`
so chunk snapshots/unloads mark render-dirty event-driven like desktop — the
web-only view-sync delta is retired), run the budget-1 shared sync over the
resident-ring compiler (distance sort + near-camera readiness +
`ApplyImmediately`), render the current cache (sky-only tolerated until sections
stream), and return the frame report plus a worker `doorbell` whenever a new
compile was armed that frame. JS is a doorbell + lifecycle relay: `tickFrame` does
`advanceCameraFrame` -> `syncCameraRenderFrame` -> post the doorbell
(fire-and-forget; the worker writes the packed result into the resident ring that
the next frame's poll drains). The `compileCameraView` begin->worker->finish
transaction, the `RenderViewCompileQueue`/`loaded_center` coalescing, the busy
flag, and the JS-owned per-request SAB arenas are gone from the live path.

Convergence vs Stage 0 (mobile `native:web:movement-perf`, no-clip traverse):

| Metric | Stage 0 baseline | Stage 3 |
|---|---|---|
| compiles / traverse | 2 (whole-view) | 9 (per-frame) |
| submitted sections / compile | 96 and 144 | 16 (one chunk column) |
| packed = shared-result bytes | 7.69 / 11.35 MB | 0.78–1.73 MB |
| worker round-trip | 34–55 ms | 6–7 ms |
| per-compile window | 262–492 ms total | 7.5–18 ms each |

The 96–144-section mega-job is gone; movement fills progressively (stale-drop +
accept stream visible) while the resident cache stays drawn. `app-smoke`,
`mobile-smoke`, and `movement-perf` are green; the harness now asserts many small
per-frame compile timings (same per-compile diagnostics) and a `streamingSettled`
gate (render-idle **and** the server runner drained, so a fast camera that outruns
chunk generation does not settle prematurely). The web app requires cross-origin
isolation (the streaming compiler needs the SAB ring; no non-isolated fallback).

Boot determinism: the warm-up applies the server-spawn snap with ~0 dt and
pre-loads the server-spawn chunks before the live rAF gravity fall, so the camera
settles at the same sub-block position every run despite per-compile frame-time
variance (the pre-streaming app fell during uniformly fast render-only frames).

**Legacy-deletion tail (landed).** The deterministic overview smoke
(`mclone-web-smoke.js`) was converted from the `beginChunkRenderCompileRequest` ->
`compiler.compile()` -> `finishChunkRenderCompileRequest` transaction to a
`streamOverviewToIdle` pump over `syncOverviewRenderFrame` for centers `(0,0)` then
`(1,0)` — the web analog of desktop `sync_all_render_sections`. The pump relays each
frame's worker doorbell via the shared `compileWithDoorbell` shim, yields to the
event loop every frame so the runner/worldgen/light workers' messages flow, and
accepts "settled" only after observing the runner do work for that center (a center
jump can momentarily look drained before its generation jobs propagate — the same
premature-settle race the keystone hit). With that gate the smoke streams real
incremental compiles (center `(0,0)` ~13, center `(1,0)` ~9) and renders the same
terrain column. With the overview smoke off the begin/finish path, the dead Rust was
deleted: the `renderChunkReport*` / `begin/finish/submit/poll/request` chunk+camera
compile `#[wasm_bindgen]` exports and their whole internal cascade
(`submit_shared_render_compile`, `poll_camera_render_compile_result`,
`*_request_to_js_value`, `write_compile_queue_request`, `write_compile_scope_report`,
`write_runner_wait_diagnostics`, `WebSharedCompileContext`, `WebPendingCompileContext`,
…), the `RenderViewCompileQueue`/`loaded_center`/`compile_requests`/`shared_compile`
fields, and the unused imports — `cargo check --target wasm32-unknown-unknown` is now
warning-clean. `pendingChunkRenderCompileJobCount` simplified to
`render_compiler.pending_job_count()`. The non-SAB `message-transfer` fallback in the
web app stays gone (init throws without cross-origin isolation). **Kept:**
`render_section_compile_result_from_report`, `WebRenderSectionCompiler`,
`render_chunk_report_with_cache_update`, the `WebRenderCompilerSession` exports, and
all `sync_*_render_frame` methods; `mclone_web_render_generated_chunk_report` is still
a live export, so `render_chunk_report_for_center` ->
`prepare_chunk_render_plan`/`prepare_chunk_view`/`finish_render_compile_result` stay
(the begin/finish-era enumeration of "delete prepare_*" was superseded by what `cargo`
actually reports unused). Note: the deterministic smoke no longer asserts the *render
session's* runner frame-metrics transport — the streaming runner ships its tiny
command/update frames over shared-memory **or** message-transfer depending on payload
size (non-deterministic run to run); the heavy worldgen/light lanes stay
deterministically shared-memory, and the shared-memory runner capability is still
asserted by `sharedTopologyStress`.

### Stage 4 — Resident snapshot mirror; request input becomes delta-only

The biggest payload win; aligns input with the ABI input spec above.

**Landed.** The render-compile worker (`WebRenderCompilerSession`) now holds a
resident `BTreeMap<ChunkPos, ChunkSnapshot>` snapshot mirror, created once with the
session and persisted across compiles, so the per-frame SAB input is a *delta*
instead of the whole loaded world. Web-only and behind the `RenderSectionCompiler`
trait — desktop still moves `Vec<ChunkSnapshot>` over `mpsc` with zero serialization
and is untouched (`native:movement:smoke` / `native:timedemo:smoke` trivially green).

- **Delta on submit (main wasm).** `WebRenderSectionCompiler` keeps a
  `BTreeMap<ChunkPos, ChunkRevision>` shadow of what the worker mirror holds.
  `submit` diffs the request's loaded snapshots against it: a `(pos, revision)` that
  differs or is absent is an **upsert**; a tracked position no longer loaded is an
  **eviction**. Only that delta is encoded into the resident input arena
  (`encode_web_render_compile_delta`, distinct `MCWRCD1` magic). The shadow is
  advanced **on submit** (apply-on-submit) so a compile requeued after a result
  overflow ships a minimal delta — the worker already applied those columns. An empty
  shadow (first compile, or post-failure recovery) is a **full resync**: every loaded
  column ships as an upsert under a fresh mirror epoch.
- **Mirror on receipt (worker).** `compileSnapshotSectionsForTargets` decodes the
  delta, applies upserts + evictions to the mirror **before** compiling (so the
  advance survives an overflow requeue), then builds the target sections from the
  mirror's resident columns (`build_render_sections_from_snapshots` is now generic
  over `Borrow<ChunkSnapshot>`, so the mirror compiles from `&[&ChunkSnapshot]` with
  no deep clone). Eviction keeps the mirror bounded to the loaded view.
- **Desync tripwire.** Each delta carries a mirror **generation**. A `reset` delta
  clears the mirror and adopts the generation; a non-reset delta whose generation
  does not match is a desync (e.g. a worker that silently lost its mirror) and is
  rejected loudly rather than compiled against a partial mirror. On a worker-reported
  failure, main wasm drops its shadow so the next submit is a full resync.

Convergence vs Stage 0 (web smokes; output is the Stage 3 fence, unchanged):

| Metric | Stage 0 baseline | Stage 4 |
|---|---|---|
| SAB input bytes / compile | ~206–215 KB (all 9 loaded chunks, every compile) | 24 B–122 KB (delta) |
| input chunk columns / compile | 9 (whole world) | 0–5 upserts (changed only); 24 B header when 0 |
| worker resident mirror | none (re-ships every frame) | 9 chunks, bounded across the `(0,0)→(1,0)` jump by evictions |
| submitted sections / compile | 96 and 144 | 16 (unchanged from Stage 3) |
| packed = shared-result bytes | 7.69 / 11.35 MB | 0.78–1.73 MB (unchanged from Stage 3) |
| worker round-trip | 34–55 ms | 5.2–11.2 ms (unchanged from Stage 3) |
| max frame gap | 12.3–21.1 ms | ≤10.2 ms (unchanged from Stage 3) |

A steady-state re-compile of an already-resident column ships just the 24-byte delta
header (`native:web:chunk-smoke`: center `(0,0)` 13 compiles then `(1,0)` 9 compiles,
both settling on a 24-byte input with a 9-chunk mirror, terrain pixel-shape identical).
Movement increments ship only the columns that crossed a boundary that frame (1 upsert
≈ 24 KB, 3 ≈ 68 KB, a 5-column burst ≈ 122 KB), and resident columns ship nothing.
Diagnostics keep `snapshotInputCompileUsed=true` / `generatedViewFallbackUsed=false`
and add `snapshotInputUpsertCount` / `snapshotInputEvictionCount` /
`snapshotMirrorChunkCount` from the worker session; the `assertCompileTimingDiagnostics`
chunk-count lower bound was relaxed to allow a 0-upsert compile (the delta byte length,
always > 0, carries the assertion).

Green: `cargo test` (no failures), `cargo check -p mclone-web-client --target
wasm32-unknown-unknown` (warning-clean), `node --check` on the three `www/*.js` +
`browser-smoke.mjs`, `native:web:build`, `native:web:chunk-smoke` (+canvas inspected),
`native:web:app-smoke`, `native:web:mobile-smoke`, `native:web:movement-perf`,
`native:movement:smoke`, `native:timedemo:smoke`.

Follow-up (not blocking): the web snapshot collector still
`client.chunk_snapshots().cloned().collect()`s every loaded column each frame before
`submit` diffs them — a main-thread clone, not a SAB cost. Eliminating it needs a
web-specific collector with access to the shadow map; it is a secondary micro-opt now
that the SAB-bytes win is banked.

### Stage 5 — JS dedup + single-source ABI constants

Collapse the ~35 shared symbols into one module; emit the ABI from Rust.

**Least-convoluted path to "close to desktop":** Stages 1->3 alone get web onto
desktop's topology and kill the mega-job, mostly by deleting JavaScript rather
than adding abstraction. Stage 4 is the payload optimization. Do not reach for
shared Wasm linear memory.

## Validation

- `cargo test --manifest-path native/Cargo.toml` after each stage (shared policy
  has dense unit coverage).
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target
  wasm32-unknown-unknown`.
- `node --check` on `mclone-web-app.js`, `mclone-web-smoke.js`,
  `mclone-render-compiler-worker.js`, and `scripts/browser-smoke.mjs`.
- `pnpm native:web:build`, `pnpm native:web:app-smoke`,
  `pnpm native:web:mobile-smoke`, `pnpm native:web:movement-perf`.
- `pnpm native:movement:smoke` and `pnpm native:timedemo:smoke` for the
  streaming/camera paths.
- Per stage, assert against the Stage-0 baselines: submitted-section count,
  packed/shared bytes, worker round-trip ms, total movement window ms, max frame
  gap. Stage 3 should show submitted sections drop from ~96-144 to single digits
  per frame and the window stop being a single large block.
- Rendered-output check: headless/Playwright capture to `/tmp` and **look at
  it** at the first drawable milestone of Stage 3, then again after Stage 4.
- Keep a deterministic smoke entry that pumps frames to idle (the web analog of
  desktop `sync_all_render_sections`) so begin/finish removal does not lose
  coverage.

## Non-Goals

- No legacy TypeScript engine edits.
- No new multi-megabyte `postMessage` transfer path unless explicitly labeled
  fallback/debug with metrics.
- No desktop transport rewrite; desktop must not pay web serialization costs for
  symmetry.
- No shared Wasm linear-memory thread runtime in this tactical; the explicit
  worker interface remains the contract even if that lands later.
- No change to worldgen correctness, chunk publication, or the server-runner /
  worldgen / light / transport lanes owned by 062.
- `wasm32-unknown-unknown` remains the browser target.

## Relationship To Other Tacticals

- `061-shared-engine-web-adapter-refactor.md` owns the shared Rust
  render-session boundary this builds on.
- `062-shared-threading-topology.md` owns the parent native-thread/Web-Worker
  topology and the runner / worldgen / light / transport shared-memory lanes.
  This tactical's render-compile backend is the render lane of that topology.
- `065-native-web-mobile-streaming-performance.md` (superseded) — its movement
  perf goal is met here by the per-frame streaming change.
- `066-web-shared-memory-worker-architecture.md` (superseded) — its render
  worker SAB ABI and JS-shrink boundary are carried forward above.
</content>
</invoke>
