# 142: Throughput Policy With Quest RD5 Guardrail

Status: active implementation plan. Initial draft 2026-07-04; revised the same
day after a bottleneck-attribution review of the scheduler/publication code
paths and the Quest guardrail lane shapes; falsification experiments were run
the same day and their results are folded in below. The publication-valve
model is now **measured, not hypothesized**: a reverted 2-line publish-budget
prototype produced a `3x` full-view-ready improvement at desktop RD10 with
unchanged frame pacing.
Workstream: native Rust performance, desktop streaming throughput, Android XR /
Quest frame-pacing guardrails.

## Decision

Optimize throughput against desktop-shaped startup streaming, not against small
Quest render-distance controls and not against synthetic full-drain benchmarks
alone.

Validation split:

- **Primary throughput target:** desktop native startup-streaming at RD10
  (inner loop, every candidate) and RD15 (promotion runs).
- **Quest safety guardrail:** Quest OpenXR RD5 settled-orbit metrics, judged
  against the fixed numeric gate below, after every candidate.
- **Quest pressure check:** Quest OpenXR RD7 settled-orbit metrics for
  candidates that touch shared pacing, worker, publication, admission, or
  upload policy. RD7 is a do-not-worsen delta check, not a quality bar.
- **Quest streaming check:** one streaming-shaped Quest lane (existing flight
  RD7 lane or the `128`/`133` chunk-view churn lane) for candidates that change
  shared admission/publication/upload behavior. Settled orbits do not exercise
  those code paths; this lane does.
- **Long-run checkpoint:** desktop RD20/RD30 only when the question is
  explicitly high-distance full-view behavior.

RD5 is too small to prove throughput. It is useful because it is currently
clean enough to expose fixed XR/render regressions: the 2026-07-04 budgeted RD5
orbit reported `skipped_delta=0`, `11.677ms` average app work, `13.434ms` p95,
and `+2.212ms` average headroom. RD7, by contrast, had zero runtime skipped
frames but averaged `15.460ms` app work and was over-period for `81.9%` of the
sample. RD7's own tail attribution (runtime upload `13.902ms`, GPU upload
`10.139ms`, eye encode `~9.5ms`, stereo poll wait `10.074ms`, drawn indices
`1.51M` vs RD5's `0.93M`) says its problem is render/draw cost at that view
distance, owned by the `117`/`119`/`130`/`131` lanes — not streaming
throughput. `119` evidence already shows RD7 settled orbit near-clean
(`11.791ms` avg, `5.7%` over-period) with frame overlap plus prepared-record
dirty diffing, so RD7's negative headroom must not be used as a reason to keep
desktop throughput throttled.

## What Actually Limits Desktop (Measured)

### The server publication valve (primary; prototype-validated)

`ChunkScheduler::poll()` publishes at most
`DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET = 1` completed feature target and at
most `DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET = 1` completed light status per
call (`native/crates/mclone-server/src/scheduler.rs`). The driver is the
**gameplay simulation tick**, not the host loop:
`ChunkScheduler::tick_report_with_record_builders(...)` calls `self.poll()`
once per gameplay tick, so at the default `20/20/60` cadence client-visible
chunk production is capped near `20` chunks/sec on any hardware. Raising only
the host rate (`--simulation-cadence 60/20/60`) does **not** touch this valve —
see the falsified cadence sweep below.

The cap compounds with job serialization. A feature job stays `Running` until
its **last** target publishes (`mark_job_complete` fires at the end of
`publish_completed_feature_job`), and `enqueue_feature_job_for_missing_targets`
early-returns while any job is `Queued | Running`. So a background
`128`-target job generates quickly on the single `mclone-worldgen` thread, then
the thread idles ~`128` gameplay ticks (`6.4s` at 20 Hz) while publication
drips, before the next job can even be enqueued. Steady state is roughly
`128 / (gen_time + 128 x 50ms)` ≈ `17` chunks/sec — matching the measured
RD20 asymptote (`17.1` chunks/sec) and the flat `13.5`-`16.8` chunks/sec
across RD5-RD15.

