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
  burst paths tracked in `119`. Slice D added an opt-in completed-result
  acceptance budget, but it is not a default policy yet.

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

### D. Completed Result Acceptance / Upload Publication

Status: first opt-in result-acceptance pass landed and measured.

Re-evaluate the upload path against Java's `uploadAllPendingUploads()` baseline.
Do not assume upload throttling is correct. First measure the vanilla-shaped
compile admission path; only keep upload/accept throttles when they beat the
baseline in headset and desktop lanes.

First pass result:

- `EngineRenderSession` now owns a FIFO queue of completed compiler results.
- The default remains unbounded, preserving the previous baseline and the
  Java-shaped "drain completed work" behavior unless a caller opts in.
- `SingleViewRuntime` and `NativeSingleViewSessionRuntime` expose
  `completed_result_accept_budget`.
- Android XR exposes the probe as
  `--xr-render-completed-result-accept-budget N` and
  `MCLONE_ANDROID_XR_RENDER_COMPLETED_RESULT_ACCEPT_BUDGET`.
- Perf markers now distinguish completed-result acceptance from section upload:
  `accepted_results`, `queued_completed_results`, completed sections, uploaded
  sections, and queued upload sections.
- If completed results remain queued after the frame's result-acceptance budget,
  the shared sync loop does not submit more compile work in that call. This is
  intentional backpressure: the renderer should not create more completed
  results while the main/render side is already behind accepting them.

Measurement after Slice D, Quest Android XR per-eye settled orbit, render
distance 7, 2 render compile workers, 30-second sample:

| Result accept budget | Frame avg / p95 / p99 / max | Over-budget frames | Dropped frames | App over-period | Max runtime sync | Max GPU upload | Result/upload read |
|---:|---|---:|---:|---:|---:|---:|---|
| unbounded | 15.142 / 24.720 / 36.125 / 84.695 ms | 977 / 1978, 49.4% | 94 | 31.9% | 39.969 ms | 14.001 ms | max 2 accepted results, 32 completed sections, 13 uploaded |
| 1 | 15.073 / 24.514 / 34.695 / 75.843 ms | 974 / 1985, 49.1% | 88 | 28.4% | 56.490 ms | 5.431 ms | max 1 accepted result, 1 queued result, 16 completed sections, 8 uploaded |
| 2 | 15.075 / 24.218 / 34.110 / 100.366 ms | 995 / 1982, 50.2% | 85 | 30.2% | 81.842 ms | 10.092 ms | max 2 accepted results, 0 queued results, 32 completed sections, 11 uploaded |

Interpretation:

- The control point works and should stay as an opt-in diagnostic/policy hook.
  The budget-1 run capped result acceptance exactly as intended and cut the max
  GPU-upload burst from 14.001 ms to 5.431 ms in the current-code control.
- It is not a default performance win yet. Budget 1 improved current-control
  p99, max frame, app-over-period, and dropped-frame count slightly, but max
  runtime sync rose from 39.969 ms to 56.490 ms. Budget 2 was worse on max frame
  and max runtime sync.
- The remaining burst is not just raw GPU upload. After upload is smoothed, the
  high tail moves into runtime sync / ready-section maintenance, so the next
  measurement must split result acceptance, ready-set recomputation, and record
  cache/update publication more finely.
- Do not enable a result-acceptance budget by default. Keep default unbounded
  until an adaptive policy or follow-up split beats the baseline consistently.

Follow-up attribution pass:

- Added `MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME` to split terrain runtime work
  into poll, completed-result acceptance, dirty-cache seed, ready/dirty prepare,
  compile submit/snapshot handoff, GPU upload apply, ready-section query, and
  ready-section publish.
- `runtime_sync_ms` remains the high-level wall-clock sync measure, so the new
  fields are attribution for the same event rather than a separate policy.
- Upload timing now also separates queue/enqueue/select from draw-resource
  `apply_section_updates`.

Measurement after the split, Quest Android XR per-eye settled orbit, render
distance 7, 2 render compile workers, 30-second sample:

