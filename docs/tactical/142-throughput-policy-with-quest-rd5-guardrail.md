# 142: Throughput Policy With Quest RD5 Guardrail

Status: active implementation plan. Initial draft 2026-07-04; revised the same
day after a bottleneck-attribution review of the scheduler/publication code
paths and the Quest guardrail lane shapes; falsification experiments were run
the same day and their results are folded in below. The publication-valve
model is now **measured, not hypothesized**: a reverted 2-line publish-budget
prototype produced a `3x` full-view-ready improvement at desktop RD10 with
unchanged frame pacing.
The persisted-world desktop-shaped split is also measured as of 2026-07-05:
RD10 already-generated startup reaches playable in `0.125s`, full target view
in `1.031s`, and stable actionable render idle in `25.142s` with `0/2400`
over-budget frames.
Workstream: native Rust performance, desktop streaming throughput, Android XR /
Quest frame-pacing guardrails.

Measurement hierarchy: [`../topics/performance.md`](../topics/performance.md)
is the parent performance index, and
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) is the
durable accounting/budgeting rulebook for any frame-pipeline policy change.
Use that rulebook's stage model, report schema, and gaps 9-10 before turning
this tactical's Candidate A/B notes into a new successor implementation
tactical.

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

## Stage Ownership And Interpretation

Do not read the publication prototype as the whole performance story. It proves
a desktop local-integrated bottleneck, while the remaining question is whether
opening that bottleneck is safe under the client/render pacing budgets on Quest
and under remote/multiplayer ownership.

| Stage | Local integrated play | Remote/multiplayer play | Current accounting |
|---|---|---|---|
| Terrain/features generation | server/worldgen work on the same device as client/render | host/server pays | mailbox pending counts; add busy/idle or per-job CPU time only if the new counters do not explain the waterfall |
| Light compute/status | server/light pipeline on the same device | host/server pays | light mailbox/status counts plus scheduler publication counters |
| Snapshot publication / ready events / save enqueue | server runner/scheduler thread on the same device | host/server pays | `poll_scheduler_publish_completed_ms`, feature/light publish counters, pending publication jobs/chunks/statuses |
| Store/persistence | server side, background write/cache paths on the same device | host/server pays | current records show queue/save side effects indirectly; add persistence timing only if it appears in the waterfall |
| Network/update receive | local encoded queue, no real network | client network/decode/apply cost | update queue depth/bytes/oldest-applied age and apply timing |
| Client apply / dirty marking | client device | client device | update apply, dirty mark, client apply timing |
| Mesh prepare / compile / upload | client device | client device | pending compile/upload counters and frame timing; Candidate B owns scan/admission attribution |
| Render / XR frame pacing | client device | client device | desktop frame-budget probe; Quest app-work/headroom/dropped-frame lanes |

Backpressure is still the right shape: server publication can stream ahead into
bounded queues while the client spreads apply, mesh, upload, and render work
across display frames. But backpressure does not make server work free in local
integrated Quest mode. The headset still shares CPU between server generation,
light, publication, client apply, mesh, upload, and render. In remote play, most
server-side costs leave the headset, so the key client questions become
network/decode/apply queue age, mesh/upload tail behavior, and render headroom.

The July 5 measurement pass confirmed the waterfall shape: desktop RD10/RD15
full-view readiness still tracks the `~19` chunks/sec publication wall with an
empty client update queue, while Quest RD5 local-integrated chunk-view churn has
good average headroom but visible update queue age, upload/accept backpressure,
and the same one-job publication backlog.

The July 6 tactical 149 contrast pass fixed the remote accounting gap and
captured two Quest RD5 local rows plus two Quest RD5 remote rows. Server and
scheduler work now visibly leaves the headset in remote mode: the client marks
`host-publication`, `server-runner`, `worldgen`, and `light-status` as
`remote-host`, while the dedicated server emits its own
`MCLONE_DEDICATED_SERVER_SUMMARY`. The headset-paid work changed shape rather
than disappearing: remote rows expose network producer read/decode and update
apply as client costs, keep mesh/upload/render local, and showed lower update
oldest-applied age but worse over-period rate than the local rows in that
capture. Use the remote lane only with both halves of the report, not as a
single headset-only row.

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