Scope of the claim: this is a **desktop-proven throughput bottleneck**, not a
proven Quest win. Scheduler publication is heavier than receiving a packet into
a client buffer: it converts generated chunks to snapshots, marks holder state,
queues save/cache records, schedules light publication, emits ready events, and
unblocks later feature-job admission. That can throttle desktop full-view
readiness even when the client could have accepted more buffered updates. On
Quest, the first client accept/store step may be cheap and the renderer should
smooth downstream dirty/mesh/upload work at display cadence. Candidate A must
therefore be validated on Quest as a streaming safety question, not assumed to
be a Quest throughput optimization.

**Prototype validation (2026-07-04, reverted after measurement):** raising both
publish budgets from `1` to `32` — no other change — produced:

| Lane (RD10, workers 1, cadence `20/20/60`) | publish `1` | publish `32` | Delta |
|---|---:|---:|---:|
| Startup-streaming playable entry | `1.078s` | `0.300s` | `3.6x` |
| Startup-streaming first full-view ready | `27.54s` | `9.23s` | `3.0x` |
| Startup-streaming first render quiescent | `33.06s` | `21.04s` | `1.6x` |
| Frame work avg / p95 (60 Hz budget) | `5.41 / 7.14ms` | `5.45 / 7.08ms` | unchanged |
| Over-budget frames | `0 / 6000` | `0 / 6000` | unchanged |

Loading-settle isolation, same prototype:

| RD | Runtime settle publish `1` | publish `32` | chunks/sec `1` → `32` |
|---:|---:|---:|---|
| `5` | `12.42s` | `3.87s` | `13.6` → `43.7` |
| `10` | `33.38s` | `11.12s` | `15.9` → `47.6` |
| `15` | `64.83s` | `22.03s` | `16.8` → `49.4` |

Mesh settle was identical in control and prototype (e.g. RD15 `33.07s` vs
`32.39s`), confirming a clean single-variable result. Desktop client pacing
absorbed the `32`-per-tick publish bursts without a single over-budget frame
(`update_pump_stalled_frames=0`); server tick-time spikes were not
instrumented in this pass — the elapsed-budget design below addresses them
regardless.

Post-prototype, the next server-side limit is visible at ~`50` chunks/sec
(isolation) / ~`57` chunks/sec (streaming): the remaining feature-job
serialization bubble plus single-thread generation. That is Candidate A's
second half (overlap job admission with publication), with headroom beyond the
prototype's `3x`.

### Mesh drain: admission/scan-bound, not worker-bound (measured)

Single-worker cached-section throughput falls `2,093` → `889` → `465`
sections/sec across RD5/RD10/RD15 in the isolation lane — superlinear
degradation. The client admits one dirty chunk column per sync call
(`DEFAULT_RENDER_CHUNK_MESH_BUDGET = 1`,
`native/crates/mclone-app-runtime/src/lib.rs`) and each call re-prepares
against the full cached/deferred section state.

**Worker falsification (2026-07-04):** `--render-compile-workers 2` left the
isolation mesh drain flat at every distance (RD15 mesh `33.05s` vs `32.39s`
single-worker; RD10 `7.96s` vs `7.77s`). The bulk drain is bounded by per-call
admission/prepare cost, not compile parallelism, so Candidate B leads with the
per-call scan attribution and admission batch size. (Worker count still helps
the live per-frame path — `120` measured near-2x live submissions at 2 workers
— so it stays a later, profile-aware lever, not the drain fix.)

The per-frame streaming variant of the same limit is now the desktop trailing
edge: with the publish valve opened, RD10 full-view-ready hit `9.2s` but
render quiescence lagged to `21.0s` — ~`45` chunk-columns/sec, consistent with
the one-chunk-per-frame admission ceiling at 60 Hz.

### Upload (not currently limiting on desktop)

Upload totaled `0.7s` across the entire RD20 streaming run. Desktop has no
upload budget path (the XR accept/upload budgets are opt-in flags defaulting to
`None`), and each section allocates fresh GPU buffers via `create_buffer_init`
with no staging belt. Leave this alone until A/B land, then watch
`mesh_upload_worst_ms` and frame tails; buffer-lifetime/arena work stays owned
by `128`.

## Falsified Levers And Probe Bugs

Recorded so nobody re-runs dead experiments:

