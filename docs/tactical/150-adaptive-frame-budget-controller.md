# 150: Adaptive Frame Budget Controller

Status: proposed; Slice 0 guardrail hygiene/baselines captured 2026-07-06 on
clean commit `bc55076c`. Drafted 2026-07-06 as the gap-10 follow-on to tactical
[`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md).
Law doc: [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
([Budget Controller Direction](../frame-pipeline-accounting.md#budget-controller-direction)
and gap 10). Strategy and measured candidate evidence:
[`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md).

Workstream: native Rust shared performance policy. Goal: replace the fixed
Quest-motivated counts that throttle every platform with one shared budget
calculation that spends **measured** elapsed time and headroom on terrain
generation, light, publication, render admission, and upload — so Quest keeps
clean pacing while desktop stops being artificially slow. Desktop and Quest
resolve to different numbers because their measured inputs differ, never
because a platform branch selects a profile.

This tactical is the policy inversion of 144: 144 changed only measurement,
never policy; 150 changes policy and is forbidden from growing new
measurement except through the diagnostics-detour rule below. The meter is
built and trusted (calibration, conservation invariants, overhead A/B) — use
it.

## Sequencing

- Gate: tactical
  [`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
  closes first (law-doc gap order 9 -> 10). Exception: Slice 0 (baselines)
  and Slice 2 (sans-I/O controller core, no wiring) may run while 149 is in
  flight, since neither changes engine behavior.
- One slice per session, in order. Between slices the user reviews.
- Candidates land one policy family at a time; never bundle Candidate A and
  B changes in one measured comparison.

## Fixed Reference Numbers

All from tactical 142 / 144 (Mac host `kmacbook`, Quest 3, seed `12345`
unless stated). These are the falsification anchors — if a Slice 0 recapture
disagrees materially, re-pin in Slice 0 and use the new numbers.

| Anchor | Value |
|---|---|
| Desktop RD10 fresh startup-streaming, publish budget 1 | clean Slice 0: playable `1.066-1.074s`, full-view `27.526-27.588s`, quiescent `56.651-56.746s`, `0/6000` over-budget |
| Desktop RD15 fresh startup-streaming, publish budget 1 | clean Slice 0: playable `1.196-1.267s`, full-view `57.214-57.255s`, quiescent `82.718-82.779s`; one row had `26/9000` over-budget and `2` over-2x frames |
| Same lane, fixed publish budget 4 (sweep knee) | full-view `9.68s`, 0 over-budget |
| Budget 4 + `--render-compile-workers 2` | quiescent `20.09s` -> `11.41s`; workers 4 flat |
| Desktop RD10 persisted reopen | clean Slice 0: playable `0.112-0.125s`, full-view `1.018-1.022s`, actionable idle `25.141-25.146s`, `0/2400` over-budget |
| Raw / server-only / server+light RD10 ceilings | `551.5` / `126.7` / `40.6` chunks/sec |
| Quest RD5 local churn (fixed budgets) | clean Slice 0: `skipped_delta=0`, app p95 `11.577-11.664ms`, queue age max `164.784-170.014ms`, 1 pump stall, pending publication `123-125` |
| Quest RD5 settled-orbit gate (re-pinned in 142) | `skipped_delta=0`, dropped delta `<= 17`, app p95 `<= 15.25ms`, headroom avg `>= +1.25ms`, over-period `<= 16%`, over-2x `0` |
| Quest RD5 shipping-default control | clean Slice 0, workers `1`, unbounded budgets: app p95 `14.920ms`, headroom avg `+1.251ms`, over-period `19.1%`, over-2x `0` |
| Quest RD7 settled-orbit pressure check (re-pinned in 142) | `skipped_delta=0`, dropped delta `<= 32`, app p95 `<= 17.5ms`, over-period `<= 40%`, over-2x `0` |

The RD5 discrepancy row is closed by Slice 0: the dirty 2026-07-04 gate numbers
no longer govern this tactical. Use the clean `bc55076c` envelopes above and in
142 for all later candidate gates.

## Java Reference Anchors (1.17.1)

Vanilla already runs measured elapsed-budget accounting in exactly the seams
this tactical opens. Per the reference-porting policy, read these sources
before implementing Slices 3-4; the table records what each mechanism does so
drift from the reference is a decision, not an accident.

| Mechanism | Reference | Shape |
|---|---|---|
| Adaptive compile-admission deadline | `client/renderer/LevelRenderer.java` `renderLevel` (~1044-1059; `frameTimes` field line 163) | deadline = frame-start nanos + `clamp(1.5 x trimmed mean of measured pre-compile frame elapsed, fps-cap period, 33.33ms)`; the mean comes from a 100-slot ring that drops min and max (`client/renderer/RunningTrimmedMean.java`) |
| Per-unit admission break | `client/renderer/LevelRenderer.java` `compileChunksUntil` (line 2105) | drains uploads first; per dirty chunk: player-caused dirty rebuilds synchronously, else async submit; breaks when remaining time `<` the mean per-unit cost measured within this call — overshoot is bounded to about one mispredicted unit |
| Bounded compile/upload resources | `client/renderer/chunk/ChunkRenderDispatcher.java` (`freeBuffers` pool, `runTask`, `uploadAllPendingUploads`) | compile tasks admitted only while a fixed `ChunkBufferBuilderPack` pool has free packs; the upload queue drains fully each frame because the pool bounds pending results; queue-depth debug counters (`pC/pU/aB`) |
| Server tick-slack task drain | `server/MinecraftServer.java` `runServer`/`waitUntilNextTick`/`haveTime` (line 735)/`pollTaskInternal` | fixed 50 ms tick; after the tick, `delayedTasksMaxNextTickTime = max(now + 50ms, nextTickTime)` and the loop drains chunk-source tasks under the `haveTime()` elapsed deadline; more than 2 s behind forgives whole ticks ("Can't keep up!") |
| Age-based starvation floor | `server/MinecraftServer.java` `shouldRun(TickTask)` | a queued tick task older than 3 ticks runs even when there is no slack |
| Elapsed + floor + backlog-pressure budget | `server/level/ChunkMap.java` `processUnloads` (line 391) | unload loop runs while `haveTime() OR processed < 200 OR backlog > 2000`; the unload queue drains while `haveTime() OR queue > 2000` |
| Light task batch cap | `server/level/ThreadedLevelLightEngine.java` (`taskPerBatch = 5`, line 32) | at most 5 pre/post light tasks per `runUpdate`, while the graph propagation inside the update drains unbounded |
| Frame-history accounting | `util/FrameTimer.java` | 240-slot frame-duration ring (the F3 graph) |

What this confirms for the design here:

- Candidate B's deadline admission and the "overshoot `<=` one unit"
  validation rule are literally `compileChunksUntil`'s loop invariant, and
  Candidate A's elapsed drain serviced from the gameplay tick is the
  `haveTime()` tick-slack shape. The floors/caps/backlog-pressure triple in
  the controller mirrors `processUnloads`. mclone's fixed `1`-per-tick
  publish budget is *tighter* than any vanilla mechanism — opening it toward
  a measured drain moves toward the reference shape, not away from it.
- Vanilla's estimators are a cross-frame 100-sample min/max-trimmed ring
  mean plus a within-call per-unit running mean. Slice 2 may choose EWMA
  instead; record the choice against these reference shapes.
- Vanilla's frame shape is update-then-render: admission spends its budget
  mid-frame, before the terrain layers draw. The Quest render-then-update
  default in Slice 1 is a frame-shape divergence justified by XR compositor
  pacing; record it as such.

What vanilla does **not** have — the recorded divergence this tactical makes:
no adaptive raise/lower of the clamps (all constants are fixed and assume
desktop-class headroom), no headroom percentiles or over-period tiers, no
queue-age tracking beyond the 3-tick task rule, no thermal input, and no
inbound-apply budget (the client applies every received packet; pacing lives
entirely at compile admission). Vanilla also hardwires the 50 ms tick — the
overload math divides by `50L` and its floors are tick-count-denominated
(the 3-tick task age) — while mclone's cadence is configurable per lane
(tactical 116, `cadence.rs`). The tick-then-slack-drain *shape* is
period-agnostic and ports cleanly; the constants do not: lane periods are
controller inputs, age-based floors are ms-denominated (the diagnostics
queue ages already are), and the cadence primitive's
`max_catch_up_host_frames` clamp with dropped excess is the existing analog
of vanilla's "Can't keep up" tick forgiveness. The controller adds adaptation because Quest
local-integrated play has no desktop-shaped slack for fixed constants to
hide in. The divergence is runtime orchestration, and it preserves the
vanilla seams — deadline admission, tick-slack drain, bounded pools,
synchronous player-edit rebuilds — so future parity work stays recoverable.
The budgeted update pump (tactical 133) is a previously recorded divergence
from vanilla's apply-everything client and stays as-is.

## Contract For Implementing Agents

Read this section, your slice, the law doc's
[Budget Controller Direction](../frame-pipeline-accounting.md#budget-controller-direction),
and 142's
[Budget Calculation Boundary](142-throughput-policy-with-quest-rd5-guardrail.md#budget-calculation-boundary)
before writing code. If your context is compressed mid-task, re-read this
section first.

1. **One shared budget calculation.** Floors, caps, ramp rates, and measured
   inputs may differ per lane; code paths may not. A `#[cfg]` or `is_quest`
   branch inside budget math is a defect — and so is a baked tick or frame
   period. The simulation cadence is configurable per lane
   (`SimulationCadenceConfig` host/gameplay/physics rates,
   `native/crates/mclone-server/src/cadence.rs`, tactical 116), so server-side
   periods enter as inputs from the cadence config and client-side periods
   from the display lane; `20 Hz`/`50 ms` never appears as a constant in
   budget math.
2. **Fail-safe floors.** Every controller output has a floor equal to
   today's shipped fixed default (`1` publish/tick, `1` light/tick, `1`
   chunk/sync-call, workers `1`, update pump `2ms/16`). Controller failure,
   cold start, or absent inputs must degrade to exactly today's behavior,
   never to zero work (starvation) and never to unbounded work.
