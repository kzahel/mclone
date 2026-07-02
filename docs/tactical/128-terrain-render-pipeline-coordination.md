# 128: Terrain Render Pipeline Coordination

Status: active architecture parent
Workstream: shared native Rust terrain runtime, render session, compiler, upload, and XR frame pacing

## Impetus

The recent terrain pacing work has produced several useful cross-sections, but
the cross-sections are starting to describe one larger system:

- which render sections become dirty,
- which dirty sections are allowed to become compile work,
- how that work crosses from the render frame into workers,
- how completed worker output is accepted,
- how accepted mesh data is uploaded to GPU resources,
- how uploaded sections become drawable,
- how much of each phase may run on a frame that still has to hit XR timing.

The pressure-control changes in `119` and `120` were the first real step toward
a stable shape. They made terrain compile admission bounded, moved the product
lane to render distance 7 for live Quest testing, and gave us enough
instrumentation to stop guessing about the biggest bursts. The next problem is
less about one more cap and more about the coordinating bus: the state machine
that moves a section through dirty, ready, inflight, completed, uploaded, and
drawable states without doing hidden full-set work on the render frame.

This tactical exists because the narrow documents now converge:

- `119` sees live XR frame drops and asks for burst control.
- `120` argues the baseline should be closer to Java's bounded compile
  dispatcher before we invent mobile-specific policy.
- `127` shows our compile input shape is still coarser than Java's
  `RenderChunkRegion` and that cloning is no longer the only suspect.
- `117` keeps the GPU and geometry work in view so the terrain bus does not
  become CPU-only tunnel vision.

The current evidence also changed the priority. On the latest Quest Android XR
render-distance-7 settled-orbit run, splitting runtime submit work showed:

- `submit_snapshot_ms` max: about `0.979ms`
- `submit_handoff_ms` max: about `13.873ms`
- `upload_apply_ms` max: about `10.743ms`
- left-eye terrain encode max: about `19.994ms`

That means the immediate long pole is not the five-column snapshot clone by
itself. The remaining `runtime_submit_ms` bucket is likely dominated by request
construction, compiler handoff, inflight/dirty mutation, ready-plan application,
or other coordination work. The doc below frames that work against the Java
client architecture so implementation chunks can be chosen from a coherent
baseline instead of from ad-hoc throttles.

## Architectural Conclusion

The Java reference is not merely a source of tactical tricks. It has a clearer
responsibility split than our current native terrain path:

- `ViewArea` owns the resident moving grid of render-chunk slots.
- `RenderChunk` owns one slot's lifecycle: origin, dirty state, current compiled
  output, buffers, and task cancellation.
- `LevelRenderer` owns frame-time admission: which dirty chunks are scheduled
  before the deadline.
- `ChunkRenderDispatcher` owns queue coordination: priority, backpressure,
  reusable compile buffers, worker starts, upload queue counters, and queue
  health.
- `RenderChunkRegion` owns the compile input boundary.

Our current native system has many of the same concepts, but they are flatter
and spread across functions and sets in several modules. Dirty state,
ready-plan construction, inflight marking, worker capacity, completed-result
acceptance, upload application, prepared-record maintenance, and traversal-ready
publication all exist, but the ownership line is harder to see. That is why the
same terrain pressure problem keeps reappearing as separate cross-sectional
tacticals.

The conclusion to draw is that we should refactor toward Java-like owners,
adapted to Rust, `wgpu`, XR, and web workers. This does not mean copying the
Java classes mechanically. It means giving the native pipeline the same kind of
named boundaries:

| Reference owner | Responsibility | Native refactor target |
| --- | --- | --- |
| `ViewArea` / `RenderChunk` | Resident view slots and per-slot lifecycle | Resident render-section or render-column slot state in shared render session |
| `LevelRenderer` | Frame admission and deadline policy | Runtime admission policy over the shared coordinator |
| `ChunkRenderDispatcher` | Priority queue, capacity, worker starts, upload queue pressure | Shared terrain render dispatcher/coordinator boundary |
| `RenderChunkRegion` | Small immutable compile input | Shared `RenderCompileRegion` contract, backed by snapshots or resident mirrors |
| `CompiledChunk` atomic publish | Old mesh stays visible until replacement is complete | Explicit compiled/uploaded/drawable state transitions |

