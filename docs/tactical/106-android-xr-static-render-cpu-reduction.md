# 106: Android XR Static-Render CPU Reduction (Cull, Records, Stereo Encode)

Status: active tactical. Slices F, G, and H have landed; Slice I full-frame
multiview is available behind the opt-in path and measured as correctness-ready
but not a decisive standalone performance win. Continues
[`099`](099-android-xr-rd10-render-cost-attribution.md) from the end of its
Slice C2. 099 established, with on-device Meta metrics, that standalone Quest 3
RD10 is **CPU draw-submission bound with ~40% GPU headroom**, and landed the
diagnostics plus the single-submit / shared-records slices. The single-submit
slice was later reverted by `d0c5161` after headset validation exposed a left-eye
projection regression; see "Post-Validation Rollback" below. This doc records
the sharper diagnosis (the residual CPU cost is *container-bound*, not
math-bound), the convergent target architecture, the ordered remaining slices
with estimates and tradeoffs, an on-device **experiment ladder (E1-E6)** to
generate our own evidence rather than inheriting Playbox's conclusions, and an
allocation/copy backlog. **E1 (GPU-timestamp split) landed and materially revised
the diagnosis — see "E1 Result" below; the frame is GPU-bound serial CPU+GPU, not
"7ms GPU with 40% headroom." E3 (allocation/copy wins) and E2 (contention probe)
both landed, but E1/E2 instrumentation depended on the reverted single-submit
path. Treat their numbers as historical diagnostics until a future safe
single-submit or multiview path gives each eye immutable per-submission uniforms.**

Scope: standalone Quest / Android XR APK only, not desktop-hosted OpenXR
streaming to a Quest client. Keep every change behind the shared XR/render
contracts; do not fork mclone rendering for Quest.

## Reproduced Baseline (carry-forward)

Fresh `native:android-xr:perf:frozen:rd10` reproduction on the current build
(`2026-06-29`, frozen lane, fixed pose `0,80,-96,180`, 179 drawn sections,
1.35M drawn indices, runtime poll/sync/upload all `0.000ms`). Consistent with
099's Slice C2 record (run-to-run / thermal variance of ~1ms):

| Metric | Value | Budget (72 Hz) |
|---|---:|---:|
| Frame avg / p50 / p95 / p99 | `15.72` / `15.69` / `17.39` / `18.65` ms | `13.889` ms |
| Effective FPS | ~62 | 72 |
| App GPU frametime (both eyes, from 099 metrics run) | `~7.0` ms @ `~57%` util | — |
| Shared records (built once/frame) | `1.545` ms | — |
| Left / right eye cull | `2.567` / `2.593` ms | — |
| Left / right eye prepare (cull+uniform+translucent) | `3.058` / `2.787` ms | — |
| Left / right eye encode (section encode) | `2.238` (`1.484`) / `2.178` (`1.767`) ms | — |
| Stereo finish / submit / poll wait | `0.033` / `0.513` / `11.457` ms | — |

Marker maxima are independent per-field maxima, not one additive frame.

## E1 Result (landed 2026-06-29; instrumentation later removed by `d0c5161`) — the frame is GPU-bound, not "7ms + headroom"

E1 added an opt-in wgpu `TIMESTAMP_QUERY` bracket around the whole stereo command
encoder (`--perf-gpu-timestamps`, marker `MCLONE_ANDROID_XR_PERF_GPU`). Quest 3's
OpenXR Vulkan adapter advertises the feature (`timestamp_period=52.083ns`). On
frozen RD10 the measured GPU time is **`10.680ms` total (both eyes)**, `~6.2ms`
per eye — and the blocking stereo poll wait on the same run was `11.139ms`. The
full row is in [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
(2026-06-29 GPU-timestamp split).

This revises the core diagnosis:

1. **The poll wait is real GPU execution.** The in-stream GPU-clock timestamp
   (`10.680ms`) and the OS fence wait (`11.139ms`) agree within `~0.46ms` — that
   residual is the entire submit/acquire/fence cost. The `11`-vs-`7ms` gap is
   resolved: it was never CPU/submit overhead hiding in the poll.
2. **The Meta `app/gpu_frametime` counter under-reports.** It read `7.0ms` in the
   099 baseline and a nonsensical `1.706ms` on the E1 run, versus the direct
   `~10.7ms`. The "GPU ~7ms, ~40% headroom, CPU draw-submission bound" framing in
   099 and the headers below was an artifact of trusting that counter. Where this
   doc says "GPU ~7ms" or "~40% GPU headroom," read `~10.7ms` and `~25%` instead.
3. **The frame is a balanced serial `CPU(~9ms) + GPU(~10.7ms)`** (sums to the
   `~19.96ms` terrain-frame max). The CPU is blocked-idle for the whole `~10.7ms`
   poll.

Consequences for the slice plan (the slices are still correct; their *weighting*
changes):

- **CPU-only wins (F/G/H) cannot reach 72Hz at RD10 by themselves.** Even CPU→0
  leaves a `~10.7ms` GPU floor that, with no overlap, *is* the frame time. F/G/H
  remain worth doing (low-risk, shrink the CPU half, make overlap easier and help
  lower distances), but they are no longer a plausible standalone path to a
  comfortable RD10.
- **CPU/GPU overlap (E2 / Slice K) is promoted to a first-class lever.** There is
  `~10.7ms` of CPU-blocked-idle every frame. Overlapping next-frame CPU prep with
  the GPU poll would take the frame toward `max(CPU, GPU) ≈ 10.7ms` — under the
  `13.889ms` budget. This is the single highest-leverage change for RD10 and E2
  should run right after the cheap E3 wins.
- **GPU-side reduction attacks the `~10.7ms` floor.** Multiview (Slice I) cuts
  per-eye vertex/draw cost; FFR cuts fragment fill. With true GPU at `~10.7ms`
  (not `7ms`) these matter more than previously weighted.

## E3 Result + Overlap Recommendation (landed 2026-06-29)

E3 worked the allocation/copy backlog as Slices F and G (both landed; rows in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)).
Measured on frozen RD10 against the original baseline at comparable GPU poll
(so the deltas are clean CPU wins, not thermal):

