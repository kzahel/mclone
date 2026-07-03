# 128: Terrain Render Pipeline Coordination

Status: active architecture parent; resident cached-section dirty flags and
ready-set upload-backpressure attribution landed, broader coordinator lifecycle
remains
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

## North Star And Anti-Drift Rules

North star: converge on a Java-shaped dirty-to-drawable terrain pipeline,
adapted to native Rust, `wgpu`, XR, and web workers:

1. resident render-section/render-column slots hold lifecycle state,
2. frame admission chooses bounded work before the deadline,
3. a dispatcher owns priority, worker capacity, request slots, and completed
   results,
4. compile workers consume a small compile-region contract,
5. upload/publication has its own queue, counters, and frame policy,
6. old drawable output remains visible until replacement output is complete.

The current confirmed problem is narrower than the whole pipeline: on Quest
Android XR render-distance-7 settled orbit, the render-thread spike is in the
compiler handoff, specifically `std::sync::mpsc` command send of an owned
compile request. A plain bounded `sync_channel` swap was measured worse. That
means the next implementation work should change the ownership shape, not keep
trying generic channel variants.

To avoid getting lost in local trees, every implementation slice under this
parent must answer these questions before it lands:

- Which Java-like owner does this slice clarify: resident slot, admission,
  dispatcher, compile-region input, upload/publication, or drawable records?
- Which measured metric should improve or become easier to attribute?
- Which stable lane proves it: currently Quest Android XR per-eye
  render-distance-7 settled orbit, 2 render compile workers, 45 seconds.
- What is the rollback rule if the metric is flat or worse?
- Does the slice preserve the shared native/web contract, or does it document a
  temporary platform-local exception?

Defer tempting side quests unless the metrics point there. Greedy meshing,
multiview, render scale, and draw-record maintenance are still relevant, but
they should not displace the confirmed dispatcher/request-transport work unless
the render-distance-7 lane stops showing submit/command-send pressure and starts
showing a different dominant tail.

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

- Cached resident render-section slots now carry a dirty flag alongside the
  mesh payload, so update-driven resident dirty marks can stay local to the
  slot.
- `RenderSectionDirtyState` tracks fallback dirty chunks/sections, inflight
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
concepts of dirty, deferred, inflight, completed, stale, removal, and ready,
and the first resident dirty-flag step is now in place. The weakness is not that
the concepts are missing. The weakness is that they are not yet gathered behind
a small number of durable owners, so reasoning about one section's path from
dirty to drawable still requires following several modules and several
transient set transformations.

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
movement frames can have thousands of deferred sections. After `133` Slice 3D,
resident cached-section dirty marks no longer insert into `dirty_sections`, and
`apply_ready_plan` clears accepted resident flags while leaving deferred
resident sections dirty in place. The remaining fallback dirty chunks/sections,
inflight sections, and deferred-ready materialization are still set-heavy. If
those sets are repeatedly rebuilt and reinserted, a small submitted job can
still pay a large coordination cost.

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

Status: landed and measured.

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

### Slice B: introduce the dispatcher boundary

Status: implemented, host/web validated, and Quest render-distance-7 measured.

The confirmed first owner boundary is a terrain compile dispatcher. This slice
should make the ownership visible before it tries to remove every request clone.
The first version may keep the current snapshot-backed request payload, but the
render frame should no longer talk directly to a flat worker channel.

Target shape:

- a shared dispatcher/coordinator API owns compile queue counters, pending job
  count, capacity, request admission, completed-result drain, and queue health,
- runtime frame code asks the dispatcher to admit ready section work before the
  deadline,
- native implementation wraps today's compile worker internally so behavior is
  preserved for the first extraction,
- Android XR logs the same `MCLONE_ANDROID_XR_PERF_TERRAIN_SUBMIT_MAX` fields
  plus dispatcher queue-health counters,
- web keeps the same high-level dispatcher contract even if its implementation
  remains a temporary inline or worker-backed path.

Success condition: the code now has an obvious owner corresponding to Java's
`ChunkRenderDispatcher`, and the render-distance-7 lane is no worse. A flat result is
acceptable for this slice if it removes the direct render-frame-to-worker
channel dependency and prepares the request-slot change.

Rollback rule: if the extraction adds new frame spikes or obscures existing
submit metrics, revert or shrink the API before continuing.

Implementation result:

- `mclone-render-session` now exposes `RenderSectionCompileDispatcher` and
  `RenderSectionCompileQueueHealth` as the shared dispatcher surface.
- `SingleViewRuntime` frame-admission paths are typed against the dispatcher
  boundary instead of the lower-level compiler trait.
- Native local and remote single-view runtimes now hold
  `NativeRenderSectionCompileDispatcher`, which wraps the existing
  `RenderSectionCompileWorker` internally.
- Android XR terrain submit logging now includes dispatcher health counters:
  pending jobs, max pending jobs, available slots, and queued compile tasks.
- Behavior is intentionally preserved: snapshot-backed request construction and
  the current native worker/channel remain inside the native dispatcher for this
  extraction.