The performance reason is concrete. A flat set-moving model can appear bounded
by chunk budget while still doing work proportional to all dirty or deferred
sections. A resident lifecycle model makes the common transitions local:
mark dirty, try ready, mark inflight, accept result, upload, publish drawable.
It also gives every phase one owner for counters and budgets.

The refactor should be staged and measurement-driven. We should not first do a
large rewrite. But the next instrumentation is not neutral bookkeeping; it is
the first probe that tells us which part of the coordinator to extract first.
If `apply_ready_plan` dominates, the first refactor target is resident lifecycle
state and dirty/deferred ownership. If compiler submit dominates, the first
target is dispatcher/request transport. If upload dominates next, the first
target is an explicit upload queue/budget owner.

## Related tacticals

| Doc | Angle | How it feeds this parent |
| --- | --- | --- |
| `024-render-section-dirty-cache-and-upload-diffs.md` | Dirty cache and upload diffs | Early section-cache/update contract. This parent treats it as part of the dirty-to-upload bus. |
| `032-native-neighbor-stable-render-boundaries.md` | Neighbor readiness | Established that render-section visibility and face correctness depend on stable neighbors. This parent rechecks that against Java's `hasAllNeighbors` and `RenderChunkRegion`. |
| `033-native-async-render-section-compile-queue.md` | Async compile queue | First async worker path. This parent asks whether the queue now needs a Java-style central dispatcher/mailbox shape. |
| `034-native-render-compile-revisions-and-priority.md` | Revisions and priority | Revision-based stale-result rejection is already one of our strongest parity pieces. This parent keeps it in the section lifecycle model. |
| `067-shared-render-worker-architecture.md` | Shared worker architecture | Keeps native and web behind one compiler/session contract while allowing OS threads or Web Workers. This parent uses that as the platform boundary. |
| `113-block-edit-render-coherence.md` | Local edit coherence | Java has a player-dirty synchronous rebuild path. This parent tracks the equivalent user-visible coherence problem without blindly copying sync rebuild into XR. |
| `117-android-xr-rd10-gpu-floor-and-frame-overlap.md` | GPU and geometry pressure | Keeps upload, encode, draw count, and greedy-meshing ideas attached to the same terrain lifecycle instead of treating CPU pacing as the whole problem. |
| `119-android-xr-live-streaming-frame-pacing.md` | Live XR burst avoidance | Current product-lane checklist and Quest evidence. This parent should feed its next implementation slices. |
| `120-vanilla-render-compile-backpressure.md` | Java-style compile admission | The closest existing doc to the Java baseline. This parent broadens it from admission/backpressure into the whole dirty-to-drawable bus. |
| `127-render-compile-region-parity.md` | Compile-region ownership | Tracks the input-data shape for worker compile. This parent uses it after the submit handoff bucket is split. |

## Java 1.17.1 Reference Shape