| Bucket (max) | Baseline | After E3 (F+G) | Delta |
|---|---:|---:|---:|
| Shared records (Slice F) | `1.537ms` | `0.017ms` | `-1.52ms` |
| Per-eye cull L/R (Slice G) | `2.28`/`2.34ms` | `1.65`/`1.48ms` | `~-0.75ms`/eye |
| CPU command-build wall L/R | `4.53`/`4.26ms` | `3.74`/`2.93ms` | `~-2.1ms` total |
| **Frame p50** | **`15.66ms`** | **`13.866ms`** | **`-1.8ms`** |
| Frame p95 / over-budget % | `17.29ms` / `94%` | `15.06ms` / `48%` | — |
| Stereo poll (GPU) | `10.54ms` | `10.90ms` | `+0.36ms` (noise) |

- E3 removed `~2-2.5ms` of CPU per frame (record cache + fast-hash cull) and
  pulled frame **p50 under the `13.889ms` 72Hz budget** with the GPU half
  unchanged — better than the E1 "CPU can't help" framing implied, because the
  CPU half (`~9-10ms`) was a larger share of the frame than Meta's `7ms` GPU
  counter had suggested.
- **RD10 is now borderline, not solid, 72Hz.** p95 is `15.06ms` and ~48% of
  frames still miss budget: the serial `CPU(~3-4ms p50) + GPU(~9.5-10.9ms)`
  structure means GPU spikes and heavier frames push the tail over.

## Slice H Result (landed 2026-06-30) - shared stereo terrain prep removes duplicate cull, but RD10 remains tail-limited

Slice H builds one `PreparedTexturedSectionStereoDraw` per XR frame and feeds it
to both eyes. The shared cull tests every candidate section against both eye
frustums and keeps the exact union; translucent collect/sort also happens once
from the midpoint view. The same prepared stereo draw now feeds the per-eye
full-frame path, the frozen terrain-only probe, and the full-frame multiview
path. This intentionally preserves the multiview-compatible shape: one terrain
draw list can feed both layers. It does **not** yet batch section draws or avoid
the conservative union draw set on the per-eye path.

Timing attribution caveat: the existing per-eye terrain-prep marker has no
shared-prep bucket, so the shared cull/collect/sort time is charged to the
left-eye prep bucket and the right-eye cull/collect/sort buckets drop to
`0.000ms`. Do not read that as "left eye got slower"; it is the shared stereo
work.

On-device Quest 3 validation (frozen RD10, fixed pose `0,80,-96,180`, based on
`1c8fdd2`, captured before this Slice H commit) shows the structural shift but
not a decisive end-to-end win:

| Path | avg | p50 | p95 | p99 | max | over budget | key terrain bucket |
|---|---:|---:|---:|---:|---:|---:|---|
| per-eye + Slice H | `13.841ms` | `13.653ms` | `15.262ms` | `18.313ms` | `25.584ms` | 526/1440 | left shared cull `1.736ms`, right cull `0.000ms`, poll `10.011ms` |
| multiview + Slice H | `13.843ms` | `13.728ms` | `14.776ms` | `17.892ms` | `25.715ms` | 546/1440 | multiview terrain `3.052ms`, poll `11.408ms` |

Compared with the immediately prior matched multiview marker run in 107,
multiview terrain CPU max moved from `4.059ms` to `3.052ms`, which is the
expected shared-prep savings. Frame time barely moved because the lane is still
dominated by submitted GPU/poll time and tail pacing. The conservative union draw
set reports `203` drawn sections / `1,523,148` drawn indices in this run, versus
the earlier per-eye cull's lower per-eye draw counts; that extra clipped edge
work can offset some CPU savings on the GPU-bound path.

Conclusion: Slice H is worth keeping as architecture cleanup and as a multiview
prerequisite, but it does not make RD10 comfortable by itself. The next perf work
should either cut per-section draw/GPU overhead (Slice J batching / indirect
arena) or test the runtime-toggleable overlap path (Slice K/E4) with motion-to-
photon measurement. If we keep optimizing the per-eye default before batching, a
possible refinement is a shared stereo traversal that records per-eye draw masks
so per-eye submit can avoid drawing the full union while multiview continues to
use the union list.

### Recommendation on E2/E4/E5 (overlap): pursue E2/E4 next; defer E5

Grounded in the measured CPU-blocked-idle time:

- **E1 measured the CPU sitting idle for the entire `~10.7ms` GPU poll** every
  frame, and E3 shrank the CPU half to `~6.7ms` (max; `~3-4ms` at p50). With no
  overlap the frame is `CPU + GPU`. **With overlap it is `max(CPU, GPU) ≈ the GPU
  poll ≈ 10.9ms`** — comfortably under the `13.889ms` budget, tail included. This
  is the single highest-leverage remaining change for RD10, and the idle time to
  reclaim is large and real. **Recommendation: pursue overlap.**