- **Host-only cadence increase does not help.** `--simulation-cadence
  60/20/60` (gameplay kept at 20 Hz) made streaming RD10 *worse*: full-view
  ready `27.54s` → `33.78s`, quiescent `33.06s` → `39.49s`. Publication is
  paced by the gameplay tick, and tripling the host loop only adds per-frame
  runner overhead (diagnostics refresh etc.) on the scheduler thread. The `140`
  cadence-sweep interpretation rule is retired; a cadence experiment would
  need to raise the *gameplay* rate, which changes simulation semantics and is
  not a throughput lever we want.
- **Loading-settle false idle at host 60.** `--loading-settle-perf` with
  `--simulation-cadence 60/20/60` exits in under a second with
  `target_ready_chunks=0`: `poll_until_idle_with_timeout` sees
  `runner_idle(...)` (empty queues, no pending jobs/publications) in the
  startup window before the first interest command produces work. Any future
  cadence/idle-sensitive use of this lane needs the idle predicate hardened
  (e.g. require loading progress to reach the target, or require at least one
  nonzero pending observation before accepting idle).
- **`runtime_poll_count` in loading-settle is the client pump loop**
  (~`663`/sec), not the scheduler publication cadence. Do not use it to infer
  the valve; the scheduler-side publish counters in the first slice below are
  the real instrument.

## Shared Throttle Inventory

Every Quest-motivated cap that also throttles desktop, in one place. "Plan"
names the candidate that may change it; nothing here changes by default.

| Throttle | Default | Owner | Why it exists | Plan |
|---|---|---|---|---|
| `DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET` | `1`/gameplay tick | `mclone-server/src/scheduler.rs` | publication hitch slicing (`030`) | Candidate A (validated `3x`) |
| `DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET` | `1`/gameplay tick | `mclone-server/src/scheduler.rs` | same | Candidate A |
| Single in-flight feature job; next job gated on full publication | structural | `scheduler.rs` (`has_incomplete_feature_status_job`, `mark_job_complete`) | job bookkeeping simplicity | Candidate A |
| Worldgen / light-status workers | `1` thread each | `worldgen_mailbox.rs`, `light_mailbox.rs` | bring-up shape | only if generation becomes the measured limit after A |
| `DEFAULT_RENDER_CHUNK_MESH_BUDGET` | `1` chunk/sync call | `mclone-app-runtime/src/lib.rs` | Quest frame safety | Candidate B |
| `DEFAULT_RENDER_SECTION_COMPILE_WORKERS` + in-flight cap | `1` | `mclone-app-runtime/src/render_assets.rs` | Quest sync/upload tails (`120`) | Candidate B, later lever (bulk-drain effect falsified) |
| Update-pump elapsed budget / unload cap | `2ms` / `16` | `mclone-app-runtime/src/lib.rs` (`133`) | Quest apply tails | keep; absorbed the `32`/tick prototype bursts cleanly |
| XR upload/accept/completed-result budgets | `None`; opt-in CLI flags | `mclone-xr-scene`, android-xr CLI | Quest measurement lanes (`128`) | unchanged; lane args, not defaults |

Any candidate that relaxes one of these for desktop must land it as an explicit
profile decision (below), not as a new global default.

## Throughput Profile Boundary

The landing zone for every candidate is a named, session-resolved throughput
profile (desktop-throughput vs headset-pacing), owning at minimum: publication
drain service budget, feature-job overlap policy, render compile chunk budget,
compile worker count, and (XR-only) upload/accept budgets. Servicing
publication from the gameplay tick is acceptable, but the target budget must be
derived from the configured tick period, active profile, and recent measured
cost, not from a magic chunk count that accidentally scales when tick rate
changes. If the tick period changes, reset moving averages/estimators before
using them for admission decisions. Desktop gets aggressive values; Quest keeps
current behavior until separately measured. Profile-gated values keep the same
scheduler/session contracts on all platforms; this is a values split, not a
code-path fork. The first candidate that introduces the profile plumbing must
run the full Quest gate set even if Quest values are nominally unchanged,
because plumbing mistakes are exactly how "unchanged" values change.

## Guardrails And Gates

Prerequisites before gating on Quest numbers:

- **Re-baseline clean.** Both 2026-07-04 Quest records were captured on a dirty
  worktree (flagged in the records). Recapture RD5 and RD7 settled orbit on a
  clean commit before treating the gates as binding.
- **Fix the Meta dropped-frame delta.** PerfMetrics currently logs one early
  absolute counter. Record a before/after delta per sample window; until then
  dropped frames are advisory, and `skipped_delta`/`app_work`/`headroom` are
  the binding fields.