- The first headset run showed the combined submit log line was too long for
  logcat and truncated before the dispatcher counters. Android XR now emits a
  separate `MCLONE_ANDROID_XR_PERF_TERRAIN_DISPATCHER_MAX` line.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client`
- `pnpm native:web:build`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, 45 seconds.

Measurement:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Dispatcher health |
|---|---|---|---|
| 15.794 / 18.482 / 27.032 / 57.255 ms | submit 15.569 ms; snapshot 0.943; handoff 15.378 | request count 2; compiler submit total 15.349; compiler submit single 15.349; command send total 15.347; command send single 15.347; apply ready plan 0.283; ready update 0.002 | pending jobs 2; max pending jobs 2; available slots 2; queued compile tasks 2 |

Interpretation:

- Slice B succeeded as an ownership extraction: the runtime now has an explicit
  dispatcher boundary and the new queue-health counters are visible.
- It did not improve performance. The confirmed tail is still the same one:
  command send of the owned compile request dominates compiler submit.
- The frame lane was worse than the earlier pre-dispatcher reference
  (`14.211 / 15.849 / 24.616 / 63.979 ms`), but this is not a clean isolated
  A/B because the current tree/render state differs and this run drew 158
  sections versus 128 in that earlier reference. Treat the result as "no
  performance win, proceed to the actual transport change" rather than proof
  that the boundary wrapper itself is the cause.

### Slice C: replace owned-request transport with dispatcher-owned slots

Status: implemented and Quest render-distance-7 measured.

Once Slice B owns the dispatcher boundary, replace the generic owned-request
handoff with explicit dispatcher-owned request slots or a preallocated request
ring.

Target shape:

- frame admission reserves a request slot from the dispatcher,
- request construction fills dispatcher-owned storage,
- worker wakeup carries a compact slot id or descriptor, not the full request,
- queue capacity is explicit and visible in counters,
- failed admission leaves dirty/ready state intact for a later frame.

Success condition: `command_send_single_ms` and `compiler_submit_single_ms`
drop materially in the render-distance-7 settled-orbit lane without increasing upload or
encode tails enough to lose the gain.

Rollback rule: if slot ownership becomes complex without reducing command-send
or submit tails, stop and move to resident compile inputs instead of adding
another queue layer.

Implementation result:

- Native render compile workers no longer receive
  `RenderSectionCompileRequest` through a `std::sync::mpsc` command channel.
- The native compile owner now has a fixed request-slot queue backed by
  `Mutex` + `Condvar`. Submit fills a dispatcher-owned slot and wakes a worker;
  workers take the slot request and then compile outside the queue lock.
- The existing result channel is unchanged.
- The existing Android XR `command_send` timing label is retained for continuity,
  but now measures slot enqueue plus worker wakeup, not `mpsc` send.
- Queue health now reports queued slot descriptors separately from pending jobs.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client`
- `pnpm native:web:build`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, 45 seconds.

Measurement:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Dispatcher health |
|---|---|---|---|
| 15.558 / 18.392 / 25.175 / 39.715 ms | submit 13.587 ms; snapshot 0.846; handoff 13.160 | request count 2; compiler submit total 13.107; compiler submit single 13.091; slot enqueue/wakeup total 13.105; slot enqueue/wakeup single 13.089; apply ready plan 0.508; ready update 0.004 | pending jobs 2; max pending jobs 2; available slots 2; queued compile tasks 1 |

Interpretation:

- Slice C changed the ownership shape as intended and modestly improved the
  measured submit tail versus the Slice B dispatcher-boundary run
  (`command_send_single_ms` from `15.347` to `13.089` in these two runs).
- It did not solve the problem. The remaining submit tail is still almost
  entirely inside the "command send" bucket, which now means slot enqueue and
  worker wakeup rather than `mpsc` payload transfer.
- The result strongly suggests the tail is not just moving the owned request
  object through `mpsc`. It may be render-thread preemption around wakeup,
  mutex/condvar scheduling behavior, or still some request/slot ownership cost
  hidden inside the coarse enqueue bucket.
- The next slice should split the slot enqueue path itself before another
  structural rewrite: lock wait, slot selection/write, queue push, notify, and
  post-notify timing. If those sub-buckets are tiny while the outer enqueue
  remains large, treat it as scheduler/preemption evidence and move to resident
  compile inputs or a worker-poll/mailbox model rather than more channel swaps.

Follow-up instrumentation result:

- Implemented July 2, 2026. `RenderSectionCompileSubmitTiming` now splits the
  slot enqueue/wakeup bucket into lock wait, slot selection, slot write, queue
  push, notify, and post-enqueue residual time. Android XR reports the split as
  `MCLONE_ANDROID_XR_PERF_TERRAIN_ENQUEUE_MAX`.