| Result accept budget | Frame avg / p95 / p99 / max | Max runtime sync split | Max upload / ready / records | Read |
|---:|---|---|---|---|
| unbounded | 15.908 / 26.562 / 42.082 / 70.163 ms | sync 42.712 ms; accept 1.381; prepare 14.181; submit 42.364 | upload apply 10.552; ready query 2.492; ready publish 7.386; records 13.590 | max 2 accepted results, 32 completed sections, 13 uploaded |
| 1 | 15.860 / 25.783 / 37.986 / 105.877 ms | sync 62.588 ms; accept 6.205; prepare 2.258; submit 61.234 | upload apply 13.388; ready query 11.819; ready publish 15.094; records 24.264 | max 1 accepted result, 1 queued result, 16 completed sections, 8 uploaded |

Interpretation:

- The new marker supports the control-point conclusion: completed-result
  acceptance itself is not the primary tail. It topped out at 1.381 ms
  unbounded and 6.205 ms with budget 1 in this run.
- The largest runtime-sync tail is compile submit/snapshot handoff
  (`max_runtime_submit_ms` 42-61 ms). In the native path, each submit currently
  clones the live client chunk snapshots into an owned `Vec` for the worker
  request, even when the compile target is a small ready-section set.
- Ready-section publish and prepared-record maintenance are also meaningful
  second-order costs: budget 1 saw 15.094 ms ready publish and 24.264 ms shared
  record work in the max frame.
- The split run had worse overall frame averages than the previous control
  runs, so use it for attribution rather than as a clean before/after
  performance comparison.

Targeted native snapshot pass:

- Native single-view runtimes now build compile requests from only the target
  render-section chunks plus their four horizontal mesh neighbors. The old
  generic `snapshots_for_submit` hook remains for web, where the worker-side
  resident mirror already uses a delta stream.
- This keeps the Java-shaped admission and backpressure policy unchanged. It
  only reduces the owned snapshot payload sent to native compile workers.
- A focused app-runtime test verifies that a target chunk compile request omits
  unrelated loaded chunks while retaining the target chunk and cardinal
  neighbors needed for cross-chunk face culling.

Measurement after targeted native snapshots, Quest Android XR per-eye settled
orbit, render distance 7, 2 render compile workers, 30-second sample:

| Result accept budget | Frame avg / p95 / p99 / max | Max runtime sync split | Max upload / ready / records | Read |
|---:|---|---|---|---|
| unbounded | 14.217 / 16.075 / 24.536 / 38.511 ms | sync 13.430 ms; accept 1.352; prepare 1.947; submit 11.548 | upload apply 4.544; ready query 1.413; ready publish 8.616; records 1.858 | max 2 accepted results, 32 completed sections, 15 uploaded |
| 1 | 14.319 / 16.232 / 24.815 / 44.506 ms | sync 17.094 ms; accept 1.162; prepare 2.102; submit 15.414 | upload apply 3.913; ready query 1.532; ready publish 1.568; records 1.783 | max 1 accepted result, 1 queued result, 16 completed sections, 8 uploaded |

Interpretation:

- This is a clear performance win for the previous long pole. Compared with the
  split-attribution unbounded run, max runtime submit fell from 42.364 ms to
  11.548 ms. Frame p95 fell from 26.562 ms to 16.075 ms, p99 from 42.082 ms to
  24.536 ms, and max frame from 70.163 ms to 38.511 ms.
- The default unbounded result-acceptance path remains stronger than budget 1.
  Budget 1 slightly improved average app headroom in this sample, but worsened
  p95/p99/max frame and dropped-frame count. Keep it diagnostic/opt-in.
- `runtime_submit_ms` is still visible at 11-15 ms, even after shrinking the
  payload to five chunk columns. The remaining cost is likely still owned
  snapshot cloning / request handoff, not compile admission policy.
- Ready publish, per-eye terrain cull/encode, and stereo submit/poll waits now
  show up as comparable second-order tails depending on run variance.