- **Prefer E4 (frame pipelining / Slice K) as the overlap mechanism**, with E2 as
  the cheap de-risking probe first. After Slice F the only *pose-independent* CPU
  left to move off the critical path (E2's "run next-frame prep during the poll")
  is nearly free (records are cached), and cull/encode are pose-dependent — so the
  real win is cross-frame: submit frame N, do frame N+1's `xrWaitFrame`/cull/
  encode while N's GPU runs, and block on N's fence only before releasing N's
  image. **E2 has now run** (see "E2 Result"): a deliberately heavy CPU load
  during the poll inflates the GPU by only `~1.9ms`, so the overlapped
  `max(CPU, GPU+contention)` frame stays under budget — contention is small enough
  to commit to E4.
- **E4 cost to weigh before landing:** it adds ~1 frame of latency; motion-to-
  photon is already `~25-39ms`, so measure MTP before/after and keep a comfort
  check (per Slice K). Keep all threading inside the shared job system
  ([`062`](062-shared-threading-topology.md)) — desktop OS threads, browser Web
  Workers — not a Quest-only hack.
- **Defer E5 (no-block via `wgpu-hal` semaphores).** It is the highest-risk path
  (raw Vulkan/OpenXR semaphore integration `wgpu` does not expose) and E4 already
  captures the overlap win with ordinary fences. Revisit only if E4's added
  latency proves unacceptable.
- **Complement overlap with GPU-side reduction.** Even with overlap the frame is
  gated by the `~10.7ms` GPU. Stereo multiview (Slice I) is the
  mclone-appropriate way to cut per-eye vertex/draw cost; FFR can cut fragment
  fill. These lower the `max(CPU, GPU)` floor itself and matter more now that the
  true GPU is `~10.7ms`, not `7ms`.

## E2 Result (landed 2026-06-29; probe later removed by `d0c5161`) — unified-memory contention is real but small; E4 is a go

E2 added an opt-in poll-contention probe (`--perf-poll-contention`, marker
`MCLONE_ANDROID_XR_PERF_CONTENTION`, lane
`native:android-xr:perf:frozen:rd10:contention`). While enabled, a single worker
thread (062 native OS-thread backend) streams a 16 MiB read-modify-write for the
whole duration of the blocking stereo `device.poll(Wait)` on alternating frames;
the app buckets the per-frame stereo poll wait and the GPU-clock stereo total
(E1's timestamps, also enabled) by loaded vs control. Within-run A/B keeps the
buckets thermally matched. Two back-to-back frozen RD10 runs (full row in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)):

| Bucket (avg) | Run 1 | Run 2 |
|---|---:|---:|
| Control GPU (uncontended) | `7.285ms` | `7.210ms` |
| Loaded GPU (contended) | `9.150ms` | `9.135ms` |
| **GPU contention delta** | **`+1.865ms`** | **`+1.925ms`** |
| **Poll-wait delta** | **`+1.897ms`** | **`+1.933ms`** |

The load is deliberately heavier than the real overlap target: one core streaming
16 MiB (past the LLC → real DRAM traffic) for the *entire* `~7-9ms` poll, versus
the real next-frame cull/encode (`~3-7ms`, a smaller mostly-cached working set).
So `~1.9ms` is a conservative upper bound on the contention E4 would actually see.

Findings:

1. **Concurrent CPU work inflates the GPU by `~1.9ms` (`~26%`)** — contention is
   real, not negligible, but bounded and highly reproducible (both runs within
   `~0.06ms`).
2. **The poll-wait delta equals the GPU delta to within `~0.03ms`.** This
   re-confirms E1 (the poll *is* GPU execution) and pins the inflation to real GPU
   slowdown (shared DRAM bandwidth / power-DVFS), not CPU scheduling — the main
   thread is parked in the fence wait, so the worker has a free core.
3. **E4 still wins decisively.** Serial today: `CPU(~3-4ms p50) + GPU`. Overlapped:
   `max(CPU, GPU + contention)`, GPU-dominated → `~9ms` at this run's cool GPU
   floor and `~12.6ms` even at E1's hot `10.7ms` floor — under the `13.889ms`
   budget across the thermal range, vs today's `~13.7ms` serial p50. The real
   overlap is lighter than the probe, so real contention should be `≤1.9ms`.

**Decision: proceed to E4 (Slice K).** Contention shaves at most `~1.9ms` off the
theoretical `max(CPU, GPU)` win and does not erase it — exactly the "changes the
size, not the existence" prediction. Carry two cautions into E4: (a) both probe
runs settled cool (`~7.2ms` control GPU; the frozen lane is idle terrain), so
contention near the SoC power ceiling is untested and the hot-frame tail must be
watched; (b) E4 adds `~1` frame of latency — measure motion-to-photon before/after
and keep the comfort check (Slice K). Defer E5 as before.

## Per-Eye Submit-Overlap Probe (landed 2026-06-30)

Before implementing true one-frame-late E4, this slice tested a lower-risk
overlap precursor on the correct per-eye submission path. The opt-in
`--xr-overlap-eye-submits` mode submits the left eye immediately, records and
submits the right eye while left-eye GPU work can already run, then waits once
before releasing the OpenXR images. It does **not** overlap frame N+1 CPU work
with frame N GPU work, does not add the E4 latency tradeoff, and does not require
cross-frame uniform lifetime changes. The package lane is
`native:android-xr:perf:frozen:rd10:overlap`.

On Quest 3 frozen RD10, fixed pose `0,80,-96,180`, `--perf-metrics`, against a
same-session default per-eye baseline:

| Path | avg | p50 | p95 | p99 | max | over budget | key sync marker |
|---|---:|---:|---:|---:|---:|---:|---|
| per-eye baseline | `13.948ms` | `13.776ms` | `15.139ms` | `17.219ms` | `27.603ms` | 647/1430 | L/R poll `7.473`/`5.761ms`; stereo poll `11.223ms` |
| per-eye-overlap | `13.851ms` | `13.865ms` | `14.918ms` | `15.373ms` | `24.825ms` | 700/1440 | L/R poll `0.000`/`0.000ms`; stereo poll `9.883ms` |

The mechanical sync change worked: per-eye poll waits moved to zero and the
single end-of-stereo wait was lower. The frame result is mixed: avg, p95, p99,
max, sampled app GPU, and motion-to-photon improved, but p50 regressed slightly
and over-budget frame count increased. Keep this mode as an opt-in diagnostic
and A/B lane, not the default. It is useful evidence that there is some sync
headroom in the current per-eye path, but it does not replace the real E4 work:
true frame pipelining still needs per-eye/per-frame immutable uniform slots
before next-frame CPU work can safely run while the previous frame is in flight.

## Per-View Uniform Frame Ring (landed 2026-06-30)

The first E4 prerequisite is now in place for the per-eye path: dynamic
per-view uniform buffers allocate a three-frame ring (`3 frames x 2 eyes`), and
the XR per-eye render paths rotate through those frame slots. This preserves the
left/right dynamic-offset protection from 107 while also preventing frame N+1
view-projection writes from reusing frame N's uniform regions when a later
latency-toggleable pipeline allows a previous frame to remain in flight. The
covered per-view uniforms are chunk terrain, sky, actors, selection outline, and
world GUI. The underwater screen-effect vertex upload also uses the same
per-view frame slot count, because that overlay is generated per view.

## Transient Per-View Upload Ring (landed 2026-06-30)

The second E4 prerequisite is now in place for the per-eye path: transient
per-view upload resources use the same frame-ring slot as their draw uniforms.
Sky disc/glow vertices, selection outline vertices, world-GUI panel vertices,
world-GUI line vertices, the GUI vertices used to render the menu panel texture,
and the world-GUI panel texture itself no longer reuse the same GPU region for
frame N+1 while frame N may still be in flight. Actor geometry is already
uploaded into fresh per-render buffers, terrain section meshes are persistent,
and the underwater screen-effect vertex upload was covered by the prior uniform
ring slice.

This still does **not** enable true E4 by itself. The per-eye render resource
lifetime is now structured for pipelining, but the app still waits in the current
frame flow. The next slice is the runtime-toggleable one-frame-late scheduling
mode itself: submit/present frame N while preparing frame N+1, keep the default
off, report the mode in perf summaries, and measure both frame timing and
motion-to-photon. If multiview ever gets the same cross-frame overlap treatment,
mirror this resource lifetime audit for its separate multiview uniform buffers
and single-view scratch uploads.

## Post-Validation Rollback (landed 2026-06-29)

Commit `d0c5161` (`Restore XR per-eye command submission`) reverted the risky
part of `264c723` (`Submit Quest XR stereo eyes together`) after in-headset
validation showed the left-eye projection was wrong/off-center while the right
eye looked correct. This was not the underwater FOV path: XR does not pass the
underwater overlay/FOV state into the render view.

The cause was uniform lifetime, not the projection math itself. The
single-encoder/single-submit path recorded the left eye and then the right eye
before submitting, while shared render resources still update per-view uniforms
with `queue.write_buffer` into reusable buffers. This is deterministic, not a
timing race: under wgpu queue ordering, writes issued before a submit are visible
to the command buffers in that submit in issue order. The recorded sequence was
effectively `write(left uniforms) -> record left -> write(right uniforms) ->
record right -> submit both`; by the time that submit executed, shared uniform
buffers held right-eye data, so left-eye draws could read right-eye uniforms.
Restoring per-eye encoder/submit/poll restores the prior ordering barrier:
left-eye commands complete before right-eye uniforms are written.

This was also broader than the chunk camera uniform. The same shared per-view
`queue.write_buffer(..., 0, ...)` pattern exists in chunk terrain, sky, actors,
selection outline, world GUI, and screen-effect renderers. A future stereo
single-submit path must fix the lifetime discipline across every per-view
renderer in the pass, not patch one chunk buffer.

The same revert removed the E1 GPU timestamp split and E2 poll-contention probe,
because both were built around one stereo command encoder and no longer matched
the safe per-eye submission path. Keep the E1/E2 numbers as useful historical
diagnostics for "poll wait is real GPU execution" and "contention is bounded,"
but do not treat the current `main` branch as having runnable E1/E2
instrumentation. Any future single-submit, multiview, or overlap slice must first
make per-eye uniform data immutable for the whole submission across all per-view
renderers. The minimal salvage path is a dynamic-offset uniform scheme backed by
a small per-eye/per-frame ring, so frame N's left/right data and any in-flight
frame N+1 data cannot alias. The strategic Slice I path is a multiview uniform
array selected by `@builtin(view_index)`, which solves the same lifetime problem
while also attacking the real GPU bottleneck. Do not re-land bare single-submit
alone; validate both eyes in-headset, and preferably capture/diff both eye
targets, before accepting performance wins.

## Sharper Diagnosis (new since 099)

### 1. The frame is CPU + GPU with no overlap

At the time of E1/E2, `render_prepared_frame`
(`mclone-xr-scene/src/lib.rs:713`) recorded both eyes into one encoder,
`queue.submit`ted once, then immediately blocked on
`device.poll(WaitForSubmissionIndex)` (`:796`). Current `main` after `d0c5161`
records/submits/waits per eye for correctness, but still has no cross-frame
CPU/GPU overlap. The GPU cannot start until submit, and the CPU is parked while
waiting for GPU completion. So frame time is essentially **serial CPU
prep/encode + GPU wait**, which is why p50 sits near the budget at RD10. Two
independent levers follow: make the CPU work cheaper, or overlap CPU and GPU
(latency tradeoff — see Slice K).

### 2. The cull cost is container-bound, not math-bound

This is the key new finding and it re-ranks the fixes. `cull_textured_sections`
(`mclone-render/src/chunk.rs:455`) is the dominant per-eye bucket (~2.6ms), but
the actual frustum geometry is trivial: the per-section test transforms 8 AABB
corners through the view-projection (`chunk.rs:710`), i.e. ~1,133 sections x 8
corners x 2 eyes ~= 18k SIMD `Mat4 x Vec4` products — **well under ~0.5ms** of
real math. The other ~4.5ms of two-eye cull is data-structure overhead:

- two redundant full passes over the records `BTreeMap` purely for the
  `loaded_section_count` / `loaded_index_count` diagnostics (`chunk.rs:464-470`),
  which are *view-independent* and recomputed twice per frame for a value that
  only changes on upload;
- `BTreeSet` inserts into `frustum_keys` / `ready_frustum_keys` / `drawn_keys`
  (O(log n), pointer-chasing, allocating) — `chunk.rs:473-490`, `:526-527`;
- the BFS using a `BTreeMap` `infos` + `VecDeque` with O(log n) lookups per
  neighbor (`chunk.rs:511-559`).

`RenderSectionKey` is a trivial 12-byte `Copy` struct (`mclone-mesh/src/data.rs:102`)
ideal for a flat `Vec` / dense index / `FxHashSet`, yet the entire draw-state
store and cull path use `BTreeMap` / `BTreeSet` with the default hasher
(`chunk.rs:1444-1446`). No `rustc-hash`/`ahash` dependency exists in the
workspace today.

### 3. Per-eye work is duplicated, and records rebuild every frame

- `prepare_render_records` → `collect_render_records` (`chunk.rs:1600`, `:1752`)
  rebuilds a fresh `BTreeMap` of **all** sections every frame, even though the
  records only change on section upload/removal/readiness — not on camera
  movement. 099's Slice C1 shares this build *across the two eyes within a
  frame*; it does **not** cache it *across frames*.
- Cull + translucent collect/sort run **per eye** although the two eye frustums
  are ~63mm apart and nearly identical, and both eyes start traversal from the
  same camera section.
- The opaque encode loop scans **all** `self.sections` doing a `BTreeSet::contains`
  per section, then issues **one `draw_indexed` per drawn section**, unbatched,
  instances `0..1` (`chunk.rs:1735-1744`, `draw_textured_mesh_range` at `:837`):
  179 sections x 2 eyes ~= 358 draws/frame, each with its own
  `set_vertex_buffer` + `set_index_buffer`.

## Convergent Target Architecture

The end state that makes all the cheap wins land in their best form:

> **cull once (shared, flat) -> encode once (stereo multiview) -> submit once.**

099's Slice B attempted to make submit/poll shared, but `d0c5161` restored
per-eye submission after the headset-only left-eye projection regression above.
The target remains valid, but `submit once` is now explicitly gated on fixing
per-eye uniform ownership first. The remaining duplication is the per-eye cull
(Slice H removes it) and the per-eye encode (Slice I / multiview removes it).
Slices F and G make the shared cull cheap. Slice J (batching) and Slice K
(CPU/GPU overlap) are the deeper levers if RD10 needs real headroom.

## Playbox CPU/GPU Wait Cross-Check

Checked against Playbox's mature wgpu/OpenXR loop (`~/code/playbox/src/xr/mod.rs`,
tactical `046-quest-hz-ramp-performance-baselines.md`) to test the hypothesis
that it pipelines CPU and GPU by doing "render then prepare." **It does not** —
and the reason it still fits a tighter 120 Hz budget sharpens this plan.