- Validation:
  - `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
  - `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client`
  - `pnpm native:web:build`
  - Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
    workers, 45 seconds.
- Measurement:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Enqueue split |
|---|---|---|---|
| 15.636 / 18.455 / 26.422 / 43.314 ms | submit 18.523 ms; snapshot 0.944; handoff 18.058 | request count 2; compiler submit total 17.998; compiler submit single 17.983; slot enqueue/wakeup total 17.996; slot enqueue/wakeup single 17.981 | lock wait total/single 0.001/0.001 ms; slot select 0.002/0.001; slot write 0.000/0.000; queue push 0.001/0.001; notify 0.498/0.488; post-enqueue 17.958/17.958 |

Interpretation:

- The queue data-structure work is not the tail. Lock wait, slot scan, slot
  write, and queue push are effectively zero at this scale.
- `notify_one` itself can cost around half a millisecond in the worst sample, but
  the frame-breaking tail is almost entirely the post-enqueue residual. In this
  code shape that residual is the time after the worker notification bucket and
  before the render frame regains control from `enqueue`: lock guard drop,
  function return overhead, and, most importantly, any scheduler/preemption
  caused by waking compile workers.
- This supports the dispatcher/pacing thesis: the render frame should not be on
  the hot side of worker wakeup behavior. Another generic queue replacement is
  unlikely to be the right next move unless it explicitly changes wakeup
  ownership.

Next implementation implication:

- First do the smallest wakeup hygiene experiment: release the queue mutex before
  `notify_one` and measure the same render-distance-7 lane. This is the
  idiomatic condvar shape and tests whether waking a worker while the render
  frame still owns the mutex is amplifying scheduler contention.
- If `post_enqueue_single_ms` remains multi-millisecond, move to a real wakeup
  ownership change: a dispatcher pump or worker-poll/mailbox shape where the
  render frame publishes bounded work and a non-render owner performs worker
  notification.

Wakeup-order experiment result:

- Implemented July 2, 2026. Native slot enqueue now releases the queue mutex
  before `notify_one`.
- Validation:
  - `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
  - `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client`
  - `pnpm native:web:build`
  - Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
    workers, 45 seconds.
- Measurement:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Enqueue split |
|---|---|---|---|
| 15.798 / 18.265 / 25.326 / 48.698 ms | submit 16.157 ms; snapshot 0.856; handoff 15.891 | request count 2; compiler submit total 15.808; compiler submit single 15.792; slot enqueue/wakeup total 15.806; slot enqueue/wakeup single 15.791 | lock wait total/single 0.006/0.006 ms; slot select 0.010/0.010; slot write 0.000/0.000; queue push 0.003/0.003; notify 15.804/15.790; post-enqueue 0.005/0.005 |

Interpretation:

- Releasing the mutex before `notify_one` fixed the previous attribution problem:
  the `post_enqueue_single_ms` residual dropped from `17.958 ms` to `0.005 ms`.
- The tail did not go away. It moved into `notify_one` itself
  (`notify_single_ms=15.790`). That means the expensive render-frame-side event
  is worker wakeup/scheduler behavior, not queue mutation, lock wait, slot
  storage, or mutex-drop ordering.
- The handoff tail improved modestly versus the previous instrumentation run
  (`command_send_single_ms` from `17.981` to `15.791`), but frame-level
  performance did not materially improve. Treat this patch as a correct
  low-risk hygiene change, not the final performance fix.

Next implementation implication:

- Stop iterating on the queue data structure itself. The remaining issue is that
  the render frame performs the worker wakeup.
- Move wakeup ownership off the render frame. The next useful slice should be a
  dispatcher pump or worker-poll/mailbox design where the render frame publishes
  bounded compile requests and another owner wakes or services compile workers.

Dispatcher-pump experiment result:

- Implemented July 2, 2026. Native compile submission now publishes filled
  request slots into a dispatcher-pending queue. A dedicated dispatcher pump
  thread polls that queue, moves slots into the worker-ready queue, and pays the
  worker `notify_one` cost off the render frame.
- This is intentionally the smallest Java-shaped ownership step, not a full
  `ChunkRenderDispatcher` port: it preserves bounded admission and the existing
  worker/result shape while changing who wakes compile workers.