3. **Sans-I/O controller core.** The budget math never reads clocks; inputs
   are report structs and samples, outputs are budget decisions. Same rule
   and same testing payoff as `mclone-diagnostics`. It compiles for
   `wasm32-unknown-unknown`.
4. **Every decision is traceable.** Each budget change carries an input
   snapshot and a reason code in the shared report schema (version bump).
   If a budget did something surprising, the trace must explain it without
   adding counters after the fact.
5. **Diagnostics-detour rule (anti-spin).** This tactical adds no new
   counters, stages, or lanes by default. A detour is allowed only when all
   three hold: (a) a named gate failed or a named slice decision cannot be
   made; (b) the slice section records which existing report fields were
   consulted and why they cannot answer it; (c) the consuming decision is
   named before the counter is built. At most one detour session per
   candidate. A second consecutive diagnostics-only session on the same
   candidate means **stop, record the blocker under Open Questions, and
   surface to the user** — do not keep instrumenting.
6. **Definition of progress.** Every session ends with one of: a baseline
   pinned, a candidate landed, a candidate falsified with evidence, or a
   blocker recorded with a named question. Two consecutive sessions on the
   same candidate producing none of those means the workstream is spinning:
   stop and escalate rather than starting another measurement pass.
7. **Gates are absolute and pinned** (142 rule). When an accepted change
   moves a baseline (e.g. Slice 1 flips the XR frame shape), re-pin the gate
   numbers in the same PR and say so.