- **Same within-frame structure as mclone.** Playbox's loop is `begin_frame`
  (xrWaitFrame) -> read poses/UI/sim -> `prepare_frame_scene` (once, shared across
  eyes) -> record both eyes -> **one `device.poll(Wait)`** -> release images ->
  `end_frame` (`mod.rs:1631-1648`, `:3532-3551`, `:3683`). The blocking GPU-fence
  wait before swapchain release is present in both engines; 046 confirms
  "`render_eye_target` ... blocks with `device.poll(Wait)`" and that it
  "serializes the stereo frame." There is no render-then-prepare overlap and no
  CPU(N+1)/GPU(N) cross-frame pipeline in either engine.
- **Its `poll(Wait)` is GPU execution, not waste.** Playbox's GPU-timestamp slice
  shows `poll(Wait)` tracks measured render-pass GPU time within ~`0.9ms`
  (floor-baseline poll `3.19ms` vs GPU `2.95ms`; default cubes poll `5.81ms` vs
  GPU `5.59ms`). That ~`0.9ms` residual is the *entire* CPU cost of submit +
  acquire/release + readback. So "reduce the poll wait" means "reduce queued GPU
  work," not "restructure the wait."
- **Why Playbox fits and mclone does not is CPU encode size, not lane overlap.**
  Playbox's render-CPU (its "wall minus GPU" gap) is ~`0.9ms` because it batches
  draws with instancing and shares frame prep across eyes. mclone's render-CPU is
  ~`11ms` (shared records `1.5` + per-eye walls `4.6`+`4.6` + submit `0.5`),
  dominated by BTree cull (~`5ms`) and ~358 unbatched per-section draws (~`4ms`).
  Identical serial CPU+GPU model; mclone simply carries ~10x the render-CPU.