Post-prototype, the next desktop-shaped full-view limit is visible around
`50`-`57` chunks/sec, but that is not a hardware ceiling. The dedicated ceiling
split added on 2026-07-05 measured the same RD10 `529`-target footprint at
`551.5` raw cold feature chunks/sec (`1243.2` warm dependency-cache chunks/sec)
with no scheduler/light/publication/client, `126.7` server-only chunks/sec with
lighting disabled, and `40.6` server-only chunks/sec with lighting enabled.
That gives Candidate A a clearer job: separate current scheduler/job
admission/publication limits from real generation and light costs, then open
only the budgeted pieces that are safe under Quest frame-pacing gates.

**Budget response sweep (2026-07-05, temporary edits reverted):** RD10
startup-streaming with fixed feature/light publish budgets `1/4/8/16/32` showed
that full-view throughput saturates early. Budget `1` produced full-view
`27.492s` (`19.2` chunks/sec). Budget `4` produced full-view `9.681s`
(`54.6` chunks/sec). Budgets `8` and `16` were flat (`9.716s` / `9.728s`),
and budget `32` regressed full-view to `10.221s` while producing two 60 Hz
over-budget frames. This supports the shared budget-calculation direction:
open the drain based on measured elapsed work and backlog pressure, but do not
chase a high fixed count. A budget around the cost of publishing a few chunks
per gameplay tick appears to capture the desktop full-view win; job-admission
overlap is the likely next server-side throughput lever.

Lighting-off attribution under budget `4` improved full-view only to `8.154s`
(`64.9` chunks/sec). Light/status work is real, but it is not the main
remaining full-view wall after the publication drain opens.

**Ceiling-lane separation (2026-07-05):** `scheduler_loading_perf` is now the
server-only loading probe: it hot-polls `ChunkScheduler` until the target view
is visible and server queues drain, but it excludes the client update pump,
mesh preparation, GPU upload, and frame loop. Use it alongside `worldgen_perf`
and `startup-streaming`, not in place of them:

| RD10 lane | Lighting | Target chunks/sec | Meaning |
|---|---:|---:|---|
| `worldgen_perf --radius 11` features cold | N/A | `551.5` | raw algorithm, no scheduler/light/publication/client |
| `worldgen_perf --radius 11` features warm | N/A | `1243.2` | raw algorithm with dependency cache already warm |
| `scheduler_loading_perf --render-distance 10` | off | `126.7` | scheduler/job/publication ceiling without light or client mesh |
| `scheduler_loading_perf --render-distance 10` | on | `40.6` | scheduler plus light-status compute, still no client mesh |

The server-only lighting-on lane attributed `10.784s` to light-status compute
for `625` completed light statuses. Future experiments must keep four rows
separate: raw feature ceiling, server-only loading ceiling, desktop-shaped
full-view readiness, and render quiescence. It is valid to use raw/server
ceiling lanes to identify headroom; it is not valid to optimize for them alone
without rerunning desktop-shaped and Quest guardrail lanes.

**Persisted and mesh-only lanes:** the next separation pass should add
already-generated inputs rather than continuing to infer them indirectly. The
scheduler-only in-memory persisted reload mode is now implemented: pass 1
generates and lights the requested view into shared `ChunkRecord`s, pass 2
reloads the same view from that store and asserts worldgen/light work stays at
zero. RD10 reload from `625` in-memory records reached view-ready in `0.523s`
(`1012.1` chunks/sec) with `0` worldgen jobs and `0` light statuses. That
isolates scheduler load/publication over already-lit chunks without disk IO.
The temp SQLite persisted reload mode is now implemented too: RD10 reload from
the normal world-dir path reached view-ready in `0.572s` and settled in
`0.721s` from `625` records, with `0` worldgen jobs and `0` light statuses.
That prices real record decode/load/storage as a modest increment over memory
reload, not the many-second fresh-path wall. The mesh CPU-only lane is now
implemented on persisted SQLite snapshots: RD10 reload reached view-ready in
`0.540s`, then CPU mesh construction for `8464` target sections took `11.037s`
with no `wgpu` device, upload, frame loop, or runtime admission budget. The GPU
upload split is now implemented too: the same RD10 prebuilt mesh footprint
uploads `360 MB` of section data in `0.133s` on desktop, with post-upload
`device.poll` effectively zero. That confirms upload is budget-sensitive but
not the many-second wall on this machine. The persisted-world startup-streaming
lane now observes those pieces together in the real desktop frame loop: RD10
prewarm takes `33.421s`, measured reopen reaches playable in `0.125s`, full
target view in `1.031s`, and stable actionable render idle in `25.142s` with
`0/2400` over-budget frames. The final report still has `88` dirty/pending
render chunks but `ready_render_work_pending=false`, so dirty boundary
bookkeeping and actionable render work must stay separate in future counters.
Next, add the Quest persisted-world guardrail and instrument the paced render
admission/mesh tail before changing broad policy.

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
— so it belongs in the shared budget/admission calculation, not as the primary
bulk-drain fix.)