8. **A/B discipline.** Same host per comparison set; candidates land behind
   a flag and are promoted to default only after the promotion loop; Quest
   numbers within `0.5ms` of a gate get one rerun (thermal); desktop deltas
   under 5% are noise — rerun before believing them.
9. **Do not** take adjacent workstreams: no thread scheduling/niceness
   (tactical 130), no CPU/GPU overlap internals (117/131 — Slice 1 only
   flips an already-validated default), no gameplay tick-rate change (116),
   no buffer/arena upload rework (128), no wgpu upgrades, no far-LOD budget
   integration (121), no XR HUD panels (148).

## Slice 0: Guardrail Hygiene And Clean Baselines

Why: three 142 prerequisites are still open, and the RD5 gate demonstrably no
longer matches the lane. Policy validation against stale or advisory gates
produces confident nonsense.

Deliverables (measurement-only; 144's invariant 1 applies to this slice):

- fix the Meta dropped-frame counter to a before/after delta per sample
  window (142 open item) so `dropped_frames` becomes a binding gate field
  instead of advisory;
- harden the loading-settle idle predicate against the startup false-idle
  window (142 open item: require loading progress to reach target or at
  least one nonzero pending observation before accepting idle);
- clean-commit baseline recapture, two runs each, host and device recorded:
  - desktop: `pnpm native:frame-budget:perf`,
    `pnpm native:startup-streaming:perf` (RD10),
    RD15 startup-streaming (9000 frames), `pnpm
    native:startup-streaming:persisted:perf`, movement-frame probe;
  - Quest, Mac-driven: RD5 and RD7 settled orbit metrics lanes in the
    **pinned lane config** (`--render-compile-workers 2`, accept/upload
    budgets `2/16/64` where the lane defines them) and RD5 once in
    **shipping default config**; RD5 local-integrated chunk-view churn;
- re-pin the 142 gate numbers from these rows (RD5 app-work p95 gate in
  particular) and update 142 in the same PR;
- add the Quest persisted-world guardrail row (142 named this the next
  needed lane; if the persisted lane cannot run on Quest yet, record the
  blocker instead of building new machinery — detour rule applies).

Exit criteria: all baseline rows recorded on a clean commit with spreads;
142 gates re-pinned; the two measurement fixes validated (dropped-frame
delta visible across two windows; idle predicate no longer false-idles at
host cadence 60).

2026-07-06 Slice 0 result:

- Measurement fixes landed in clean commit `bc55076c`: loading-settle now waits
  for the expected loading target before accepting runtime idle, and Quest
  PerfMetrics emits `dropped_frames_start/end/delta`.
- Desktop clean baselines are recorded in `docs/performance-records.md` from
  `/tmp/mclone-142-baselines/{frame-budget,startup,movement}*.json`.
- Quest clean baselines are recorded in
  `docs/quest-standalone-performance-records.md`: RD5/RD7 pinned orbit, RD5
  shipping-default control, RD5 local-integrated churn, and a periodic
  dropped-frame delta validation row.
- 142 gate numbers are re-pinned from those rows. RD5 is no longer judged
  against the stale 2026-07-04 envelope.
- Quest persisted-world guardrail is no longer structurally blocked:
  `pnpm native:android-xr:perf:orbit:rd5:persisted` now cleans an
  adb-visible Quest world dir, prewarms it, reopens the same `--world-dir`, and
  samples the existing RD5 settled-orbit metrics lane. No Quest numbers are
  recorded here until that command is run on-device.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
pnpm native:accounting:smoke
pnpm native:frame-budget:perf
pnpm native:startup-streaming:perf
pnpm native:android-xr:perf:orbit:rd5:metrics
pnpm native:android-xr:perf:orbit:rd7:metrics
git diff --check
```

## Slice 1: Quest Frame-Shape Default Decision (Render-Then-Update)

Why: the controller spends "measured headroom", and where headroom lives
depends on the frame shape. The opt-in XR frame-overlap path
(`--xr-frame-overlap`, optionally `--xr-overlap-runtime-prefetch`; evidence
in [`119`](119-android-xr-live-streaming-frame-pacing.md)) already
implements the law doc's target shape: submit eye work, defer part of the
wait, spend the slack on pose-independent runtime/render work. Deciding its
default **before** pinning controller behavior avoids re-baselining the
whole gate set twice.

Deliverables:

- A/B on hardware, same session set: RD5 orbit, RD7 orbit, RD5 churn — each
  default-shape vs `:frame-overlap` variant (lanes already exist for orbit;
  add churn args, not new machinery), in pinned lane config plus one RD5
  default-config control;
- decision, recorded here and in 117/119: make overlap (+prefetch if it is
  part of the win) the Android XR default, or keep per-eye-serial default
  with the measured reason;
- if flipped: re-pin the Slice 0 Quest gate numbers in the same PR; the
  overlap path becomes the shape all later candidate gates run against;
- desktop flat loop reordering (submit-early) is explicitly **not** this
  slice and not this tactical: desktop has measured headroom at 60/120 Hz,
  and the law doc already allows elapsed budgets without the reorder.
  Revisit only if a later candidate shows admission work delaying present
  on desktop — record the trigger, do not do it speculatively.

Exit criteria: decision recorded with rows; gates re-pinned if the default
changed; no open "maybe" state.

Validation: the four Quest lanes above plus
`pnpm native:android-xr:validate` launch health, `git diff --check`.

## Slice 2: Controller Core (Sans-I/O)

Why: the budget math must be provably correct before it owns real budgets —
same argument, same payoff as the 144 accounting core.

Deliverables:

- new leaf crate, working name `mclone-frame-budget`, depending only on
  `mclone-diagnostics` (report/summary types) and `serde`. If review prefers
  a module inside `mclone-diagnostics`, record the decision; the constraint
  that matters is sans-I/O, engine-free, one owner;
- inputs (per law doc): target frame/tick period and lane; recent app-work
  and headroom percentiles; recent misses/over-period/dropped deltas; queue
  depth and oldest-age; per-unit elapsed cost estimators (EWMA) for
  feature publish, light publish, result accept, section upload, admission
  scan; host mode (local-integrated vs remote, from 149's projection);
  floors/caps config;
- outputs: per-family budget decisions in elapsed terms with unit caps —
  publication drain budget (ms/tick + max units), job-admission backlog
  bound, render admission budget (ms/frame + max units), XR accept/upload
  budgets (ms/frame + max units) — plus a decision trace (inputs snapshot,
  reason code) for the report;
- control law: cut fast (one bad window: miss, negative headroom p05, or
  queue-age violation halves toward floor), raise slow (N consecutive clean
  windows add one increment toward cap), hysteresis so steady state does not
  limit-cycle, estimator reset on tick-period change (142 requirement),
  cold-start at floor;
- period-relative budget semantics: every budget is an elapsed slice of the
  configured lane period (gameplay tick period from the cadence config
  server-side, display period client-side), never a count that silently
  scales with the rate knob — the current fixed `1`/tick default is exactly
  such a count (its chunks/sec doubles if the gameplay rate doubles, per-tick
  cost unchanged). Under cadence catch-up, `advance_elapsed` can bunch
  several gameplay ticks into one host frame (bounded by
  `max_catch_up_host_frames`, excess dropped), so the drain budget is
  additionally bounded per host-frame wall window — bunched catch-up ticks
  must not multiply the burst;
- deterministic tests on synthetic histories: step response (injected spike
  cuts budget within a recorded frame count; recovery ramp within a recorded
  window count), starvation floor never violated, oscillation bound on
  steady input, granted-vs-spent conservation bookkeeping, tick-period
  reset, cold-start-at-floor;
- schema: budget-decision panel added to the shared report (version bump),
  rendered by logs and benchmark JSON (overlay adoption is 148's business).

Non-goals: no engine wiring; no consumer crate edits beyond the workspace
manifest.

Exit criteria: crate tests pass;
`cargo check -p mclone-frame-budget --target wasm32-unknown-unknown` passes;
step-response and floor semantics are asserted exactly, not eyeballed.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-frame-budget
cargo check --manifest-path native/Cargo.toml -p mclone-frame-budget --target wasm32-unknown-unknown
git diff --check
```

## Slice 3: Candidate A — Publication Drain Policy

Why: the measured primary desktop bottleneck. Fixed budgets `1`/`1` per
gameplay tick (`DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET`,
`DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET`,
`native/crates/mclone-server/src/scheduler.rs`) cap production near `20`
chunks/sec everywhere; the fixed-count sweep proved the win saturates near
budget `4` on desktop RD10 and that `32` adds tail risk — so the target is a
measured elapsed drain, not a bigger constant.

Reference shape (read before implementing): `MinecraftServer.runServer` /
`waitUntilNextTick` / `haveTime()` — the tick is mandatory, then chunk work
drains under an elapsed deadline; `ChunkMap.processUnloads` for the
elapsed-gate + unit-floor + backlog-pressure combination; `shouldRun(TickTask)`
for the age-based starvation floor.

Deliverables:

- `ChunkScheduler::poll()` publication serviced under a controller elapsed
  budget per gameplay tick (feature and light families), with per-unit cost
  fed back to the estimators; the fixed constants become the floors;
- job admission decoupled from full publication: mark the feature job
  pipeline-complete when the worldgen mailbox drains, track the
  pending-publication backlog separately, admit the next feature job while
  publication drains, and bound the backlog (no new job past N pending
  chunks — N is a controller cap, and RD20-long watches peak memory);
- scheduler tick/publish elapsed (`publish_completed_us`-shaped) recorded
  before/after so server-runner spikes are visible in existing counters;
- controller wiring behind a lane/config flag, default off until promotion.

Gates (inner loop every iteration; promotion before default-on):

- inner: desktop RD10 startup-streaming, movement-frame probe, Quest RD5
  orbit gate (re-pinned numbers);
- promotion: RD15 (9000 frames) plus second RD10 run, Quest RD7 pressure
  check, Quest RD5 churn streaming check, one native-window desktop run,
  desktop-XR functional smoke (below);
- acceptance: RD10 full-view `<= 11s` and quiescent improved, `0` 60 Hz
  over-budget frames, reproduced twice; RD15 full-view improvement
  `>= 1.9x` vs Slice 0 baseline; Quest RD5 gate green; RD7 not worse than
  its pinned envelope; churn: chunks published/sec `>=` Slice 0 churn
  baseline within noise, oldest-applied-update age and pump stalls not
  materially worse, pending-publication backlog respects its bound;
- the controller must reach the sweep-knee region (a drain worth roughly
  `4` publications/tick on this lane, measured on the default `20/20/60`
  cadence profile — at other gameplay rates the per-tick knee moves and the
  durable target is the elapsed/per-second form) **from measurement, not
  from a tuned desktop-specific constant** — assert by running the same
  controller config on the Quest churn lane and recording the different
  budget it converges to;
- cadence-compatibility probe, one-off and diagnostic-only (not a gate,
  since changing the gameplay rate changes simulation semantics and stays a
  non-lever per 116/142): rerun RD10 startup-streaming once under one
  non-default gameplay-rate profile accepted by the cadence config, and
  assert from the decision trace that per-tick drain time stays inside its
  budget slice and chunks/sec tracks measured cost rather than scaling
  linearly with tick rate. This exists to catch count-shaped budget bugs,
  not to tune cadence.

Exit criteria: acceptance rows recorded; default flipped on for desktop and
Quest local-integrated; 142's throttle-inventory rows for the two publish
budgets updated to point here.

## Slice 4: Candidate B — Render Admission And Workers

Why: after publication opens, the desktop trailing edge is the paced render
admission/mesh tail (`DEFAULT_RENDER_CHUNK_MESH_BUDGET = 1` chunk per sync
call, `native/crates/mclone-app-runtime/src/lib.rs`; workers and in-flight
cap in `native/crates/mclone-app-runtime/src/render_assets.rs`), measured as
admission/scan-bound, with workers `2` a real live-streaming quiescence
lever (`20.09s -> 11.41s`) despite being falsified for the synthetic bulk
drain.

Reference shape (read before implementing): `LevelRenderer.renderLevel`'s
deadline computation plus `compileChunksUntil`'s per-unit break, and
`ChunkRenderDispatcher`'s bounded builder pool — the Java Reference Anchors
section above has the mechanics.

Integration seam (inventoried 2026-07-06): the elapsed-shaped admission API
already exists —
`sync_render_sections_until_deadline_with_completed_result_acceptance_timed`
in `mclone-app-runtime`, already used by XR with a fixed one-frame-period
deadline from `XrMcloneTerrainState::render_compile_frame_deadline()`
(`native/crates/mclone-xr-scene/src/lib.rs`), and unused by the live desktop
path (`flat_client_driver.rs` calls the unbudgeted variant with `None`).
Upload/accept count caps are owned by `RenderSectionUploadFramePolicy` in
`native/crates/mclone-render-session/src/lib.rs`. Candidate B is therefore
mostly "make the controller supply the deadline/caps these seams already
accept", not new plumbing.

Deliverables:

- render admission serviced under a controller elapsed budget per
  frame/sync-call through the existing deadline API (unit caps in sections,
  floor = today's 1 chunk on desktop; on XR the controller replaces the
  fixed full-period deadline with a headroom-aware one), using the Slice
  5-of-144 admission sub-stage costs (dirty/ready scan, request build,
  worker submit, acceptance, upload apply) as estimator inputs;
- compile-worker count decision from measurement: desktop default `2` if
  the live win reproduces on the clean baseline, Quest default decided by
  its own gate set (lane configs already run workers `2` on RD5 — shipping
  default is `1`); record both decisions;
- XR accept/upload budgets (`--xr-render-completed-result-accept-budget`,
  `--xr-render-section-upload-budget`, `--xr-render-section-accept-budget`,
  currently `None` defaults with lane-arg candidate values `2/16/64`):
  decide whether the controller supplies them on Quest as outputs, replacing
  lane-arg policy — gated by RD5/RD7/churn like everything else;
- desktop persisted-world lane attribution: use the existing admission
  sub-stage split to say where the `25.1s` actionable-idle goes before and
  after.

Gates: same inner/promotion loop as Slice 3, plus the persisted lanes.
Acceptance: RD10 fresh quiescent `<= 14s`; persisted RD10 actionable idle
`<= 15s` **or** a recorded falsification with sub-stage attribution saying
which stage bounds it and why the controller cannot move it (that outcome
re-scopes the remainder to a named follow-up, it does not extend this
tactical); Quest gates green with queue ages bounded.

## Slice 5: Config Surface, Soak, And Close-Out

Deliverables:

- configuration surface: controller floors/caps/ramps exposed to benchmark
  lanes via CLI/lane args; shipping defaults derived per lane (refresh rate,
  tick period, host mode). A user-facing quality/aggressiveness preset is
  explicitly deferred unless a measured need appears — if added later it
  follows the tactical 147 shared-settings taxonomy
  (`ClientExperienceSettingsState` in
  `native/crates/mclone-app-runtime/src/client_experience.rs` plus the
  `GameUiRenderState`/`GameUiAction` round-trip), not an app-local knob;
- soak: one `>= 15 min` Quest local-integrated mixed run (orbit + churn
  phases) with controller on — no skipped-frame drift, no budget
  oscillation (decision-trace variance recorded), thermal note taken;
- contrasting-seed checkpoint (one biome-heavy or ocean-heavy seed) to catch
  budgets tuned to seed `12345`;
- desktop RD20 or RD30 long run watching pending-publication peak memory;
- docs closure: update 142 (candidate ladder states, throttle inventory,
  re-pinned gates), the law doc's gap 10 line, and
  [`../topics/performance.md`](../topics/performance.md) priority queue;
  durable rows to
  [`../performance-records.md`](../performance-records.md) /
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).