- **Playbox reached, and deferred, mclone's Slice K for the same reason.** 046:
  "avoid the CPU wait before release through proper graphics/runtime
  synchronization ... likely requires raw Vulkan/OpenXR semaphore-style
  integration that `wgpu` does not expose cleanly today, so it should not be the
  first slice"; its standing guidance is "keep the explicit wait and focus on
  reducing the queued GPU workload." That is exactly Slice K's posture here.

Implications, which do not overturn the plan but re-weight it:

1. The cheap CPU slices (F/G/H) plus draw batching (J) are the **core** levers,
   not optional polish: closing mclone's ~`11ms` -> ~`1ms` render-CPU gap is what
   lets serial CPU+GPU fit, exactly as it does for Playbox.
2. **Playbox's instancing trick does not port directly.** Playbox collapses many
   *instances of one mesh* into a single instanced `draw_indexed`. mclone's
   sections are each a *unique* mesh with their own vertex/index buffer, so
   batching needs a shared vertex/index arena + `multi_draw_indexed_indirect`
   (Slice J) and/or stereo multiview (Slice I) to remove the per-eye duplication.
   More work than Playbox needed, but the ~40% GPU headroom and Playbox's
   ~`0.9ms`-encode proof show the ceiling is high once the draw stream collapses.
3. **Build the GPU-timestamp split before Slice I/J** (see "Measure First").
   Playbox's `--gpu-timestamps` path (`mod.rs:3640-3679`, `collect_gpu_draw_timing`)
   is the porting reference; it is what proved poll == GPU there, and it will
   resolve mclone's ~`11ms` stereo-poll max vs ~`7ms` Meta-GPU gap (max-vs-mean,
   or extra queued passes such as sky/UI/selection-outline).

## CPU/GPU Overlap Mechanics (Why The Frame Serializes Today)

The frame is serial CPU-then-GPU **by construction, not by hardware limit**.
Foundational facts a future session should hold before the overlap experiments:

- `queue.submit()` is **asynchronous**: the GPU begins executing and the call
  returns; the CPU is free to keep running. CPU and GPU are independent
  processors that run concurrently by default. The frame only serializes because
  we then call `device.poll(Wait)` (`lib.rs:796`) and *choose* to stop the CPU
  until the GPU signals completion.
- That block exists to satisfy OpenXR's "GPU must finish writing the eye image
  before `xrReleaseSwapchainImage`/`xrEndFrame`" rule. The block-free alternative
  is a GPU-side semaphore the compositor waits on, so the CPU never stalls; wgpu
  does not expose that handshake ergonomically, hence the workaround. This is an
  API-ergonomics gap, **not** a hardware wall (Experiment E5 tests punching
  through it via `wgpu-hal` / raw Vulkan).
