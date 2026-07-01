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
  "no job in flight" as the only safe submission state. Slice A fixed this
  contract gap.
- `mclone-app-runtime` used a fixed dirty chunk budget of `1`. Slice C added a
  first Java-shaped deadline loop for measured native lanes, but the default
  live window path still needs a clear target-budget signal.
- There was no explicit compile buffer-pack pool/capacity in the compiler
  contract. Slice B added this for native workers with a conservative default.
- There was no frame-deadline admission equivalent to `compileChunksUntil(...)`.
  Slice C added the first native runtime version, but it does not yet use a
  cross-frame cost estimate before the first request.
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

Status: landed with runtime toggle and initial measurement.

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

Runtime setting and diagnostics:

- `--render-compile-workers N` / `renderCompileWorkers=N` selects the native
  compile slot count for desktop, flat Android, Android XR, and shared
  single-view runtime startup paths.
- The default remains one slot.
- Startup rejects zero workers.
- Desktop frame-budget/movement captures and Android XR perf markers now record
  the selected worker count plus pending/max/available compile slot state.

Initial measurement, render distance 7:

| Lane | Workers | Frame/app result | Compile/upload result | Read |
|---|---:|---|---|---|
| Desktop release frame-budget, 240-frame settled orbit | 1 | avg 2.798 ms, p95 4.519 ms, p99 5.582 ms, 1 over-budget frame | 772 submitted, 756 completed, 259 uploaded; slot saturated 240/240 frames | baseline |
| Desktop release frame-budget, 240-frame settled orbit | 2 | avg 3.109 ms, p95 4.918 ms, p99 6.374 ms, 1 over-budget frame | 1384 submitted, 1334 completed, 451 uploaded; slot saturated 231/240 frames | nearly doubles throughput but costs frame time |
| Quest Android XR per-eye, 30-second settled orbit | 1 | frame avg 15.381 ms, p95 24.181 ms, app-work avg 14.994 ms, app-over-period 53.9%, metrics dropped_frames 201 | max 16 submitted, 16 completed, 8 uploaded; max runtime upload 30.413 ms | compile limited and over budget |
| Quest Android XR per-eye, 30-second settled orbit | 2 | frame avg 14.954 ms, p95 23.025 ms, app-work avg 14.035 ms, app-over-period 27.0%, metrics dropped_frames 110 | max 16 submitted, 32 completed, 14 uploaded; max runtime upload 53.398 ms | better average/headroom, worse upload/sync tail |

Interpretation:

- The bounded pool is useful and should stay: it gives us a vanilla-shaped
  buffer-pack control point and exposes the throughput/latency tradeoff.
- Two workers are not a default yet. Desktop gets more terrain throughput but
  slightly worse frame metrics, and Quest improves average/headroom while
  producing larger tail bursts in runtime sync/GPU upload.
- The next policy decision should be deadline-driven admission, not a fixed
  global default increase. Worker count is now an opt-in variable for measured
  lanes.

### C. Deadline-Driven Admission

Status: first native pass landed and measured.

Add a Java-shaped admission controller:

- track submit/admission cost,
- compare estimated next compile admission cost against remaining frame budget,
- stop scheduling when the next task likely exceeds the deadline,
- keep deterministic pump-to-idle paths able to override the live-frame
  deadline.

First pass result:

- `SingleViewRuntime::sync_render_sections_until_deadline(...)` repeatedly runs
  the existing one-chunk shared sync/admission step while compiler capacity is
  available and the per-frame average admission cost still fits the remaining
  frame budget.
- This deliberately keeps deterministic `sync_all_render_sections(...)`
  unchanged for startup, captures, smoke tests, and pump-to-idle paths.
- Desktop frame-budget probes use the probe `--target-hz` as the deadline.
- Android XR uses the terrain state's display refresh as the terrain-runtime
  deadline for per-eye, full-frame multiview, and overlap-prefetch runtime sync.
- Perf markers now record `deadline_skipped_compile_requests`.

Measurement after Slice C, render distance 7:

| Lane | Workers | Frame/app result | Compile/admission result | Read |
|---|---:|---|---|---|
| Desktop release frame-budget, 240-frame settled orbit | 1 | avg 2.487 ms, p95 3.960 ms, p99 4.958 ms, 1 over-budget frame | 724 submitted, 708 completed, 242 uploaded, 0 deadline skips | desktop has enough headroom; gate does not fire |
| Desktop release frame-budget, 240-frame settled orbit | 2 | avg 2.798 ms, p95 4.708 ms, p99 5.296 ms, 1 over-budget frame | 1300 submitted, 1250 completed, 420 uploaded, 0 deadline skips | still throughput-heavy but within desktop budget |
| Quest Android XR per-eye, 30-second settled orbit | 2 | frame avg 15.130 ms, p95 25.104 ms, p99 39.982 ms, app-over-period 35.5%, metrics dropped_frames 75 | max 32 submitted, 32 completed, 1 deadline skip; max runtime sync 41.167 ms, max GPU upload 20.285 ms | gate is observable, but tails remain completion/upload dominated |

Interpretation:

- This is useful scaffolding, not a complete pacing fix. It moves native runtime
  admission toward the Java `compileChunksUntil(...)` shape and proves the
  deadline can gate extra worker admission.
- The first-pass rule is still Java-like within a frame: it needs at least one
  admission before it has a per-frame average. That means it cannot prevent a
  too-expensive first live request.
- Quest still shows large runtime sync/GPU-upload tails, so the remaining burst
  is mostly result acceptance/upload work rather than simply scheduling too many
  compile requests.
- Do not change the default worker count yet.

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

Move to Slice D, but keep it baseline-first: measure and shape result
acceptance/upload publication against Java's `uploadAllPendingUploads()` path.
The immediate candidate is not another worker cap; it is separating "compile
request admission" from "how many completed sections we accept/upload into this
frame" so Quest can avoid 40-50 ms runtime-sync/upload spikes without throwing
away the vanilla queue/buffer-pack structure.