- **Pin lane configs.** The orbit lanes pass `--render-compile-workers 2` and
  the `2/16/64` accept/upload budgets — a candidate policy, not the shipping
  default (defaults are workers `1`, budgets unbounded). Keep lane args stable
  across comparisons, and when a candidate changes a shared default, also run
  RD5 once in default config (the 2026-07-04 records already include a passing
  unbounded RD5 control).

Gates are **absolute numbers pinned to the clean baseline**, not
relative-to-previous-run. Relative gates ratchet: five consecutive candidates
each eating `0.3ms` of headroom would all individually pass "no material
regression" and collectively kill the lane.

**Quest RD5 gate (every candidate):**

- `skipped_delta = 0`
- `app_work_p95 <= 14.0ms` (baseline `13.434ms`)
- `headroom_avg >= +1.5ms` (baseline `+2.212ms`)
- `app_over_period_pct <= 8%` (baseline `3.7%`)
- no frames over 2x period (baseline `0`)

**Quest RD7 pressure check (shared-policy candidates):**

- `skipped_delta = 0`
- `app_work_p95 <= 19.5ms` (baseline `18.515ms`)
- `app_over_period_pct <= 85%` (baseline `81.9%`)
- frames over 2x period `<= 5` (baseline `3`)

**Quest streaming check (candidates changing shared admission, publication,
upload, or worker policy):** run the existing flight RD7 metrics lane or the
chunk-view churn lane. The first post-candidate run establishes the durable
baseline row; thereafter compare `app_work_p95/p99`, `max_runtime_upload_ms`,
`max_runtime_gpu_upload_ms`, `max_runtime_upload_apply_ms`, update queue depth,
and oldest-applied-update age. Until a durable baseline exists, treat this lane
as evidence-gathering, not pass/fail — but run it, because settled orbit leaves
the streaming machinery idle and cannot catch a burstier admission policy.
Candidate A is exactly the kind of change this lane exists for: it multiplies
the per-tick publish burst the client must absorb. This is a safety/attribution
lane, not proof that publication is an important Quest bottleneck; the first
question is whether larger publication drains show up as server runner spikes,
update queue age, client accept/store cost, dirty/prepare cost, upload cost, or
dropped-frame deltas.

**Desktop gates (every candidate):**

- RD10 startup-streaming: `first full-view ready` and `first render quiescent`
  improve by `>= 10%`, or the candidate improves a specifically attributed
  subphase without regressing the totals.
- movement-frame probe (`240` frames, `120 Hz`): over-budget frame count does
  not grow.
- startup-streaming p95 measured frame work does not grow more than `10%`
  (headless caveat applies; it is a coarse screen, not a pacing verdict).

**Noise floor:** deltas under `5%` on desktop settle/ready times are noise —
rerun before believing them. Promotion requires the RD10 result reproduced
once (two runs total). Quest runs within `0.5ms` of a gate boundary get one
rerun; Quest thermal state is a real variance source (`Slice F` in the records
was thermal-flat), so avoid back-to-back headset runs when a number is
surprising.

## Benchmark Loop

Per-candidate inner loop (minutes, not tens of minutes):

1. Desktop RD10 startup-streaming before/after (`6000` frames, `60 Hz`).
2. Desktop movement-frame probe.
3. Quest RD5 settled-orbit gate.

Promotion loop (before a default/profile value changes on main):

4. Desktop RD15 startup-streaming (`9000` frames at `60 Hz` — `6000` frames is
   `100s`, too close to the current expected RD15 quiescence to leave headroom
   for a regressed run to finish).
5. Second RD10 run (reproducibility).
6. Quest RD7 pressure check, plus the streaming check for shared-policy
   candidates.
7. One native-window desktop run with the same policy, watching present-path
   pacing. The headless lane serializes on `wgpu::PollType::Wait` and cannot
   answer swapchain/present questions.
8. Record results in `docs/performance-records.md` /
   `docs/quest-standalone-performance-records.md`.

Occasional checkpoints, explicitly outside the iteration loop: desktop
RD20/RD30 long runs; one contrasting-seed run (all current lanes share seed
`12345` at `0,0` — a biome-heavy or ocean-heavy spawn will price worldgen
differently and should occasionally sanity-check tuned budgets).