- The Quest SoC has unified memory: CPU and GPU share one memory bus, last-level
  cache, and a thermal/power budget. Concurrent CPU+GPU work therefore contends
  for bandwidth and can mutually slow down — **contention, not exclusion**. It
  changes the *size* of an overlap win, never whether one exists. The Adreno is
  also tile-based, so when GPU work actually executes relative to submit is
  workload-dependent; measure on-device rather than reasoning a priori
  (Experiment E2).
- What overlap buys: today frame ~= CPU(~`9ms`) + GPU(~`7ms`) serial ~= `16ms`.
  With overlap the frame is gated by `max(CPU, GPU)`, not the sum. Because
  `xrWaitFrame` paces wall-clock to the display period, the payoff appears as
  headroom / fewer missed frames, not a shorter wall clock.
- Do **not** over-index on Playbox's "wgpu can't do this cleanly" conclusions.
  Their *measurements* (poll == GPU time) are reusable method; their *decisions
  to defer* were for small batched scenes and their risk appetite. Re-derive ours
  from the experiments below.

## Experiments To Run First (Generate Our Own Evidence)

Cheap/diagnostic -> deep. Each should produce an on-device number, not an
inference. Keep diagnostics opt-in; never add always-on per-frame cost.

| # | Experiment | Tests | Effort | A good result tells us |
|---|---|---|---|---|
| E1 | GPU-timestamp split of the stereo poll | how much of the ~`11ms` poll is real GPU vs CPU/submit overhead | low | **DONE (2026-06-29): GPU `10.68ms` total ≈ poll `11.14ms`; the poll is GPU, Meta's `7ms` under-reported. Frame is serial CPU~9 + GPU~10.7. Instrumentation later removed by `d0c5161` with the single-submit rollback. See "E1 Result".** |
| E2 | Worker-thread overlap probe | run real next-frame prep on a job thread *during* `poll(Wait)`; does it run concurrently, improve wall time, or lose to bandwidth contention | low-med | **DONE (2026-06-29): a deliberately heavy single-core 16 MiB load during the poll inflates the GPU by `~1.9ms` (`~26%`); poll delta == GPU delta to `~0.03ms`. Contention is real but bounded — the overlapped `max(CPU, GPU+1.9ms)` frame stays under budget across the thermal range. Probe later removed by `d0c5161` with the single-submit rollback. See "E2 Result".** |
| E3 | Allocation/copy elimination | the backlog below | low | **DONE (2026-06-29): Slices F+G landed. shared_records 1.5ms->0.02ms, per-eye cull 2.3ms->1.5ms; frame p50 15.66ms->13.87ms (under budget at median, p95 still over). See "E3 Result".** |
| E4 | Frame pipelining | submit N, prepare N+1 pose-independent work, block on N's fence only before releasing N's image | med | the real latency<->throughput tradeoff (Slice K), measured |
| E5 | Semaphore / no-block via `wgpu-hal` | remove the CPU fence wait entirely; compositor waits GPU-side | med-high | whether Playbox's "wgpu can't" is actually true for us |
| E6 | Threaded cull | fan cull across the Quest's 8 cores via the shared job system | med | cull scaling headroom independent of overlap |

- E1 method: request `wgpu::Features::TIMESTAMP_QUERY` (Quest 3 reports support),
  bracket the eye render passes with timestamp writes, read back, surface on an
  opt-in marker. Port reference: `~/code/playbox/src/xr/mod.rs:3640-3679`
  (`collect_gpu_draw_timing`) and 046 sixth slice.
- Start with **E1 + E3** (cheapest, safe, own baseline), then **E2** (answers the
  overlap question directly), then decide E4/E5 from E1/E2 data.
- Keep overlap/threading work inside the shared job system
  ([`062-shared-threading-topology.md`](062-shared-threading-topology.md)), not a
  Quest-only thread hack: desktop native threads, browser Web Workers, same
  lifecycle.

## Obvious-Win Backlog (Allocations / Copies)

Per-frame / per-eye churn in the hot path, mostly movement-agnostic and
independently landable (this is E3, and it feeds Slices F/G/H):

- `collect_render_records` builds a fresh `BTreeMap` every frame
  (`chunk.rs:1752`) -> cache + dirty flag (Slice F).
- `cull_textured_sections` allocates 3x `BTreeSet` + `BTreeMap` + `VecDeque`
  every eye every frame (`chunk.rs:473-513`) -> reuse scratch buffers, flat
  structures (Slice G).
- redundant `loaded_section_count` / `loaded_index_count` full passes
  (`chunk.rs:464-470`) -> compute on section change, not per frame.
- translucent collect: `.iter().filter().collect::<Vec>()` + sort per eye
  (`chunk.rs:1686-1701`) -> reuse buffer, share across eyes (Slice H).
- `uniform_bytes` builds a `Vec` per eye per frame (`chunk.rs:1680`) -> reused
  staging / `bytemuck` POD.
- `vertex_bytes` / `textured_vertex_bytes` copy byte-by-byte via
  `extend_from_slice` (`chunk.rs:1797-1812`) -> `bytemuck` cast (upload/streaming
  path, not the frozen frame, but obvious waste).
- `actor_instances_from_presentations` allocates a `Vec` every frame
  (`xr-scene/src/lib.rs:742`).

## Ordered Slices (recommended order)

Continues 099's slice lettering (A-E landed there). Cheap, movement-agnostic,
fully-shared wins first; structural levers last. Re-benchmark frozen RD10 +
stationary RD10 after each.

### Slice F - Cache prepared records across frames

**Landed 2026-06-29** (commit `Cache prepared Quest XR culling records across
frames (Slice F)`). `prepare_render_records` now returns a cached `Arc`, rebuilt
only when `apply_section_updates` / `set_traversal_ready_sections` set a dirty
flag; loaded section/index counts are folded into the build. Frozen RD10
`shared_records` max `1.537ms -> 0.006ms`. Behavior-preserving (74 render tests
pass). Frame-level effect was masked by GPU thermal drift on that run; the clean
attribution is the `shared_records` bucket.

**Follow-up (same day): the cache now lives on the shared `&self` render path**
(`render_with_options_inner` via `RefCell`/`Cell` interior mutability), not just
the XR-only `prepare_render_records` entry point. So flat desktop, flat Android,
web, and headless — every single-view client culling through
`TexturedSectionDrawResources` — reuse the same cross-frame cache, the same way
Slice G's cull win reached them for free. XR re-confirmed unregressed
(`shared_records` `0.021ms`, p50 `13.838ms`, drawn sections `179`).