Exit criteria: close conditions below all green or explicitly falsified
with evidence; tactical closed.

## Validating The Budget Estimate

How we know the controller's budget is *right*, not just green on averages:

1. **Granted-vs-spent accounting.** Every budgeted phase reports granted
   budget, spent elapsed, and overshoot. Because work is admitted in units,
   overshoot p99 must stay `<=` one unit's estimated cost — this is
   vanilla's `compileChunksUntil` loop invariant, enforced as a report
   check. Violations are controller bugs (bad unit estimator or a
   non-preemptible unit that is too large).
2. **Estimator accuracy.** Predicted per-unit cost vs measured elapsed error
   percentiles live in the report. Provisional tolerance: p95 relative error
   `<= 25%` on steady lanes (adopt or revise in Slice 2 and record, the way
   144 adopted the `max(10%, 0.3ms)` calibration tolerance).
3. **Convergence against known ground truth.** The fixed-count sweep is the
   reference curve on desktop RD10: the controller must land at the knee
   (not the floor, not the `32`-style tail-risk zone) without lane-specific
   tuning, and must land somewhere *different and safe* on Quest churn.
4. **Step response on device.** Reuse the accounting-smoke busy-spin
   pattern: inject a known K-ms load window mid-run (headless flag), assert
   from the decision trace that budgets cut within the Slice 2 frame count
   and recover within the recorded ramp window. Deterministic twin lives in
   the Slice 2 unit tests; the device run is the integration check.