## Candidate Ladder

Ordered by the measured bottleneck model.

**A. Server publication drain policy (validated by prototype; implement
properly).** Replace the fixed `1`-per-tick feature and light publish budgets
with an elapsed-time drain budget serviced from the gameplay tick
(profile-aware: desktop gets an aggressive measured budget, Quest keeps current
values until a streaming lane proves a safe change), and decouple job admission
from full publication: mark the job pipeline-complete when the mailbox drains,
track the pending-publication backlog separately, and let the next feature job
enqueue while publication drains. Bound the pending-publication backlog (do not
enqueue a new job past N pending chunks) so generated-but-unpublished snapshots
cannot grow memory without limit. The fixed-count `32` prototype already
delivered `3x` full-view-ready at RD10 with clean desktop pacing; the
elapsed-budget version bounds server tick cost too, and job-admission overlap
should push past the prototype's residual `~50` chunks/sec toward
generation-bound rates.

**B. Mesh drain admission.** Worker count is falsified for the bulk drain
(flat at 2 workers, all distances), so B starts with per-call attribution:
time the prepare/scan vs compile vs upload split per sync call at RD15/RD20,
find the O(loaded-sections) component, then raise the per-call chunk budget
under the existing frame-deadline admission on desktop (the streaming trailing
edge is the one-chunk-per-frame ceiling, ~`45`-`60` chunk-columns/sec at
60 Hz). Sweep compile workers `2/4` only after the scan cost is understood,
as a live-path, profile-aware lever.

**C. Upload path.** Only with post-A/B evidence of upload pressure on desktop
(`mesh_upload_worst_ms`, frame tails during streaming). Buffer
reuse/staging/arena direction belongs to `128`.

**D. Publication pump cadence.** Only if A's elapsed-budget drain per gameplay
tick proves insufficient or hitchy: consider draining publications from the
host-rate loop or a dedicated pump instead of the gameplay tick. `116` owns
cadence semantics; do not fold this into A silently. (Raising the host rate
alone is a falsified lever and off the table.)

## First Slice

Falsification is done; the first implementation slice is Candidate A plus the
instrumentation to watch it:

- [x] Falsification: host-only cadence increase does not open the valve
  (measured worse; see above).
- [x] Falsification: publish budget `1` → `32` prototype delivers `3x`
  full-view-ready at RD10 with unchanged desktop frame pacing (reverted).
- [x] Falsification: compile workers `2` do not move the bulk mesh drain.
- [x] RD10 startup-streaming control baseline captured (doc-only-dirty tree,
  `4a3604cf`): playable `1.078s`, full-view `27.54s`, quiescent `33.06s`.
- [x] Add scheduler publication counters and surface them through runtime
  diagnostics, native startup-streaming JSON, frame-budget probe JSON, and Quest
  perf logs: feature jobs drained, feature chunks published/skipped, feature
  jobs completed, feature/light snapshot-ready events, light statuses
  published/skipped, light batches enqueued, pending worldgen-publication jobs
  and chunks, and pending light publications.
- [x] Sample queue depths as a time series (every `~60` frames) in
  startup-streaming JSON: pending publications, mailbox pending counts, pending
  compile jobs, `server_update_queue_depth`, update queue bytes/oldest-applied
  age, and render compile/upload activity.
- [ ] Add worldgen/light mailbox busy-vs-idle time if Candidate A still needs
  mailbox utilization after the publication counters and queue samples are
  captured on full RD10/RD15 lanes.
- [ ] Harden the loading-settle idle predicate against the startup false-idle
  window (required before any idle-sensitive sweep reruns).
- [ ] Add the Meta dropped-frame before/after delta to the Quest perf summary.
- [ ] Capture clean-commit baselines: RD15 startup-streaming (`9000` frames);
  recapture Quest RD5/RD7 settled orbit clean.
- [ ] Implement Candidate A behind the throughput profile and run the full
  promotion loop, including the Quest streaming check.

## Cost Questions To Resolve First

Before opening shared levers broadly, answer these with counters rather than
inference:

- Is desktop full-view readiness still dominated by scheduler publication after
  separating feature publish, light publish, update encode/send, and
  job-admission wait time?
- How much of `poll_scheduler_publish_completed_ms` is actual feature/light
  publication work versus update encode/send and downstream client apply work?