- Add a dirty flag to `TexturedSectionDrawResources`; rebuild the
  `PreparedTexturedSectionRecords` only when `apply_section_updates` /
  `set_traversal_ready_sections` change the section set, not every frame.
- Fold the view-independent `loaded_section_count` / `loaded_index_count` stats
  into that cached build so they stop being recomputed twice per frame
  (`chunk.rs:464-470`).
- Estimate: ~1.5ms/frame when static; still valid on every camera-only frame
  during movement (sections change only a few times/sec on streaming).
- Strict win, no visual change, low effort. This is the correct form of the
  "don't re-prep when the pose barely moved" instinct: key the cache on what
  actually invalidates the prep (section changes), not on pose, so it is
  movement-agnostic.

### Slice G - Flatten cull/encode data structures

**Landed 2026-06-29** (commit `Flatten Quest XR cull onto reused fast-hash
scratch (Slice G)`). The per-eye cull `BTreeSet`/`BTreeMap`/`VecDeque` are now
reused `rustc-hash` `FxHashSet`/`FxHashMap` scratch (`RefCell` on the draw
resources, cleared per call); `drawn_keys` is `FxHashSet` so the encode loop's
membership test is fast-hash too. Frozen RD10 per-eye cull `~2.3ms -> ~1.5ms`;
combined with Slice F, frame p50 `15.66ms -> 13.866ms` (under the 72Hz budget at
the median, GPU poll unchanged). Behavior-preserving (drawn `179`/`1,352,166`
unchanged, 74 render tests pass). Not done in this slice: flattening the
*persistent* draw-state store (`sections`/`visibility_sections`) off `BTreeMap`,
and the encode loop still scans all sections with a per-section `contains` rather
than iterating the drawn list — both deferred as lower-value follow-ups.

- Replace the `BTreeMap`/`BTreeSet` in the cull path and draw-state store with a
  dense layout: a flat `Vec` of records iterated linearly, section membership via
  a dense index + reused bitset (or `FxHashSet`/`hashbrown` with a fast hasher).
- The encode loop should iterate the drawn index list directly instead of
  scanning all sections with a per-section set lookup (`chunk.rs:1735`).
- Estimate: ~1.5-2ms/frame (this is where most of the container-bound cull cost
  goes). Strict win; medium effort. Add `rustc-hash` (or equivalent) to the
  workspace.

### Slice H - Single shared cull for both eyes

**Landed 2026-06-30** as shared stereo terrain prep. The implementation builds
one `PreparedTexturedSectionStereoDraw` from the exact union of both eye
frustums, uses the midpoint view for traversal seeding and translucent ordering,
and feeds that prepared draw to the per-eye full-frame, frozen terrain-only, and
multiview paths. The current per-eye path draws the shared union in both eyes;
that is correct but conservative, and it can raise clipped GPU work. The follow-
up options are per-eye masks for the per-eye path, Slice J batching, or Slice K
overlap.

- Run the records iteration + occlusion BFS **once per frame** from the midpoint
  camera (`center_position` is already computed at `lib.rs:723-724`); both eyes
  are in the same camera section so `traversal_start_keys` is shared.
- For the frustum gate, test each candidate against **both** eye view-projections
  and keep it if visible in either (exact union — no conservative-margin
  guesswork, which matters because Quest eye projections are asymmetric/canted, so
  the union of two frustums is not itself a frustum). Both eyes then draw the same
  union set; the extra edge sections one eye does not see are clipped by the GPU.
- Share the translucent collect + a single midpoint-sorted order across eyes
  (63mm of eye separation does not change distant-section depth order).
- This is the "extract a pure per-eye prepared draw/cull result from command
  encoding" step 099's Slice C2 flagged as next. It removes one full cull
  (~2.6ms) + one translucent collect/sort, and is the precondition that makes
  multiview natural (one draw list feeds both eye layers).
- Estimate: ~2.5-3ms/frame. Mostly safe (slightly conservative draw set);
  medium effort. Capture a screenshot — this is pixel-adjacent.

### Slice I - Stereo multiview encode

- Render both eyes in one pass with `OVR_multiview`: a single 2-layer array
  color+depth target, pipeline `multiview: Some(2)`, and a chunk shader that
  selects its eye view-projection from a `[2]` uniform via
  `@builtin(view_index)`. The GPU broadcasts each draw to both layers, so the
  ~358 draws/frame collapse to ~179 and the per-eye encode bucket collapses to
  one.
- Requires changing the swapchain from `array_size: 1` to a 2-layer array (or a
  2-layer offscreen target blitted to the two eye swapchain images) — today both
  are single-layer (`mclone-xr-graphics/src/lib.rs:284`, depth/color
  `depth_or_array_layers: 1` at `:333,:354`). Touches `mclone-xr-graphics`
  (swapchain/target creation) and the `mclone-render` chunk shader + uniform
  layout + render-pass attachment.
- This is a different axis from 099's Slice D (batching) — it cuts the *stereo*
  duplication and also reduces GPU draw-call overhead. It is the standard Quest
  stereo path (Playbox uses per-eye instancing instead; multiview is the
  mclone-appropriate choice given heterogeneous per-section meshes).
- Estimate: removes ~one eye's encode (~2ms CPU) plus GPU draw-submission
  overhead; medium-high effort. Pixel-affecting — screenshot both eyes.

### Slice J - Cut per-draw CPU/GPU cost (batching)

- 099's Slice D1: combine section vertex/index data into a shared arena and draw
  with `multi_draw_indexed_indirect`, so the remaining ~179 per-section draws
  become a handful. Note Playbox's instancing does not port directly — its
  objects are instances of one mesh, while mclone sections are each a unique mesh
  — so the arena + indirect path is the right primitive here (see the Playbox
  cross-check). Complements multiview (multiview halves *eye* duplication;
  batching cuts *per-section* overhead).
- Alternative D2 (`RenderBundle`) is a poorer fit once the draw list is shared
  and stable; prefer the megabuffer path.
- Do this only if Slices F-I leave RD10 short of the target distance. Measure GPU
  draw overhead first (see "Measure first").