The per-frame streaming variant of the same limit is now the desktop trailing
edge. With publish budget `4`, RD10 full-view-ready hit `9.681s` but render
quiescence lagged to `20.089s`. Raising render compile workers from `1` to `2`
left full-view unchanged (`9.690s`) but moved render quiescence to `11.412s`;
workers `4` did not improve further (`11.639s`). This does not undo the
isolation finding that workers do not fix the full bulk drain, but it changes
the live-streaming interpretation: after publication opens, workers `2` are a
real desktop render-tail lever and Candidate B should measure worker count,
admission, and per-call scan cost together.

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

Any candidate that relaxes one of these must land as a measured budget
calculation, not as a new fixed global default.

## Budget Calculation Boundary

The landing zone for every candidate is one shared budget algorithm, not a
hardcoded desktop-vs-headset profile split. The algorithm owns at minimum:
publication drain service budget, feature-job overlap/backlog bounds, render
compile chunk budget, compile worker count, and (XR-only) upload/accept budgets.
Servicing publication from the gameplay tick is acceptable, but the target
budget must be derived from the configured tick period, recent measured work
cost, queue/backlog state, and session/device caps, not from a magic chunk count
that accidentally scales when tick rate changes. If the tick period changes,
reset moving averages/estimators before using them for admission decisions.

Desktop and Quest may resolve to different numeric limits because they have
different measured costs, refresh periods, CPU budgets, and local-integrated vs
remote ownership. That is an input/cap difference, not a separate code path or
manually selected platform mode. The first candidate that introduces this budget
plumbing must run the full Quest gate set, because plumbing mistakes are exactly
how "unchanged" limits change.

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

Promotion loop (before a shared budget calculation changes on main):

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
with an elapsed-time drain budget serviced from the gameplay tick. The budget
calculation should use tick period, recent publish cost, backlog age/size, and
device/session caps so it can open naturally on desktop without hardcoding a
desktop-specific branch, while staying bounded on Quest when local integrated
server work competes with rendering. The fixed-count sweep says the first target
is a modest drain equivalent to roughly `4` feature/light publications per
gameplay tick on this desktop lane, not `32`. Decouple job admission from full
publication: mark the job pipeline-complete when the mailbox drains, track the
pending-publication backlog separately, and let the next feature job enqueue
while publication drains. Bound the pending-publication backlog (do not enqueue
a new job past N pending chunks) so generated-but-unpublished snapshots cannot
grow memory without limit. The fixed-count prototype/sweep already showed that
opening the drain to this range delivers about `3x` full-view-ready at RD10
with clean desktop pacing; the elapsed-budget version bounds server tick cost
too, and job-admission overlap should push past the prototype's residual `~50`
chunks/sec toward generation-bound rates.

**B. Mesh drain admission and worker count.** Worker count is falsified for the
synthetic bulk drain (flat at 2 workers, all distances), but desktop-shaped
streaming after publication opens measured a clear workers-2 quiescence win
(`20.089s` -> `11.412s`) and no additional workers-4 win. B should therefore
measure the live prepare/scan vs compile vs upload split at RD10/RD15 with
workers `1/2/4`, then decide whether the production budget calculation should
raise workers to `2`, raise the per-call chunk admission budget, or both.

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
- [x] Falsification: opening the publish drain beyond budget `1` delivers `3x`
  full-view-ready at RD10 with unchanged desktop frame pacing (temporary fixed
  count prototypes reverted; later sweep found budget `4` captures the useful
  full-view win).
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
- [x] Capture directional desktop waterfall baselines with publication counters
  (2026-07-05, dirty/current-state): RD10 full-view `27.553s`, quiescent
  `32.834s`; RD15 full-view `57.190s`, quiescent `64.253s`; both had
  `server_update_queue_depth=0`, max pending publication chunks `125`, and
  zero 60 Hz over-budget frames.