5. **The gates.** RD5/RD7/churn/desktop lanes stay green with the controller
   on, in default config, on a clean tree.

## Desktop XR Role

Numeric budget authority for XR is the Quest standalone lane set — desktop
XR (Mac WiVRn convenience lane, Windows VDXR/native runtime) inserts a PC
GPU and a streaming compositor, so its headroom numbers do not transfer to
Quest budgets. Its job here is **functional**: prove the XR-shaped frame
loop with the controller active reaches `FOCUSED`, submits N frames with `0`
skipped, and emits the budget-decision panel. Run `pnpm native:xr:check`
per-candidate (compile gate) and one live desktop-XR smoke at each
promotion (Mac WiVRn by default; Windows VDXR when a Windows session is
already scheduled — 144's checkpoint pattern). The 144 Windows confidence
pass caught a real stale-argument compile bug in the XR lane precisely
because it is exercised rarely: treat the promotion smoke as rot insurance,
not as a perf row.

## Non-Goals

- Desktop flat loop reordering (submit-early); recorded trigger in Slice 1.
- Thread scheduling/niceness/perf-levels (130), CPU/GPU overlap internals
  (117/131), upload buffer/arena lifetime (128), cadence semantics (116),
  far-LOD budget integration (121), web worker/job lifecycle, XR overlay
  panels (148).
- Making Quest RD7 settled-orbit clean (render/draw-cost bound; 117/119).
- Any new fixed global default as a "temporary" policy.

## Close Conditions

All measured on clean commits, default config, controller on:

- desktop RD10 fresh: full-view `<= 11s`, quiescent `<= 14s`, `0`
  over-budget frames, reproduced twice;
- desktop RD15: full-view `>= 1.9x` better than Slice 0 baseline;
- desktop RD10 persisted: actionable idle `<= 15s`, or falsified with
  attribution and a named follow-up;
- movement-frame probe: over-budget count not grown;
- Quest RD5 orbit gate green (Slice 0/1 re-pinned numbers); RD7 within its
  pinned envelope; RD5 churn publish rate `>=` baseline with queue
  ages/stalls not materially worse and backlog bounded;
- controller default-on for publication and render admission on desktop and
  Quest local-integrated; XR accept/upload budget decision recorded;
- estimator-accuracy and granted-vs-spent tolerances adopted and green.

When these are hit, close the tactical. Remaining ideas (upload pacing,
dynamic render distance, LOD budgets, remote-mode budget shaping) get their
own tacticals with their own evidence.

## Links

- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) —
  law doc: controller direction, stage model, validation rules.
- [`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
  — measured candidates, gates, benchmark loop, falsified levers.
- [`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md)
  — the meter this tactical steers by.
- [`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md)
  — host-mode input honesty (gap 9).
- [`119-android-xr-live-streaming-frame-pacing.md`](119-android-xr-live-streaming-frame-pacing.md)
  / [`117-android-xr-rd10-gpu-floor-and-frame-overlap.md`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md)
  — frame-overlap evidence and ownership for Slice 1.
- [`120-vanilla-render-compile-backpressure.md`](120-vanilla-render-compile-backpressure.md)
  — Java-shaped deadline admission history behind Candidate B.