Primary source files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/RenderChunkRegion.java`

### Phase 1: Render chunks are stable slots

Java's `ViewArea` owns a fixed array of `ChunkRenderDispatcher.RenderChunk`
objects for the current view area. Camera movement repositions these slots by
changing their origins rather than allocating an entirely new render-chunk graph.
Each `RenderChunk` owns:

- an origin and six cached neighbor origins,
- a dirty bit plus a `playerChanged` dirty bit,
- a current `CompiledChunk` in an atomic reference,
- one `VertexBuffer` per chunk render layer,
- references to the last rebuild and translucency-sort tasks so they can be
  cancelled.

Performance implication: most of the world-to-render mapping is resident state.
Movement updates origins and dirty state, while old compiled data remains visible
until replacement work is published.

### Phase 2: Dirty chunks feed a renderer-owned compile set

`LevelRenderer` keeps `chunksToCompile`, a linked set of dirty `RenderChunk`
objects. During setup/update it collects chunks that need rebuilds. Dirty chunks
from local player edits carry the `playerChanged` bit so they can be treated
differently from background streaming work.

`compileChunksUntil(deadline)` is the key admission loop. It first drains the
dispatcher upload queue, then iterates `chunksToCompile`. For each chunk:

- player-dirty chunks are rebuilt synchronously with fixed buffers,
- normal chunks create an async task and are scheduled into the dispatcher,
- the dirty bit is cleared once the rebuild or scheduling step is accepted,
- the loop measures average scheduling cost so far and breaks when the remaining
  deadline is smaller than that average.

Performance implication: Java does not push all dirty chunks into worker work in
one frame. It has a simple adaptive deadline gate around scheduling, and the
local-edit exception favors visual coherence over background throughput.

### Phase 3: The dispatcher is a mailbox plus bounded buffers

`ChunkRenderDispatcher` owns three important queues:

- `toBatch`: a priority queue of compile tasks, ordered by distance at task
  creation,
- `freeBuffers`: a bounded pool of `ChunkBufferBuilderPack` instances,
- `toUpload`: a concurrent queue of upload runnables.

It also owns a `ProcessorMailbox<Runnable>`. Scheduling a compile task posts to
the mailbox, inserts into `toBatch`, updates counters, and calls `runTask`.
`runTask` only starts work when both a compile task and a free buffer pack are
available. When a task completes, the buffer pack is returned through the
mailbox and another task is attempted.

The number of buffer packs is bounded from available processors and heap budget.
On 32-bit it is capped more aggressively. If allocation fails, Java backs off
the pool size.

Performance implication: worker concurrency and compile memory are the same
resource. A task cannot start just because a thread exists; it needs a reusable
buffer pack. The mailbox also gives the dispatcher one serialized coordination
point for queue counters, free buffers, and follow-up dispatch.

### Phase 4: Compile input is a small render region

`RenderChunk.createCompileTask()` captures a `RenderChunkRegion` using the target
section bounds expanded by one block and then expanded by a chunk-radius
parameter. `RenderChunkRegion.createIfNotEmpty`:

- grabs the relevant `LevelChunk` references,
- skips the task entirely when the target Y span is empty,
- builds a compact block-state array for the padded region,
- keeps access to the underlying chunks for block entities, light, tint, and
  world metadata.

The compile loop still iterates only the 16x16x16 target section. Neighbor data
is available through the region for model, liquid, occlusion, AO, tint, and
lighting queries.

Performance implication: Java's worker task does not receive a whole render
distance snapshot. It receives a small, task-local region with exactly the
neighbor reads needed by the block renderer. That region is still a copy of
block states, but its ownership and size are tied to one render chunk.

### Phase 5: Missing neighbors cancel background work

Before rebuilding, `RenderChunk.hasAllNeighbors()` checks whether horizontal
neighbors exist. For chunks close to the camera, Java allows the rebuild anyway.
For farther chunks, missing cardinal neighbors cancel the task.

Performance implication: the baseline is not "wait for every possible neighbor
always." It has a near-player exception and otherwise avoids compiling far
sections whose visible faces may be unstable because adjacent chunks are absent.

### Phase 6: Worker compile produces layer buffers and visibility

The rebuild task:

- creates a fresh `CompiledChunk`,
- iterates all target block positions,
- marks opaque positions in `VisGraph`,
- collects renderable/global block entities,
- emits fluid and block geometry into per-layer builders,
- stores translucent sort state when needed,
- resolves the visibility set,
- ends only the layers touched by the section.

Performance implication: Java's render compile is a real renderer pass on the
worker side. The output is layered geometry, visibility metadata, block entity
metadata, and translucent sort state, not just a mesh blob.

### Phase 7: Upload is explicit and publish waits for upload

For each layer present in the compiled chunk, the rebuild task schedules
`uploadChunkLayer`. Upload work is routed through the dispatcher's `toUpload`
queue and then into `VertexBuffer.uploadLater`. `LevelRenderer` calls
`uploadAllPendingUploads()` from the render path before scheduling more compile
work.

The task publishes the new `CompiledChunk` into the render chunk's atomic
reference only after the layer upload futures complete. If the task is cancelled
before that point, publication is skipped.

Performance implication: the renderer keeps old compiled content while new work
is in progress. A section becomes drawable atomically at the end of the compile
and upload chain, not halfway through.

### Phase 8: Java exposes queue health directly

The dispatcher reports pending compile count, pending upload count, and available
buffer count. These counters are not just debugging trivia; they are the visible
shape of the pressure-control system:

- pending compile work,
- pending upload work,
- compile resource availability.

Performance implication: a useful parity baseline should expose the same kinds
of counters in our runtime and make phase budgets reason about them directly.

## Java Perf Characteristics To Preserve First

The baseline we should assume correct until measurement says otherwise:

1. Keep render-section slots resident and publish replacements atomically.
2. Bound worker starts by reusable compile-buffer capacity, not only by thread
   count.
3. Schedule dirty work through one coordination owner with distance priority.
4. Gate scheduling by frame deadline or measured remaining headroom.
5. Drain and measure upload as its own explicit phase.
6. Keep old compiled geometry drawable while replacement work is pending.
7. Use small compile regions rather than whole-view snapshots.
8. Treat local-player edits as a separate coherence class.
9. Report compile queue, upload queue, and available compile resources.

Where we may need measured divergence:

- Java targets a different runtime stack: Java objects, OpenGL-style buffers,
  desktop-oriented 60 Hz assumptions, and no standalone Quest compositor budget.
- Java's `uploadAllPendingUploads()` drains all pending uploads. That may be too
  bursty for Android XR even if it is the correct baseline to test first.
- Java's synchronous player-edit rebuild may be unacceptable on XR if the
  section is complex. We should preserve the coherence goal, but the native
  implementation might use a high-priority tiny async path or an upload budget
  instead of a direct synchronous compile.

## Current Native Shape

Primary source files:

- `native/crates/mclone-render-session/src/lib.rs`
- `native/crates/mclone-app-runtime/src/lib.rs`
- `native/crates/mclone-app-runtime/src/render_assets.rs`
- `native/crates/mclone-xr-scene/src/lib.rs`
- `native/apps/mclone-android-xr-client/src/lib.rs`

### Resident state we already have

`mclone-render-session` owns the core shared terrain cache and dirty state:

- `RenderSectionDirtyState` tracks dirty chunks, dirty sections, inflight
  sections, and per-section revisions.
- `RenderSectionDirtyWork` classifies dirty work into stale, loaded, and removal
  buckets.
- `RenderSectionReadyPlan` separates ready section keys from deferred section
  keys and tracks chunk-budgeted loaded/dirty work.
- `RenderSectionSyncPlan` carries dirty work plus ready work through the shared
  compile loop.
- Completed compile results are revision-checked before acceptance, so stale
  worker output cannot overwrite newer dirty state.

This is already stronger than a naive worker queue. We have the important
concepts of dirty, deferred, inflight, completed, stale, removal, and ready.
The weakness is not that the concepts are missing. The weakness is that they are
not yet gathered behind a small number of durable owners, so reasoning about one
section's path from dirty to drawable still requires following several modules
and several transient set transformations.

### Shared admission loop

`EngineRenderSession::sync_render_sections_with_budget...` drives the shared
loop:

1. drain completed compiler results,
2. accept some completed results, with an optional acceptance budget,
3. seed dirty work when the cache is empty,
4. prepare a ready plan under a chunk budget,
5. skip submission when the compiler has no pending-job capacity,
6. collect snapshots and submit the compile request,
7. merge cache-update diagnostics.

This is the right architectural boundary: native, web, and XR can keep different
transport details behind `RenderSectionCompiler` while running the same dirty
and admission policy.

### Native worker shape

`RenderSectionCompileWorker` currently uses native OS worker threads, an `mpsc`
command channel, an `mpsc` result channel, and a `pending_jobs` counter bounded
by `max_pending_jobs`. The current Quest live lane has used two workers in
recent measurements.

This gives us Java-like capacity pressure at the job level, but it is not yet
the same as Java's dispatcher:

- there is no explicit mailbox object that owns all compile queue mutation,
- the reusable compile-buffer pool is not modeled as the resource that starts
  work,
- upload queue pressure is tracked elsewhere,
- the compile request payload is a set of section keys plus chunk snapshots, not
  a small `RenderChunkRegion`-style object.

### Current data movement shape

The current native request contains:

- target render-section keys,
- per-section revisions,
- a `Vec<ChunkSnapshot>` for the needed chunk columns.

Recent work reduced this from broad snapshot submission to targeted target-plus
horizontal-neighbor submission. On the latest Quest run, the snapshot clone and
selection bucket no longer dominated. The result does not mean snapshots are
finished; it means the next bottleneck is now more likely the coordination work
around building and applying the request.

### Current diagnostics shape

Android XR now reports useful terrain runtime and upload markers, including:

- pending chunks before/after,
- pending jobs and available slots,
- deferred sections,
- submitted sections,
- deadline-skipped requests,
- completed and stale sections,
- uploaded and removed sections,
- ready and drawn sections,
- runtime submit split into snapshot and handoff buckets.

The next missing diagnostic is inside `submit_handoff_ms`. It needs to be split
farther before we optimize it.

## Current Shortfalls And Bottleneck Hypotheses

The main shortfall is organizational before it is algorithmic. We should expect
some measured cost to come from concrete data structures, but the deeper problem
is that the current architecture does not make the hot path's ownership obvious.
That weak ownership makes it easier for bounded admission to be followed by
unbounded ready-plan or upload work.

### 1. The section bus is fragmented

The state machine spans:

- render-session dirty and ready-plan structures,
- app-runtime polling and deadline policy,
- native worker request/result transport,
- XR scene upload application,
- prepared draw-record maintenance,
- renderer traversal/drawable caches.

That is workable, but there is no single named "terrain render bus" contract
with phase budgets, counters, and invariants. The risk is death by local fixes:
one cap in admission, another cap in upload, another special case in prepared
records, without a shared lifecycle that explains when a section is allowed to
advance.

Desired direction: keep the implementation modular, but extract a named shared
terrain render coordinator/dispatcher boundary that owns lifecycle transitions
and exposes phase counters end to end.

### 2. `submit_handoff_ms` is too coarse

The measured `submit_handoff_ms` bucket includes several very different costs:

- compile-request construction from `RenderSectionReadyPlan`,
- collecting target-section revisions,
- moving or sending the request into the compiler,
- marking ready sections inflight,
- removing ready work from dirty sets,
- re-extending deferred section keys,
- generating the cache update/diagnostic report.

These should not be optimized as one bucket. The first implementation chunk
should split them. If the largest cost is set mutation, the fix is data
structure/lifecycle work. If it is `mpsc` send or request move, the fix is
transport/ownership work. If it is revision collection, the fix is compact
section IDs or a narrower request object.

### 3. Dirty/deferred bookkeeping may be O(total dirty) on hot frames

`RenderSectionDirtyState`, `RenderSectionDirtyWork`, and `RenderSectionReadyPlan`
use `BTreeSet` and `BTreeMap` heavily. That is simple and deterministic, but
movement frames can have thousands of deferred sections. The current
`apply_ready_plan` removes ready work and extends dirty sections with all
deferred keys. If deferred sets are repeatedly rebuilt and reinserted, a small
submitted job can still pay a large coordination cost.

Hypothesis: after the next split, ready-plan application or dirty/deferred set
maintenance will explain a large part of `submit_handoff_ms`.

Potential fixes if confirmed:

- store per-section state in a chunk-keyed state machine instead of repeatedly
  moving keys between sets,
- keep deferred work in per-chunk buckets and only revisit chunks whose
  readiness changed,
- use compact section IDs and indexed bitsets for current view sections,
- avoid materializing a full `deferred_section_keys` set when only counts and
  "try again later" ownership are needed,
- make readiness invalidation event-driven when neighbor chunks arrive/remove.

### 4. Our compile input is still not Java-shaped

`127` captured the input mismatch. Java compiles from `RenderChunkRegion`, a
small padded region bound to one render chunk. Native currently submits owned
chunk snapshots. Targeted snapshots made that cheaper, but the abstraction is
still column-centric rather than render-region-centric.

Potential shortfalls:

- more data crosses into the worker than one section needs,
- section compile cannot naturally express "this one-block padded region is the
  immutable input",
- native and web have different transport pressure, even though they share the
  trait,
- future AO/light/tint/fluid parity may need the padded-region contract anyway.

This should probably come after the `submit_handoff_ms` split unless snapshot or
request construction reappears as the long pole.

### 5. Upload is an explicit phase, but not yet a Java-parity baseline

Java has an explicit pending upload queue and drains it from `LevelRenderer`.
Our XR path has upload application markers and has already shown about `10.743ms`
max upload apply in a recent run. That is large enough to be a first-class
terrain bus phase.

Unknowns:

- how many section uploads are applied on the worst frames,
- whether one section can contain a pathological amount of geometry,
- whether uploads should be drained like Java first, then budgeted only if Quest
  data proves the Java baseline too bursty,
- whether resource creation/update shape in `wgpu` is the bigger issue than
  section count.

### 6. Drawable record maintenance and encode can still dominate

The same run that showed `submit_handoff_ms` had a left-eye terrain encode max
around `19.994ms`. That means the coordination bus must not stop at upload. A
section is not "free" when it becomes compiled; it can still cause prepared
record churn, traversal updates, cull cost, command encoding cost, and GPU work.

Potential fixes if encode remains dominant:

- incremental prepared-record maintenance for section changes,
- coalescing ready-publication updates,
- chunk-keyed draw-record arrays instead of map-heavy per-frame walks,
- draw batching or greedy meshing from `117`,
- multiview/encode sharing only after the normal path is measured cleanly.

### 7. Local edit coherence is not yet a first-class lane

Java's `playerChanged` dirty path is a strong signal: local edits are not merely
background streaming work. Our system has revision rejection and old-mesh
visibility, but the desired local-edit presentation policy is still spread
across `113`, runtime compile budgets, and upload behavior.

We should not blindly do synchronous rebuilds on Quest XR. But we should copy
the architectural distinction: local-player edits need their own priority and
acceptance policy, with explicit measurement.

### 8. The web/native transport difference is hidden but not resolved

The shared trait is good, but native currently moves owned requests through
`mpsc`, while the web direction is resident worker memory with
`SharedArrayBuffer` and atomics. This is acceptable only if the common contract
names the logical object being transferred. A `RenderCompileRegion` or resident
snapshot mirror contract would make both platforms converge semantically even if
their transport mechanics differ.

## Working Architecture Model

The terrain render bus should be viewed as this pipeline:

| Phase | Java owner | Native owner today | Main pressure |
| --- | --- | --- | --- |
| Dirty mark | `RenderChunk` / `LevelRenderer` / `ViewArea` | `RenderSectionDirtyState`, client chunk events | Number of dirty sections/chunks |
| Admission | `LevelRenderer.compileChunksUntil` | `sync_render_sections_with_budget...` | Frame deadline, chunk budget, capacity |
| Queue coordination | `ChunkRenderDispatcher` mailbox | render session plus compiler trait plus worker | Set churn, request build, channel send |
| Compile resource | `freeBuffers` buffer-pack pool | `pending_jobs` / worker count | Memory, worker throughput |
| Compile input | `RenderChunkRegion` | `Vec<ChunkSnapshot>` | Clone/move size, ownership, parity |
| Worker build | `RebuildTask.compile` | `build_render_sections_from_snapshots` | CPU compile cost, geometry count |
| Completion acceptance | upload futures then atomic compiled publish | result queue plus revision partition | Acceptance budget, stale rejection |
| Upload | `toUpload` plus `VertexBuffer.uploadLater` | XR/renderer section upload application | GPU resource update burst |
| Drawable publish | `CompiledChunk` atomic reference | ready sections, prepared records, traversal-ready state | cache invalidation, record churn |
| Draw/encode | `LevelRenderer` render lists | terrain renderer/XR scene | cull, encode, draw calls, GPU |

The critical design point: every row needs both a correctness invariant and a
budget/pressure counter. Otherwise the system can look bounded at one row while
silently doing unbounded work at the next.

## Refactor Direction

The target organization is a small set of shared owners, with platform glue
kept outside the policy:

1. **Resident terrain render state**: per-section or per-slot lifecycle records
   for dirty, deferred, inflight, completed, uploaded, and drawable state. This
   replaces repeated large set motion with local state transitions where
   possible.
2. **Terrain render coordinator**: the shared owner of dirty classification,
   readiness, admission, revision checks, and lifecycle counters.
3. **Terrain compile dispatcher**: the owner of priority, worker capacity,
   request submission, completed-result drain, and Java-style queue health.
4. **Compile-region boundary**: the shared input contract for worker compile,
   eventually closer to Java `RenderChunkRegion` than column snapshot bundles.
5. **Upload/publication boundary**: the owner of accepted mesh upload pressure
   and the transition from uploaded to drawable records.

The first extraction should be selected by measurement. The desired end state is
not one giant object; it is a clearer split where each stage has one owner and
one set of counters.

## Implementation Driver

### Slice A: split `submit_handoff_ms` as the refactor probe

Status: landed, pending Quest measurement.

Add timing and count instrumentation inside shared submit, keeping the current
behavior unchanged:

- request-build time,
- compiler submit time,
- mark-inflight time,
- apply-ready-plan time,
- ready-update/report time,
- ready section count,
- deferred section count,
- dirty section count before/after,
- inflight section count before/after,
- request snapshot count,
- request revision count.

Implementation notes:

- `RenderSectionSyncTiming` now carries the split fields.
- The timed native runtime path performs the same submit sequence as before:
  build request, submit to compiler, mark ready sections inflight, apply the
  ready plan, then build the ready update. Compiler-submit errors still return
  before inflight/ready mutation.
- Android XR logs the new max line as
  `MCLONE_ANDROID_XR_PERF_TERRAIN_SUBMIT_MAX`.
- A targeted snapshot unit test checks the new request snapshot/revision counts
  and inflight before/after counters.
- Follow-up instrumentation adds request count, worst single handoff, and worst
  single compiler-submit time so accumulated deadline-loop work can be separated
  from one bad submit.
- The second follow-up splits `RenderSectionCompileWorker::submit` itself into
  capacity check, command send, and render-thread pending-count mark, while
  logging request target/snapshot/light/revision counts and estimated owned
  payload bytes.

Measurement, Quest Android XR per-eye settled orbit, render distance 7, 2 render
compile workers, unbounded completed-result acceptance, 45-second sample:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Read |
|---|---|---|---|
| 14.211 / 15.849 / 24.616 / 63.979 ms | submit 13.078 ms; snapshot 0.859; handoff 12.735 | single handoff 12.699; request count 2; request build 0.060; compiler submit total 12.653; compiler submit single 12.653; command send total 12.651; command send single 12.651; capacity check 0.000; pending mark 0.001; mark inflight 0.017; apply ready plan 0.670; ready update 0.009 | 32 ready sections, 3552 deferred sections, 10 request snapshots, 72 snapshot sections, 102 light sections, 32 revisions, 587300 estimated payload bytes total, 306164 single |

Interpretation:

- The handoff tail is not ready-plan application or request construction in
  this run. Those are sub-millisecond even with thousands of deferred sections.
- The visible spike is inside native compiler submission. The request-count
  follow-up still shows this is not merely two medium submits added together:
  `request_count=2`, but `compiler_submit_single=12.653 ms`, effectively the
  whole compiler-submit total for the worst frame.
- The inner-submit follow-up narrows the current native worker tail to
  `std::sync::mpsc` command send: `command_send_single=12.651 ms`, while
  capacity check and render-thread pending-count mutation round to zero. The
  largest individual request in that max line targets 16 sections and carries
  about 306 KB of estimated owned payload.
- A direct bounded-std-channel attempt was not retained. Swapping the command
  channel to `mpsc::sync_channel(worker_count)` and using nonblocking `try_send`
  kept behavior otherwise unchanged, but the same lane worsened the measured
  command-send single tail to `15.361 ms` and frame avg / p95 / p99 / max to
  `14.237 / 15.949 / 25.078 / 48.903 ms`.
- This result changes the first extraction target from dirty/deferred set
  ownership to compiler dispatcher/request transport ownership. The set-moving
  model is still a likely architectural cleanup, but it is not the measured
  source of the current submit spike.
- Other tails remain comparable or larger on different frames: upload apply
  reached 12.661 ms, ready publish 11.527 ms, prepared-record rebuild 12.348 ms,
  and left-eye section encode 20.524 ms. The coordinator refactor still needs to
  cover upload/publication/drawable state, not only compiler handoff.

The target is the same Quest Android XR render-distance-7 settled orbit lane.
This should tell us whether the next implementation is data structure work,
transport work, or request-shape work. It also tells us which owner boundary to
extract first.

### Slice B: extract the first confirmed owner boundary

Choose based on Slice A:

- If `apply_ready_plan` dominates: extract resident lifecycle state and reduce
  dirty/deferred set churn instead of merely hiding it behind another cap.
- If request construction dominates: extract a narrower coordinator-owned
  request builder, avoid cloning `BTreeSet`s, and collect revisions from compact
  section state.
- If compiler submit dominates: extract dispatcher/request transport ownership
  and inspect channel send/move cost, worker locking, and request payload shape.
- If ready-update/report dominates: move diagnostics behind the coordinator as
  count-first telemetry and avoid large intermediate structures.

### Slice C: restore Java-region parity at the input boundary

Once handoff cost is understood, introduce a shared compile-region contract:

- target section bounds,
- one-block padded block-state access,
- neighbor chunk completeness/readiness metadata,
- block entity/light/tint hooks for future parity,
- native implementation backed by snapshots or resident mirrors,
- web implementation backed by worker-side shared memory.

This should make `127` an implementation plan rather than a loose idea.

### Slice D: define upload baseline and divergence rule

Measure a Java-like "drain all pending uploads" policy against an upload-budgeted
policy on Quest. Keep the Java baseline as the default mental model unless data
proves it is too bursty for XR. If we diverge, record the exact reason and the
phase budget that forced it.

### Slice E: attach draw-record maintenance to the same bus

If encode/prepared-record churn remains a tail after handoff and upload are
bounded, move prepared-record invalidation into the same section lifecycle so a
completed section causes a small, predictable drawable update instead of broad
record maintenance.

## Open Questions

1. Does `submit_handoff_ms` primarily come from `apply_ready_plan` and
   `BTreeSet` churn?
2. How often do deferred sections exceed thousands on stable render-distance-7
   movement, and how many are reprocessed per frame?
3. Does the worker submit cost scale with snapshot payload size, target-section
   count, or revision count?
4. Is upload apply dominated by section count, vertex/index bytes, GPU resource
   creation, or queue submission?
5. Does local edit coherence require a true synchronous path, or can a
   high-priority async path meet the visual target?
6. Can a compact current-view section index replace most tree-set operations
   without making chunk movement and removals fragile?
7. What is the minimum Java-region contract that supports current face culling
   plus future AO/light/tint/fluid parity?

## Success Criteria

This parent tactical is successful when:

- every terrain section has a clear lifecycle state from dirty to drawable,
- each lifecycle transition has a named owner and counter,
- the native ownership split is at least as easy to explain as Java's
  `ViewArea` / `RenderChunk` / `LevelRenderer` / `ChunkRenderDispatcher` /
  `RenderChunkRegion` split,
- no frame performs hidden work proportional to the full dirty/deferred set
  unless that work is explicitly budgeted,
- the baseline behavior is explainable as Java-like first and Quest-specific
  only where measurement requires divergence,
- native and web share the same compile-region semantics even if their worker
  transport differs,
- implementation tacticals `119`, `120`, and `127` can point here for the
  coordinating architecture instead of restating it from different angles.

## Current Recommended Next Step

Extract or replace the native terrain compile dispatcher/request transport
boundary. The measured next target is no longer `apply_ready_plan`; it is the
compiler handoff itself. The first follow-up should keep behavior the same while
making `RenderSectionCompileWorker::submit` more Java-dispatcher-like and less
dependent on moving an owned request through `std::sync::mpsc` on the render
frame. Good candidate slices:

- do not repeat a plain `std::sync::mpsc::sync_channel` swap; it was measured
  worse than the current unbounded channel in this lane,
- introduce a custom/preallocated dispatcher queue or request-slot ring with
  explicit render-thread ownership, queue-health counters, and nonblocking
  admission,
- move toward resident worker-owned snapshot/request storage if the custom
  dispatcher still pays too much to move owned request payloads from the render
  frame.