- Validation:
  - `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
  - `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client`
  - `pnpm native:web:build`
  - Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
    workers, 45 seconds.
- Measurement:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Enqueue split |
|---|---|---|---|
| 15.924 / 18.634 / 26.912 / 55.557 ms | submit 12.928 ms; snapshot 9.676; handoff 12.500 | request count 3; compiler submit total 0.011; compiler submit single 0.010; slot enqueue/wakeup total 0.003; slot enqueue/wakeup single 0.003 | lock wait total/single 0.001/0.000 ms; slot select 0.001/0.001; slot write 0.001/0.001; queue push 0.001/0.000; notify 0.000/0.000; post-enqueue 0.002/0.002 |

Other tail evidence from the same run:

- `max_apply_ready_plan_ms=12.456`
- `max_runtime_gpu_upload_ms=20.444`
- `max_runtime_ready_sections_ms=18.992`
- `max_terrain_left_eye_encode_ms=39.845`
- `max_runtime_upload_apply_ms=20.418`
- `submitted_sections=48`, `accepted_results=3`, `uploaded_sections=15`

Interpretation:

- The dispatcher pump succeeds at the narrow target. Render-frame submit no
  longer pays worker wakeup: `command_send_single_ms` dropped from `15.791 ms`
  in the notify-after-unlock run to `0.003 ms`.
- Overall frame pacing did not materially improve because the burst moved
  downstream. This run accepted and uploaded more completed terrain work in the
  worst window, and the max terrain frame was dominated by ready/apply/upload and
  left-eye encode work rather than compile handoff.
- This is still a useful parity step: it makes the native ownership shape closer
  to Java's dispatcher/mailbox split and removes a confirmed render-thread
  wakeup hazard. It also clarifies that the next bottleneck is completed-result
  admission/publishing/upload/record maintenance, not compile request transport.

Next implementation implication:

- Keep the dispatcher pump shape.
- The next slice should pace completed-result acceptance and ready-section
  publication/upload. A Java-baseline measurement should first drain all pending
  ready/upload work as today; then compare a bounded policy that caps accepted
  completed results, uploaded sections/removals, and prepared-record rebuilds per
  frame.

Opt-in ready/upload pacing experiment result:

- Measured July 2, 2026 using the existing Android XR runtime flags:
  `--xr-render-completed-result-accept-budget 1`,
  `--xr-render-section-upload-budget 8`, and
  `--xr-render-section-accept-budget 16`.
- No code change was needed for this experiment. These flags already exercise
  the opt-in budgeted path while leaving the default Java-like drain-all behavior
  unchanged.
- Validation:
  - Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
    workers, 45 seconds.
  - The APK built and `validate-quest-openxr.sh` passed.
- Measurement:

| Frame avg / p95 / p99 / max | Runtime / upload split | Work accepted this frame | Backlog |
|---|---|---|---|
| 15.640 / 18.084 / 23.280 / 48.932 ms | sync 5.750 ms; submit 3.935; snapshot 3.738; handoff 0.570; upload apply 4.813; ready sections 21.583; ready publish 16.609 | accepted results 1; completed sections 16; uploaded sections 8; removed sections 14 | queued completed results 1; queued upload sections 288; queued upload removals 320 |

Other comparison points:

- Meta perf dropped frames improved from `68` in the unbounded dispatcher-pump
  run to `61`.
- The p99 improved from `26.912 ms` to `23.280 ms`, and `over_2x_budget`
  improved from `25` to `9`.
- Upload/apply burst improved sharply (`runtime_upload_apply_ms` from `20.418`
  to `4.813`), and submit/apply stayed low (`max_apply_ready_plan_ms=0.445`).
- The strict budgets created substantial backlog and shifted the visible tail to
  ready-set computation/publication and eye encode (`ready_publish_ms=16.609`,
  left-eye encode `28.259`, right-eye encode `12.683`).

Interpretation:

- The existing budget knobs are useful as diagnostics and should remain opt-in.
  They reduce the worst upload/apply spikes and improve p99, but the tested
  values are too strict to treat as a default because they accumulate hundreds of
  queued section uploads/removals in a 45 second render-distance-7 orbit.
- The bottleneck is now more clearly split: upload apply can be bounded, but
  ready-set recompute/publish and prepared draw-record churn still produce large
  frame tails when work is spread across many more frames.
- A second, less aggressive budget point was attempted
  (`completed-result=2`, `upload=16`, `accept=64`), but ADB lost the headset
  before launch. Restarting the ADB server did not rediscover the device during
  this session.

Next implementation implication:

- Do not make the strict `1 / 8 / 16` policy the default.
- When the headset is visible again, measure a less aggressive budget point:
  completed-result accept budget `2`, upload budget `16`, section accept budget
  `64`.
- The strict budget run also exposed an architectural mismatch: XR budgeted
  uploads queue on the render side, but runtime sync could still accept completed
  compile results and submit more section compiles before that queue caught up.
  Java's dispatcher shape is more coordinated: pending uploads/free buffer packs
  backpressure worker admission.

Coordinated upload-backpressure implementation:

- Implemented July 2, 2026 in `mclone-xr-scene` as the first bridge toward the
  Java shape.
- In opt-in budgeted upload mode only
  (`--xr-render-section-upload-budget` and/or
  `--xr-render-section-accept-budget`), `poll_runtime_and_upload` now drains one
  budgeted slice of already queued section uploads/removals before runtime
  section sync.
- If that upload queue still has work after the drain, the frame skips runtime
  section sync. That means it does not accept more completed compile results or
  submit more compile requests while the render-side upload queue is already
  behind.
- The default unbudgeted path is unchanged and still applies section updates in
  the drain-all style unless measured XR evidence justifies a different default.
- Perf counters now preserve real pending compile job and queued completed-result
  counts on upload-backpressured frames instead of reporting the default empty
  cache-update values.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `git diff --check -- native/crates/mclone-xr-scene/src/lib.rs`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene` (`53`
  tests)

Quest measurement:

- Measured July 2, 2026 on Quest 3 using the render-distance-7 settled orbit
  lane, 2 render compile workers, 45 second sample, per-eye path, render scale
  `1.000`.
- Commands:
  - strict: `--xr-render-completed-result-accept-budget 1`,
    `--xr-render-section-upload-budget 8`,
    `--xr-render-section-accept-budget 16`
  - less aggressive: `--xr-render-completed-result-accept-budget 2`,
    `--xr-render-section-upload-budget 16`,
    `--xr-render-section-accept-budget 64`

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Upload backlog max | Key tail |
|---|---:|---|---:|---|---|
| `1 / 8 / 16` coordinated bridge | 72 | 15.626 / 18.172 / 23.855 / 50.325 ms | 77.2% | queued uploads 8; queued removals 208; queued completed results 1 | upload apply 20.588 ms; ready sections 21.980 ms; left/right eye 28.396/31.635 ms |
| `2 / 16 / 64` coordinated bridge | 68 | 15.830 / 18.475 / 25.732 / 55.895 ms | 84.6% | queued uploads 16; queued removals 160; queued completed results 0 | upload select 22.347 ms; upload apply 15.560 ms; ready publish 32.058 ms |