- [x] Capture Quest RD5 local-integrated chunk-view churn with the new counters
  (2026-07-05): `skipped_delta=0`, `app_work_avg/p95=7.032/12.631ms`,
  headroom avg `+6.857ms`, but update queue depth/age reached `283` /
  `167.287ms`, update pump stalled once, runtime upload/GPU-upload max reached
  `11.594/9.977ms`, and pending worldgen-publication chunks reached `124`.
- [x] Attempt remote-dedicated RD5 chunk-view churn contrast. Result is a probe
  bug / architecture gap, not a baseline: remote still ignores `SendOnly`
  command policy, the sample caught a `5.7s` chunk-view command stall, and
  server/scheduler/runtime counters reported zero in the client summary.
- [x] Run RD10 fixed publish-budget sweep (`1/4/8/16/32`, temporary edits
  reverted): full-view improves from `27.492s` at budget `1` to `9.681s` at
  budget `4`, then stays flat through `8/16`; budget `32` does not improve
  full-view and produced two 60 Hz over-budget frames.
- [x] Run attribution probes at publish budget `4`: lighting off improves
  full-view only to `8.154s`, while render compile workers `2` cut render
  quiescence from `20.089s` to `11.412s`; workers `4` are flat.
- [x] Add raw/server ceiling split (`worldgen_perf --radius 11` plus
  `scheduler_loading_perf --render-distance 10`): raw cold features for the
  RD10 `529`-target footprint are `551.5` chunks/sec, scheduler-only with
  lighting off is `126.7` chunks/sec, and scheduler-only with lighting on is
  `40.6` chunks/sec. This proves the desktop-shaped `~50` chunks/sec result is
  not a hardware ceiling.
- [x] Add scheduler-only in-memory persisted reload mode: RD10 reload from
  `625` already-generated/lit in-memory records reaches view-ready in `0.523s`
  (`1012.1` chunks/sec) with `0` worldgen jobs and `0` light statuses.
- [x] Add scheduler-only temp SQLite persisted reload mode: RD10 reload through
  a temp world-dir SQLite store reaches view-ready in `0.572s` and settled in
  `0.721s` from `625` records, with `0` worldgen jobs and `0` light statuses.
- [x] Add mesh CPU-only lane: RD10 persisted SQLite reload reaches view-ready in
  `0.540s`, then full target-column mesh CPU build takes `11.037s` for `8464`
  sections (`2789` non-empty), with no GPU upload or frame loop.
- [x] Add GPU upload-only lane from prebuilt meshes: RD10 uploads `2789`
  non-empty sections (`360 MB`, `1.96M` faces) in `0.133s`; post-upload device
  poll is effectively zero on desktop.
- [x] Add desktop-shaped persisted-world startup-streaming lane so reload,
  mesh workers, upload budgets, and frame pacing are observed without fresh
  generation/light noise: RD10 persisted reopen reaches playable in `0.125s`,
  full target view in `1.031s`, and stable actionable render idle in `25.142s`
  with `0/2400` over-budget frames.
- [ ] Add worldgen/light mailbox busy-vs-idle time if Candidate A still needs
  mailbox utilization after the publication counters and queue samples are
  captured on full RD10/RD15 lanes.
- [ ] Harden the loading-settle idle predicate against the startup false-idle
  window (required before any idle-sensitive sweep reruns).
- [ ] Add the Meta dropped-frame before/after delta to the Quest perf summary.
- [ ] Capture clean-commit baselines: repeat RD10/RD15 startup-streaming from a
  clean tree, and recapture Quest RD5/RD7 settled orbit clean.