Submit split pass:

- Added `max_runtime_submit_snapshot_ms` and
  `max_runtime_submit_handoff_ms` to
  `MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME`.
- `submit_snapshot_ms` covers target-section neighbor selection plus
  `ChunkSnapshot` cloning for the request payload.
- `submit_handoff_ms` covers `submit_prepared_sync_plan(...)`: request
  construction, compiler submission, inflight marking, ready-plan application,
  and cache-update bookkeeping.

Measurement, Quest Android XR per-eye settled orbit, render distance 7, 2
render compile workers, unbounded completed-result acceptance, 30-second sample:

| Frame avg / p95 / p99 / max | Max runtime sync split | Max upload / render tails | Read |
|---|---|---|---|
| 14.240 / 16.332 / 24.325 / 37.713 ms | sync 16.256 ms; submit 14.130; submit snapshot 0.979; submit handoff 13.873 | upload apply 10.743; ready query 2.509; ready publish 1.463; left-eye encode 19.994 | max 2 accepted results, 32 completed sections, 15 uploaded |

Interpretation:

- Snapshot selection/clone is not the remaining submit long pole. It is below
  1 ms in the max submit frame.
- The remaining tail sits inside the handoff bucket. That does not prove the
  raw `mpsc` compiler send is expensive: the bucket also includes request
  construction and ready-plan/dirty-state mutation. In particular, the max
  compile frame still reports thousands of deferred sections, so BTreeSet
  churn around applying the ready plan is a plausible culprit.
- Do not jump straight to native resident snapshot mirrors from this result.
  They may still be desirable for parity and memory ownership, but they are not
  the next measured performance lever.

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

The `submit_handoff_ms` bucket is now split in the timed native path and exposed
on Android XR as `MCLONE_ANDROID_XR_PERF_TERRAIN_SUBMIT_MAX`: request build,
compiler submit, compiler capacity check, compiler command send, compiler
pending-count mark, mark inflight, apply ready plan, ready-update generation,
and the relevant dirty/inflight/request/payload counts are visible.

Measurement, Quest Android XR per-eye settled orbit, render distance 7, 2 render
compile workers, unbounded completed-result acceptance, 45-second sample:

| Frame avg / p95 / p99 / max | Runtime submit split | Submit handoff split | Read |
|---|---|---|---|
| 14.211 / 15.849 / 24.616 / 63.979 ms | submit 13.078 ms; snapshot 0.859; handoff 12.735 | single handoff 12.699; request count 2; request build 0.060; compiler submit total 12.653; compiler submit single 12.653; command send total 12.651; command send single 12.651; capacity check 0.000; pending mark 0.001; mark inflight 0.017; apply ready plan 0.670; ready update 0.009 | 32 ready sections, 3552 deferred sections, 10 request snapshots, 72 snapshot sections, 102 light sections, 32 revisions, 587300 estimated payload bytes total, 306164 single |

Interpretation:

- Raw compiler handoff is not cheap in the worst frame. It dominates the old
  handoff bucket.
- The request-count follow-up shows this is not just two admitted requests
  accumulating: one compiler submit can take essentially the whole submit spike.
- The inner-submit follow-up narrows that further: the spike is in
  `std::sync::mpsc` command send (`12.651 ms` single submit here), while the
  explicit capacity check and render-thread pending-count mutation are
  effectively zero.
- Ready-plan/deferred-section bookkeeping is not the measured submit spike in
  this run: `apply_ready_plan` is still sub-millisecond even with 3552 deferred
  sections.
- The next backpressure slice should focus on native compile dispatcher/request
  transport: replace the unbounded `mpsc` command send with a bounded,
  nonblocking/preallocated queue shape or enqueue a compact resident-worker
  descriptor instead of an owned request payload.
- In parallel, continue tracking upload-apply, ready publish, prepared-record
  rebuild, and per-eye terrain encode tails because they remain comparable to
  runtime submit in worst frames.