Comparison against the earlier strict budget run before the bridge:

- The bridge did what it was designed to do mechanically: max queued upload
  sections dropped from `288` to `8` on the strict point. Queued removals also
  improved from `320` to `208`.
- It did not improve frame pacing. Meta dropped frames were worse than the prior
  strict run (`61` before, `72` after), and the less aggressive point remained
  worse on p99/max and app-over-period frames.
- The likely reason is that the bridge prevents unlimited render-side backlog,
  but it still performs large lifecycle maintenance on the render frame. The
  tail moved into budgeted upload selection/apply, ready-set recomputation or
  publication, and eye encode/wait.

Interpretation:

- Keep this as an opt-in diagnostic bridge for now. Do not make these budgeted
  policies default.
- The result strengthens the Java-shape conclusion: a frame-local upload queue
  bridge is not enough. The capacity/backpressure owner needs to move upstream
  into the shared dispatcher/coordinator so the system reserves capacity before
  compile starts and releases it after upload/publication, instead of producing
  completed work first and asking the render frame to absorb it later.
- The `2 / 16 / 64` run also suggests the current budgeted removal-selection path
  needs scrutiny before more budget tuning. `runtime_upload_select_ms=22.347`
  is far too high for a pacing mechanism.

Next implementation implication:

- Move the capacity token upstream: make the shared dispatcher own the
  drawable/upload capacity and hold compile slots until results are
  uploaded/published, closer to Java's buffer-pack lifetime.
- In that design, avoid the current frame-side backlog queue becoming the
  pacing owner. The render frame should consume a bounded number of already
  admitted upload/publication transitions, not decide after the fact whether too
  much work was produced.
- Also inspect the current budgeted upload/removal selection path. Even if the
  dispatcher shape is corrected, a pacing mechanism that can spend 22 ms just
  selecting budgeted removals is not viable.

Dispatcher-held capacity implementation:

- Implemented July 2, 2026 as the next Java-shaped ownership step.
- `RenderSectionCompiler` now exposes `release_completed_jobs(count)`.
- Native render compile capacity is no longer released when the worker result is
  received from the result channel. The native dispatcher keeps the job counted
  as pending until a caller explicitly releases it.
- XR terrain queues compile-release batches beside budgeted section
  uploads/removals. Accepted compile results release their compile capacity only
  after their queued drawable lifecycle work is applied or superseded.
- Pump-to-idle and startup paths that do not have a live GPU upload boundary
  release accepted results internally to preserve their existing behavior.