- On Quest streaming, does a larger publication drain remain invisible until
  downstream render work, or does it create measurable server runner spikes,
  update queue depth/age, client accept/store cost, dirty seed/prepare cost, or
  upload cost?
- Are the downstream client budgets actually smoothing 20 Hz server publication
  into 72 Hz render work, or do update bursts leak through as frame tails?
- Does job-admission overlap make worldgen itself the next limit, or does
  pending-publication memory/backlog become the practical bound first?

## Non-Goals

- Do not spend this workstream making Quest RD7 settled-orbit clean. Its
  over-period is render/draw-cost-bound and already near-clean under the
  `119` overlap + dirty-diff path; that default decision belongs to
  `117`/`119`, not here.
- Do not tune for RD5 throughput. RD5 is a guardrail/control lane.
- Do not remove Quest backpressure globally, and do not relax the `133` update
  pump or XR budget lanes as a side effect of desktop work.
- Do not use synthetic loading-settle as the success metric. It remains the
  attribution probe; startup-streaming full-view-ready/quiescent is the target.
- Do not fold speculative worker-pool scaling (multi-thread worldgen/light)
  into A. If the valve fix makes single-thread generation the limit, that is a
  new measured candidate with its own row.
- Do not raise the gameplay tick rate as a throughput lever; it changes
  simulation semantics (`116`) and the host-rate variant is already falsified.

## Risks And Blind Spots Kept Visible

- **Settled orbit cannot catch streaming regressions.** Mitigated by the
  streaming check; do not quietly drop it when it is inconvenient. Candidate A
  specifically multiplies per-tick publish bursts — the Quest streaming check
  and the `133` update-pump counters (queue depth, oldest-applied age) are the
  instruments that would catch the cost.
- **Server tick-time spikes were not instrumented in the prototype.** Desktop
  client pacing was clean, but the elapsed-budget design must cap per-tick
  publish work on the runner thread, and the A slice should record scheduler
  `publish_completed_us` before/after.
- **Absolute gates go stale.** When an intentional, accepted change moves the
  Quest baseline, re-pin the gate numbers in this doc in the same PR.
- **Publication backlog memory.** Candidate A holds more
  generated-but-unpublished chunks; the backlog bound is part of the design,
  and RD30 long runs should watch peak memory once A lands.
- **Per-status rates can look fine while latency rots.** The `133` churn
  evidence showed capped pumps trading burst cost for oldest-update age
  (`235ms`). Track oldest-pending ages on Quest streaming runs, not just
  rates.
- **Single-seed tuning.** All lanes share seed `12345`; the contrasting-seed
  checkpoint exists to catch budget values tuned to one biome mix.

## Links

- [`140-streaming-throughput-frame-pacing-baselines.md`](140-streaming-throughput-frame-pacing-baselines.md)
  keeps the setup-era evidence and baseline history (its cadence-sweep
  interpretation rule is superseded by the falsification above).
- [`../performance-records.md`](../performance-records.md) owns desktop and
  flat Android performance records, including the 2026-07-04 publication-valve
  attribution record.
- [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  owns standalone Quest/OpenXR records.
- [`139-vanilla-chunk-startup-scheduling.md`](139-vanilla-chunk-startup-scheduling.md)
  owns the playable gate and Java-shaped startup scheduling model; the
  `128`-chunk background jobs and center-first ordering come from there.
- [`030-native-streaming-publish-and-render-budget.md`](030-native-streaming-publish-and-render-budget.md)
  owns the original publication-slicing motivation.
- [`116-simulation-cadence-and-stepping.md`](116-simulation-cadence-and-stepping.md)
  owns cadence semantics.
- [`120-vanilla-render-compile-backpressure.md`](120-vanilla-render-compile-backpressure.md)
  owns the Java-shaped compile admission history.
- [`128-terrain-render-pipeline-coordination.md`](128-terrain-render-pipeline-coordination.md)
  owns the dirty-to-drawable coordinator direction and upload lifecycle.
- [`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md)
  owns update-pump budgets and the churn lane.
- [`117-android-xr-rd10-gpu-floor-and-frame-overlap.md`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md)
  and [`119-android-xr-live-streaming-frame-pacing.md`](119-android-xr-live-streaming-frame-pacing.md)
  own the Quest RD7/RD10 render-cost and overlap work.
