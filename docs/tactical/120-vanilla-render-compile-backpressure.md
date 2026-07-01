# 120: Vanilla Render Compile Backpressure

Status: active. Shared native Rust renderer/runtime workstream.

## Purpose

Move the terrain-to-GPU pipeline closer to Minecraft Java 1.17.1's render chunk
compile pacing before adding more ad-hoc Android XR throttles.

Baseline policy: assume the Java client shape is correct unless profiling proves
otherwise. Match the reference queue, buffer-pack backpressure, deadline-driven
compile admission, upload publication, and local edit coherence first when
practical. Diverge only when a target lane has measured evidence that the
vanilla policy is the problem.

## Reference Shape

Primary files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`

Relevant Java behavior:

- `ChunkRenderDispatcher` owns dirty compile work, pending uploads, worker
  buffer packs, and a mailbox.
- Compile work starts only when a `ChunkBufferBuilderPack` is free. This is both
  worker backpressure and memory backpressure.
- `LevelRenderer.compileChunksUntil(...)` uses a frame deadline plus recent
  average compile cost to decide how many tasks to schedule.
- Completed chunk layers upload, then the compiled chunk swaps in atomically.
- Nearby player-dirty chunks can rebuild synchronously; treat that as the
  baseline local-edit coherence behavior unless profiling proves it is the wrong
  policy for a target lane.

## Current Native Gap

The shared render loop already has async render-section compilation, dirty
tracking, stale-result handling, and old-mesh retention. The admission policy is
still coarser than Java:

- `RenderSectionCompiler` only exposed pending job count, so callers inferred
  "no job in flight" as the only safe submission state.
- `mclone-app-runtime` uses a fixed dirty chunk budget of `1`.
- There is no explicit compile buffer-pack pool/capacity in the compiler
  contract.
- There is no frame-deadline admission equivalent to `compileChunksUntil(...)`.
- Upload/result acceptance and prepared-record maintenance still have separate
  burst paths tracked in `119`.

## Slice Plan

### A. Compiler Capacity Contract

Status: landed first pass.

Add explicit pending-job capacity to `RenderSectionCompiler` and make the shared
sync loop submit when capacity is available instead of hard-coding
`pending_job_count() == 0`.

This is intentionally conservative: current desktop/native and web compilers
still expose one slot, so production behavior stays the same. The contract now
matches the Java "free buffer pack" decision point and lets later native worker
pool work increase capacity without changing every caller.

Validation:

- Passed: `cargo test --manifest-path native/Cargo.toml -p mclone-render-session`

### B. Native Buffer-Pack Pool

Status: landed first conservative pass.

Replace the single native render compile worker slot with a bounded pool. This
does not need Java's exact `BufferBuilder` objects, but it should model the same
resource: a fixed number of compile work buffers/permits, worker slots, and
result handoff capacity.

Initial target:

- add a small native compile slot count behind a shared/runtime setting,
- keep default conservative until measured,
- expose pending jobs, max slots, and available slots in diagnostics,
- prove no unbounded request queue can grow behind the renderer.

First pass result:

- `RenderSectionCompileWorker` now owns a bounded native worker pool instead of a
  single hard-coded thread shape.
- The default remains one compile slot, so runtime behavior is intentionally
  unchanged until measurements justify a larger pool.
- `submit` now rejects work when all compile slots are occupied, which prevents
  a hidden unbounded request queue behind the renderer.
- Shared runtime accessors expose pending compile jobs, max compile slots, and
  available compile slots.

Remaining for this slice:

- add a runtime setting/launch option for the native slot count,
- surface slot counts in Android XR and desktop perf markers,
- benchmark one slot against larger slot counts before changing the default.

### C. Deadline-Driven Admission

Status: planned.

Add a Java-shaped admission controller:

- track recent submit/finish/apply cost,
- compare estimated next compile admission cost against remaining frame budget,
- stop scheduling when the next task likely exceeds the deadline,
- keep deterministic pump-to-idle paths able to override the live-frame
  deadline.

This should replace fixed "N chunks per frame" as the live policy once measured.

### D. Upload Publication Baseline

Status: planned after B/C.

Re-evaluate the upload path against Java's `uploadAllPendingUploads()` baseline.
Do not assume upload throttling is correct. First measure the vanilla-shaped
compile admission path; only keep upload/accept throttles when they beat the
baseline in headset and desktop lanes.

### E. Local Edit Coherence

Status: planned.

Reproduce Java's local edit coherence shape first, including synchronous or
high-priority near/player dirty behavior where practical. Any Quest-specific
bounded async alternative must be measured against that baseline and must avoid
partial visual holes.

## First Slice Result

Slice A adds `RenderSectionCompiler::max_pending_job_count()`,
`available_pending_job_slots()`, and `has_pending_job_capacity()`. The shared
`EngineRenderSession::sync_render_sections_with_budget(...)` now submits when
the compiler reports capacity rather than only when `pending_job_count() == 0`.

Current compilers keep the default capacity of one, so this does not change
runtime behavior yet. Tests prove the shared loop can admit another compile when
a compiler exposes spare capacity and waits when capacity is full.

## Next Step

Finish Slice B instrumentation: add a runtime setting for native compile slots
and include max/available slot counts in the perf capture. Then compare one
slot against two or more slots on the RD7 settled orbit and desktop streaming
probes before changing the default.