### Slice K - Overlap CPU and GPU (deferred; latency tradeoff)

- Today `device.poll(Wait)` right after submit (`lib.rs:796`) forbids any
  CPU(frame N+1)/GPU(frame N) overlap. Pipelining across the OpenXR swapchain
  images (3 deep) could hide the ~7ms GPU behind the next frame's CPU.
- **Not free in VR.** It adds ~1 frame of latency, and motion-to-photon is
  already ~39ms (099 metrics run); +~14ms is meaningful for comfort. Treat as a
  last resort after CPU is already small, and measure motion-to-photon before/after.
- Playbox reached the same conclusion and deferred it: the no-wait path needs raw
  Vulkan/OpenXR semaphore sync `wgpu` does not expose, and its standing guidance
  is to keep the explicit wait and reduce queued GPU work instead (see the Playbox
  cross-check). So the realistic order is shrink CPU encode (F-J) first; revisit
  K only if GPU/CPU still cannot overlap enough at the target distance.

## Alternative Headroom Levers (not in the main path)

- **Application SpaceWarp / AppSW** (`compositor/spacewarp_mode` is `0` today):
  render at 36 and let the compositor reproject to 72. Mostly-static terrain is
  near its best case, but disocclusion edges artifact and it needs depth + motion
  vectors. A "buy headroom" lever orthogonal to the CPU work; evaluate only if the
  CPU slices cannot reach the target distance.
- **Fixed Foveated Rendering**: low value right now — we are not GPU-fill-bound
  (~57% util). 099 already flags that whether FFR is actually applied is
  unverified; worth confirming opportunistically, not as a perf slice.

## Measure First (de-risk the structural slices)

- A wgpu `TIMESTAMP_QUERY` split of the ~7ms GPU (099's Slice E, currently
  deferred) tells how much is draw-call overhead vs raster/fill — that predicts
  the GPU payoff of multiview (Slice I) and batching (Slice J) before building
  them. This is Experiment **E1** above; run it first.
- The container-bound cull claim can be confirmed cheaply by bracketing the
  frustum-test loop vs the set-building separately inside `cull_textured_sections`
  on a perf run; the estimate above predicts set-building dominates.

## Expected Cumulative Effect And The Product Question

- Slices F + G + H are movement-agnostic, low-risk, fully shared, and should take
  the serial CPU from ~9-11ms toward ~5-6ms — likely enough to bring RD10 to/near
  the 13.889ms budget and to make lower distances comfortable.
- But with no CPU/GPU overlap, frame ~= CPU + GPU, so even a ~5ms CPU still sits
  at ~5 + 7 = ~12ms at RD10. A *comfortable* RD10 at 72Hz likely needs multiview
  (Slice I, which cuts both CPU and GPU) and/or batching (Slice J), or a
  ship-distance decision.
- **Decide the target:** RD1 and RD5 already hold 72Hz; RD10 is explicitly a
  stress lane. What render distance do we want to ship at solid 72Hz on Quest 3?
  If ~RD7-8, Slices F-H probably suffice. If RD10+, Slice I (and J) are on the
  critical path. This answers 099's open question #4.

## Tradeoffs And Risks

- Slice H union cull draws a few edge sections per eye that that eye cannot see;
  harmless (GPU-clipped) but slightly raises drawn-section counts — keep the
  per-eye stats honest in diagnostics.
- Slice I changes swapchain/target shape; verify both eye images still present
  correctly (screenshot both layers) and that depth works per-layer.
- Slice K trades latency for throughput; do not land it without a
  motion-to-photon measurement and a comfort check.

## Guardrails (carry from 099)

- Keep diagnostics opt-in; never add always-on per-frame diagnostic cost to the
  headset loop (the prior ~76ms diagnostics regression is the cautionary tale).
- Do not fork mclone rendering, chunk streaming, or session runtime for Quest;
  extend the shared contracts and add platform adapters around them.
- For pixel-affecting slices (H, I, J): capture a headset screenshot and look
  before moving on.
- Always run the validator cleanup and verify `pidof com.kzahel.mclone.xr`
  empty, `mWakefulness=Asleep`, `mHoldingDisplaySuspendBlocker=false` after
  on-device runs.
- Record new frozen/stationary RD10 rows in
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  per landed slice, with the benchmarked commit.

## References

- Predecessor / diagnostics + Slices A-C2:
  [`099`](099-android-xr-rd10-render-cost-attribution.md).
- Records: [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  (2026-06-27 frozen RD10 Meta-metrics and render-split rows).
- Priority index: [`../topics/performance.md`](../topics/performance.md).
- Hot path (current line numbers):
  - stereo frame: `mclone-xr-scene/src/lib.rs:713` (`render_prepared_frame`),
    `:796` (the blocking `device.poll`), `:1140` (`render_eye_target`).
  - cull: `mclone-render/src/chunk.rs:455` (`cull_textured_sections`), `:710`
    (per-section frustum test), `:464-470` (redundant stats passes).
  - records: `chunk.rs:1600` (`prepare_render_records`), `:1752`
    (`collect_render_records`), `:1444-1446` (BTree draw-state store).
  - encode: `chunk.rs:1735` (per-section draw loop), `:837`
    (`draw_textured_mesh_range`).
  - swapchain/targets: `mclone-xr-graphics/src/lib.rs:284,:333,:354`
    (`array_size`/`depth_or_array_layers` = 1 today).
  - key type: `mclone-mesh/src/data.rs:102` (`RenderSectionKey`).
- Bench lanes: `native:android-xr:perf:frozen:rd10`,
  `native:android-xr:perf:frozen:rd10:metrics`,
  `native:android-xr:perf:stationary:rd10` (see `package.json`).
- Playbox CPU/GPU wait cross-check (this doc): blocking-poll structure and the
  GPU-timestamp evidence that `poll(Wait)` == GPU time live in
  `~/code/playbox/src/xr/mod.rs:1631-1648,:3532-3551,:3640-3679,:3683` and
  `~/code/playbox/docs/tactical/046-quest-hz-ramp-performance-baselines.md`
  (fifth/sixth slices). See also 099's Playbox section for stereo prep/draw
  batching.
</content>
</invoke>