- [x] Fix remote-dedicated churn accounting before using remote play as the
  "server costs moved off headset" comparison: completed in
  [`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
  with schema v6 availability markers, Quest local-vs-remote contrast rows, and
  dedicated-server summaries.
- [ ] Implement Candidate A as a shared budget calculation and run the full
  promotion loop, including the Quest streaming check.

## Cost Questions To Resolve First

Before opening shared levers broadly, answer these with counters rather than
inference:

- Is desktop full-view readiness still dominated by scheduler publication after
  separating feature publish, light publish, update encode/send, and
  job-admission wait time? Current answer: yes for the coarse waterfall. RD10
  and RD15 both scale at about `19` target chunks/sec, with empty client update
  queues and max pending publication chunks near one `128`-target job. Raising
  the fixed drain to `4` moves RD10 full-view to `54.6` chunks/sec; raising it
  higher does not materially improve full-view.
- How much of `poll_scheduler_publish_completed_ms` is actual feature/light
  publication work versus update encode/send and downstream client apply work?
  Current answer: total publish time is small per frame (`1.6ms` max desktop),
  but elapsed wall time is capped by the fixed one-feature/one-light drain, not
  by client apply. Subphase timers are only needed if Candidate A exposes
  server tick spikes after raising the drain.
- On Quest streaming, does a larger publication drain remain invisible until
  downstream render work, or does it create measurable server runner spikes,
  update queue depth/age, client accept/store cost, dirty seed/prepare cost, or
  upload cost? Current answer before changing the drain: RD5 local integrated
  already shows update queue age (`167ms`), one update-pump stall, upload/accept
  backpressure, and server/scheduler tick maxima around `42ms` during churn.
  That does not forbid Candidate A, but it means Candidate A must be
  elapsed-budgeted and measured on this lane.
- Are the downstream client budgets actually smoothing 20 Hz server publication
  into 72 Hz render work, or do update bursts leak through as frame tails?
  Current answer: mostly smoothed at RD5 local integrated (`skipped_delta=0`,
  `app_work_p95=12.631ms`), but queue age and one `2x` frame show that bursts
  still leak into tail risk under churn.
- Does job-admission overlap make worldgen itself the next limit, or does
  pending-publication memory/backlog become the practical bound first?
  Current answer: raw RD10 cold feature generation is `551.5` chunks/sec, but
  current server-only loading is `126.7` chunks/sec with lighting disabled and
  `40.6` chunks/sec with lighting enabled. Candidate A should explain how much
  of that gap is job admission/publication versus unavoidable light/generation
  work.
- Does mesh worker count matter once publication is opened? Current answer:
  yes for desktop-shaped streaming tail, not for full-view readiness. Budget
  `4` plus workers `2` cuts RD10 quiescence from `20.089s` to `11.412s`, while
  workers `4` is flat.
- How much of already-generated startup is persistence/load/publication versus
  mesh/upload/render? Current answer: server-only reload and mesh CPU are now
  bounded. RD10 in-memory reload reaches view-ready in `0.523s`; temp SQLite
  reaches view-ready in `0.572s` and settles in `0.721s`, both from `625`
  already-lit records with no worldgen/light. Mesh CPU-only from persisted
  snapshots then takes `11.037s` for `8464` target sections. GPU upload-only
  from those prebuilt meshes takes `0.133s` for `360 MB`. The paced integrated
  persisted lane now shows server reload/publication and upload are not the
  remaining wall: measured RD10 reopen is playable in `0.125s`, full target
  view in `1.031s`, and stable actionable render idle in `25.142s` with total
  upload `0.170s`, total remesh `0.314s`, and total render `1.622s`. The wall
  is the paced render admission/mesh progression shape, not aggregate GPU
  upload bandwidth.

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
  can increase per-tick publish work — the Quest streaming check and the `133`
  update-pump counters (queue depth, oldest-applied age) are the instruments
  that would catch the cost.
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

- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) owns the
  shared frame-pipeline accounting model, validation guardrails, and follow-on
  gaps 9-10 for remote/dedicated contrast and elapsed/headroom-aware budgeting.
- [`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
  is the gap-9 successor: it owns the remote `SendOnly` fix and the
  remote-dedicated contrast rows this tactical's blocked checkbox needs.
- [`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md)
  is the gap-10 successor: it implements Candidate A/B as the shared budget
  calculation, re-pins the stale gate baselines first, and owns the Quest
  frame-shape (render-then-update) default decision.
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