- Flat desktop and flat Android release accepted compile jobs after successful
  section upload so the shared runtime does not become XR-only.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `git diff --check -- native/crates/mclone-render-session/src/lib.rs native/crates/mclone-app-runtime/src/lib.rs native/crates/mclone-app-runtime/src/local_single_view.rs native/crates/mclone-app-runtime/src/render_assets.rs native/crates/mclone-xr-scene/src/lib.rs native/apps/mclone-native-client/src/flat_client_driver.rs native/apps/mclone-native-client/src/perf.rs native/apps/mclone-native-client/src/scene_runtime.rs native/apps/mclone-android-client/src/lib.rs`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client`

Quest measurement:

- Measured July 2, 2026 on Quest 3 using the render-distance-7 settled orbit
  lane, 2 render compile workers, 45 second sample, per-eye path, render scale
  `1.000`.

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Upload backlog max | Key tail |
|---|---:|---|---:|---|---|
| `1 / 8 / 16` bridge before held capacity | 72 | 15.626 / 18.172 / 23.855 / 50.325 ms | 77.2% | queued uploads 8; queued removals 208; queued completed results 1 | upload apply 20.588 ms; ready sections 21.980 ms |
| `1 / 8 / 16` held capacity | 80 | 15.851 / 18.145 / 21.549 / 50.590 ms | 81.9% | queued uploads 8; queued removals 208; queued completed results 1 | upload apply 16.885 ms; submit snapshot 11.764 ms; ready publish 9.218 ms |
| `2 / 16 / 64` bridge before held capacity | 68 | 15.830 / 18.475 / 25.732 / 55.895 ms | 84.6% | queued uploads 16; queued removals 160; queued completed results 0 | upload select 22.347 ms; upload apply 15.560 ms; ready publish 32.058 ms |
| `2 / 16 / 64` held capacity | 62 | 15.767 / 18.296 / 24.512 / 51.107 ms | 82.7% | queued uploads 16; queued removals 160; queued completed results 0 | submit handoff 12.900 ms; upload apply 11.355 ms; ready publish 17.849 ms |

Interpretation:

- The held-capacity model is worth keeping. The less aggressive `2 / 16 / 64`
  point improved dropped frames, p99, max frame time, app-over-period rate, and
  upload/ready tails compared with the bridge-only run.
- It is not a complete pacing solution. The strict `1 / 8 / 16` point improved
  p99 and upload apply but increased dropped frames, so budget size still
  matters and the smaller policy should not become default.
- The 22 ms upload-selection spike did not reproduce after held capacity
  (`0.018 ms` in the `2 / 16 / 64` run). Keep the concern open, but the stronger
  next target is now phase coordination rather than a standalone selection-path
  fix.
- The remaining shape is still too XR-local: compile capacity is held until
  upload, but the release batches live beside XR's frame-side upload queue. Java
  centralizes this in `ChunkRenderDispatcher` through buffer-pack/upload-future
  lifetime. We should move the release/admission policy into a shared terrain
  coordinator rather than accreting more frame-loop bookkeeping.

Next implementation implication:

- Promote held compile capacity from an XR-local release-batch bridge into the
  shared dispatcher/coordinator. The coordinator should own the dirty/ready,
  compiling, completed, uploading, and drawable transitions and expose a small
  per-frame drain/admit API.
- Avoid releasing capacity and immediately submitting expensive new compile
  work in a way that can recreate `runtime_submit_*` tails on the same render
  frame. Java drains uploads first, then admits compile work under its frame
  deadline; the native coordinator should make that phase boundary explicit.
- Add release/backlog counters: held completed jobs, pending upload lifecycle
  items, released jobs this frame, and compile admissions after release. Current
  counters still make it hard to tell whether a frame was upload-limited,
  capacity-limited, or submit-limited.

Shared upload-coordinator extraction:

- Implemented July 2, 2026 as the first promotion step out of XR-local
  bookkeeping.
- `mclone-render-session` now owns `RenderSectionUploadCoordinator`, a shared
  pending upload/removal queue plus compile-release lifecycle tracker.
- XR terrain no longer owns its own pending section uploads, pending removals,
  or compile-release batches. It asks the shared coordinator to enqueue cache
  updates, drain a budgeted slice, and release compile capacity after applied or
  superseded lifecycle work.
- Behavior is intentionally preserved. This slice changes ownership and test
  coverage, not the budget policy or the default unbounded path.
- The coordinator has focused unit coverage for budgeted removal drains,
  batch-held release behavior, and superseded removal-to-rebuild release.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `git diff --check -- native/crates/mclone-render-session/src/lib.rs native/crates/mclone-xr-scene/src/lib.rs`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session upload_coordinator`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client -p mclone-web-client`
- `pnpm native:web:build`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, `2 / 16 / 64`, 45 seconds.

Measurement:

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Upload backlog max | Key tail |
|---|---:|---|---:|---|---|
| `2 / 16 / 64` held capacity before shared coordinator | 62 | 15.767 / 18.296 / 24.512 / 51.107 ms | 82.7% | queued uploads 16; queued removals 160; queued completed results 0 | submit handoff 12.900 ms; upload apply 11.355 ms; ready publish 17.849 ms |
| `2 / 16 / 64` shared upload coordinator | 64 | 15.813 / 18.174 / 21.490 / 52.185 ms | 81.7% | queued uploads 16; queued removals 160; queued completed results 0 | submit snapshot 12.748 ms; upload apply 11.853 ms; ready publish 10.410 ms |

Interpretation:

- This is effectively performance-neutral, which is the desired outcome for an
  ownership extraction. Dropped frames moved from `62` to `64`, p99 improved
  from `24.512 ms` to `21.490 ms`, max moved from `51.107 ms` to `52.185 ms`,
  and app-over-period improved slightly from `82.7%` to `81.7%`.
- The shared coordinator did not itself solve pacing. That is expected: it is
  still only the upload/removal/release queue, not the full dirty-to-drawable
  coordinator.
- The useful result is architectural: the release-after-upload invariant is no
  longer encoded in XR-only frame-loop fields. That makes the next step a shared
  phase API instead of another XR-local cap.

Next implementation implication:

- Keep the shared upload coordinator.
- Expand it into a real shared terrain frame phase: drain pending
  upload/publication work first, release capacity from applied/superseded
  lifecycle items, then admit new compile work under the same coordinator
  counters.
- Add explicit release/backlog diagnostics to the shared API before changing the
  budget policy again: held completed jobs, pending upload lifecycle items,
  released jobs this frame, upload-limited frames, capacity-limited frames, and
  compile admissions after release.

Shared upload-phase diagnostics:

- Implemented July 2, 2026 without changing upload, release, or admission
  policy.
- `RenderSectionUploadCoordinator` now reports a shared
  `RenderSectionUploadPhaseReport` for enqueue, drain, and release work:
  accepted compile jobs, queued lifecycle items, superseded lifecycle items,
  drained lifecycle items, released compile jobs, queue backlog before/after,
  held release lifecycle items, held compile jobs, and whether upload or accept
  budgets limited the drain.
- XR terrain carries that phase report through `XrTerrainUploadSummary`.
- Android XR logs the maxima as
  `MCLONE_ANDROID_XR_PERF_UPLOAD_PHASE_MAX`.
- `android-xr/validate-quest-openxr.sh` now validates and includes that marker
  in perf summaries.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session upload_coordinator`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client -p mclone-web-client`
- `pnpm native:web:build`
- `bash -n android-xr/validate-quest-openxr.sh`
- `git diff --check -- android-xr/validate-quest-openxr.sh native/apps/mclone-android-xr-client/src/lib.rs native/crates/mclone-render-session/src/lib.rs native/crates/mclone-xr-scene/src/lib.rs`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, `2 / 16 / 64`, 45 seconds.

Measurement:

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Upload phase max | Key tail |
|---|---:|---|---:|---|---|
| `2 / 16 / 64` shared upload coordinator | 64 | 15.813 / 18.174 / 21.490 / 52.185 ms | 81.7% | not yet logged | submit snapshot 12.748 ms; upload apply 11.853 ms; ready publish 10.410 ms |
| `2 / 16 / 64` upload phase diagnostics | 75 | 15.623 / 17.960 / 22.534 / 52.100 ms | 79.0% | phase events 4; enqueued lifecycle 224; drained lifecycle 80; queued lifecycle 160; held lifecycle 128; held jobs 2; upload limited true; accept limited true; backpressured true | submit snapshot 11.037 ms; upload apply 3.728 ms; ready sections 24.015 ms; right-eye encode 29.623 ms |

Interpretation:

- The code change is diagnostic and should not be read as a performance win.
  Dropped frames worsened in this single run (`64` to `75`), while average,
  p95, app-over-period, and max frame time were similar or slightly better.
  Treat this as within current run-to-run variability unless repeated.
- The new phase line confirms the less aggressive budget lane still hits both
  upload and accept limits. The max aggregator is per-field, not a consistent
  single-frame snapshot, but it shows the important pressure shape: pending
  upload lifecycle peaked at `160`, held release lifecycle at `128`, and held
  compile jobs at `2`.
- The run also shows upload apply can be low (`3.728 ms`) while ready-section
  computation/publication and eye encode still produce large tails. That means
  the next policy work should coordinate upload/release/admission with ready
  publication, not only throttle GPU upload.

Next implementation implication:

- Use the new phase counters as the guardrail for the next policy change.
- The next slice should make the shared coordinator return a frame decision:
  drained/released work, backlog/held capacity after the drain, and whether
  compile admission should be skipped because upload/publication is still
  backlogged.
- Keep `2 / 16 / 64` as the measurement lane. Do not tune the numbers again
  until the shared phase can explain whether the frame was upload-limited,
  accept-limited, capacity-limited, or ready/encode-limited.

Shared frame-decision API:

- Implemented July 2, 2026 as a behavior-preserving ownership move.
- `mclone-render-session` now exposes `RenderSectionUploadFramePolicy`,
  `RenderSectionUploadFrameDecision`, and `RenderSectionUploadFrameLimit`.
- XR no longer computes the pre-sync upload-backpressure rule locally. It asks
  the shared coordinator whether to pre-drain pending upload work, whether
  runtime section sync may run, and whether a post-sync section update should be
  applied.
- The policy remains unchanged: budgeted mode drains one pending upload slice
  before runtime sync; if upload/removal lifecycle work remains queued, runtime
  sync is skipped for that frame.
- Empty enqueue/drain calls no longer inflate upload phase events.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session upload_`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client -p mclone-web-client`
- `pnpm native:web:build`
- `bash -n android-xr/validate-quest-openxr.sh`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, `2 / 16 / 64`, 45 seconds.

Measurement:

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Upload phase max | Key tail |
|---|---:|---|---:|---|---|
| `2 / 16 / 64` upload phase diagnostics | 75 | 15.623 / 17.960 / 22.534 / 52.100 ms | 79.0% | phase events 4; queued lifecycle 160; held lifecycle 128; held jobs 2; upload limited true; accept limited true; backpressured true | submit snapshot 11.037 ms; upload apply 3.728 ms; ready sections 24.015 ms; right-eye encode 29.623 ms |
| `2 / 16 / 64` shared frame decision | 64 | 15.735 / 18.248 / 23.384 / 52.201 ms | 80.5% | phase events 3; queued lifecycle 160; held lifecycle 64; held jobs 2; upload limited true; accept limited true; backpressured true | submit snapshot 32.169 ms; upload apply 19.191 ms; ready sections 25.926 ms; left-eye encode 24.578 ms |

Interpretation:

- This slice is not intended to improve performance; it centralizes the frame
  decision so the next policy change can be made in shared code. The result is
  in the same noisy band as the previous diagnostic run.
- The final run still shows both upload and accept limits tripping, queued
  lifecycle peaking at `160`, and held compile jobs at `2`.
- The bad tails are now visibly outside the boolean decision itself:
  ready-section computation/publication, upload apply on some runs, submit
  snapshot on this run, and eye encode/poll waits. The next implementation
  should use the shared decision as the place to coordinate those phases rather
  than adding another XR-local condition.

Next implementation implication:

- Keep the shared frame-decision API.
- Move the next phase boundary into the same shared model: publish/refresh
  traversal-ready sections only when the shared decision says the frame should
  do terrain state work, or add explicit counters proving why ready publication
  must still run on upload-backpressured frames.
- Treat ready-section recomputation/publication as a first-class bus phase,
  because it is repeatedly showing tails in the RD7 lane.

Traversal-ready backpressure attribution:

- Implemented July 3, 2026 without changing rendering behavior.
- `TexturedSectionRecordCacheStats` now splits traversal-ready publish calls by
  upload-backpressured context: total, changed, unchanged, backpressured,
  backpressured-changed, and backpressured-unchanged.
- XR passes the shared `RenderSectionUploadFrameDecision.upload_backpressured`
  flag into traversal-ready publication. The existing no-work early path records
  a non-backpressured ready publish.
- Android XR logs the counters in
  `MCLONE_ANDROID_XR_PERF_RECORD_CACHE`.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render record_cache_stats_track_upload_backpressured_ready_set_context -- --nocapture`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene -- --nocapture`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
- `cargo test --manifest-path native/Cargo.toml`
- Quest Android XR per-eye render-distance-7 settled orbit, 2 render compile
  workers, `2 / 16 / 64`, 45 seconds.

Measurement:

| Policy | Meta dropped frames | Frame avg / p95 / p99 / max | App over period | Ready-set calls | Upload phase max | Key tail |
|---|---:|---|---:|---|---|---|
| `2 / 16 / 64` shared frame decision | 64 | 15.735 / 18.248 / 23.384 / 52.201 ms | 80.5% | not yet split by backpressure | phase events 3; queued lifecycle 160; held lifecycle 64; held jobs 2; upload limited true; accept limited true; backpressured true | submit snapshot 32.169 ms; upload apply 19.191 ms; ready sections 25.926 ms; left-eye encode 24.578 ms |
| `2 / 16 / 64` ready-set attribution | 16 | 15.147 / 17.714 / 19.393 / 28.131 ms | 70.7% | calls 2961; changed 10; unchanged 2951; backpressured 0; backpressured changed 0; backpressured unchanged 0 | phase events 2; queued lifecycle 16; held lifecycle 16; held jobs 2; upload limited true; accept limited false; backpressured false | poll 7.284 ms; upload apply 7.418 ms; ready publish 4.328 ms; left/right eye encode 3.475/3.851 ms; stereo poll wait 9.723 ms |

Interpretation:

- The attribution counters landed and are visible in the Quest perf line. This
  slice is instrumentation-only and should not be read as the cause of the
  better July 3 result; the resident dirty-flag change from `133` is now also in
  the lane, and run-to-run variability remains significant.
- The current run did not exercise the precise upload-backpressured ready-publish
  path: `ready_set_backpressured=0`, and the upload phase max also reported
  `backpressured=false`.
- The ready set itself was almost always unchanged (`2951 / 2961` calls), so the
  expensive work to remove next is broad per-frame ready publication, not a
  blind upload-backpressured skip alone.
- The ready publish max dropped to `4.328 ms` in this run, so the highest current
  tails are no longer exclusively ready publication. Still, a versioned
  ready-publish phase is the right coordinator step because it removes hidden
  full-set work from every frame and gives later policy work an explicit input
  change key.

Next implementation implication:

- Add a shared, versioned traversal-ready phase instead of an
  upload-backpressure-only skip. It should publish only when one of the inputs
  changed: visible draw resources, render-neighbor readiness, or the camera/view
  state that affects the near-camera missing-neighbor exception.
- Keep the current counters as the guardrail. A correct fast path should drive
  unchanged ready publishes down without hiding changed publishes during camera
  movement or upload/removal application.
- Deferred note for closeout: force or construct a run that actually reports
  `ready_set_backpressured > 0`. The July 3 RD7 orbit did not hit that condition,
  so it cannot decide whether a narrower upload-backpressured skip is useful.

### Slice D: restore Java-region parity at the input boundary

Once handoff cost is understood, introduce a shared compile-region contract:

- target section bounds,
- one-block padded block-state access,
- neighbor chunk completeness/readiness metadata,
- block entity/light/tint hooks for future parity,
- native implementation backed by snapshots or resident mirrors,
- web implementation backed by worker-side shared memory.

This should make `127` an implementation plan rather than a loose idea.

### Slice E: define upload baseline and divergence rule

Measure a Java-like "drain all pending uploads" policy against an upload-budgeted
policy on Quest. Keep the Java baseline as the default mental model unless data
proves it is too bursty for XR. If we diverge, record the exact reason and the
phase budget that forced it.

### Slice F: attach draw-record maintenance to the same bus

If encode/prepared-record churn remains a tail after handoff and upload are
bounded, move prepared-record invalidation into the same section lifecycle so a
completed section causes a small, predictable drawable update instead of broad
record maintenance.

## Open Questions

1. What is the smallest completed-result admission and upload pacing policy that
   reduces ready/upload/encode bursts without falling behind chunk movement?
2. How often do deferred sections exceed thousands on stable render-distance-7
   movement, and how many are reprocessed per frame?
3. Does completed-result publish cost scale with accepted section count, removed
   section count, prepared-record rebuild count, or GPU upload bytes?
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

Use the new attribution counters to implement a shared, versioned
traversal-ready publication phase. The July 3 RD7 orbit showed `2951 / 2961`
ready publishes were unchanged, but it did not hit the upload-backpressured path
at all. That means the next safe implementation chunk is not a blind
upload-backpressure skip; it is a keyed fast path that avoids recompute/publish
when visible draw resources, render-neighbor readiness, and the camera/view
near-neighbor exception inputs are unchanged.

Use `2 / 16 / 64` as the current measurement lane, not a default policy. It was
the best held-capacity result, but it is still over the 72 Hz app budget most of
the time. The next run should tell us whether shared phase ownership reduces
submit/ready/upload tails and clarifies the limiting phase, not whether one more
isolated budget number looks better.
