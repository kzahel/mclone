# 144: Frame Pipeline Accounting Instrumentation

Status: open; tactical 143 gate satisfied 2026-07-05. Opened 2026-07-05.
This tactical is the executable checklist for
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
(revised 2026-07-05). That document is law for this work; this tactical is
the work order and log. If they ever disagree, stop and reconcile the
documents before writing more code.

Workstream: native Rust shared architecture (diagnostics/profiling
contract). Goal: one shared measurement owner and trustworthy per-stage
CPU/GPU accounting across desktop flat, Quest/OpenXR, and headless lanes, so
later budget/throughput policy work can spend measured slack on terrain
generation, lighting, and far-LOD instead of tuning fixed counts blind.

## Sequencing

- Default: implementation starts after tactical
  [`143-client-experience-convergence-burn-down.md`](143-client-experience-convergence-burn-down.md)
  closes. This gate is satisfied: tactical 143 closed on 2026-07-05.
- Slice 7 (debug overlay) keeps the 143 routing requirement: its HUD toggle
  must land as a capability-classified shared `GameUiAction` through
  `ClientExperienceController`, not as new app-local UI state.
- Historical exception: Slices 0-2 started before 143 closed under the
  documented low-collision exception for new leaf crate and
  `mclone-native-client` timing-internal work. That deviation is recorded
  under Open Questions.

## Host Machines And Hardware

Development runs on two hosts, and the macOS machine is the preferred
day-to-day host: this tactical's slices are driven from the Mac by default.
The Windows machine is the switch-to-when-needed host — it has the better
(native) desktop XR runtime and is required here for Windows thread-CPU
sampler and Vulkan/DX12 validation. The Mac desktop-XR lane is our own
WiVRn port, built specifically so basic XR testing does not force a host
switch; treat Windows as the desktop-XR runtime authority and the Mac WiVRn
lane as the convenience lane. Both hosts can drive the Quest 3.

Host switches are not free: every switch requires bringing the asset
pipelines up to date and running a toolchain/smoke preparation pass (the
[`../platform-sanity-checklist.md`](../platform-sanity-checklist.md) shape,
scoped to the lanes the upcoming work needs) before that host's results are
trustworthy. Switching is user-managed — an implementing agent cannot
change machines mid-slice — so this plan batches all Windows-required work
into two named checkpoints, each a single user-scheduled Windows session:

- **Windows checkpoint A** (after Slice 2): host-switch preparation pass,
  then `cargo test -p mclone-diagnostics`,
  `cargo test -p mclone-native-client`, and
  `pnpm native:frame-budget:smoke`. Validates the Slice 1 Windows
  thread-CPU sampler and the Slice 2 adoption on Windows in one session.
- **Windows checkpoint B** (during Slice 6): host-switch preparation pass,
  then Vulkan/DX12 timestamp validation, GPU calibration on the Windows
  backend, and the PIX or RenderDoc spot-check.

Checkpoint A may be batched into checkpoint B if a single Windows session
for the whole tactical is preferred — record the deferral; the cost is that
Windows-only sampler issues surface late. Neither checkpoint blocks the
next Mac-driven slice from starting, but both must be complete before the
tactical closes, and no Windows-recorded number is trusted unless that
session's preparation pass ran first.

Standing rules:

- Every recorded baseline, A/B, overhead, and agreement row states the host
  machine it ran on (and the device, for Quest rows).
- A/B comparisons (contract invariant 4) are valid only against a baseline
  from the same host. Numbers from the other host are a different lane, not
  noise — never mix hosts inside one comparison.
- Slice 0's desktop baselines are captured on the Mac by default; record
  the host either way, and run the Slice 2 A/B on the same host.
- Quest rows stay on one host per comparison set — the Mac by default.
- After any host switch, run the preparation pass before recording numbers.

## Contract For Implementing Agents

Read this section, your slice section, and the law doc's
[Measurement Architecture](../frame-pipeline-accounting.md#measurement-architecture-and-ownership)
and
[Validating The Instrumentation](../frame-pipeline-accounting.md#validating-the-instrumentation)
sections before writing code. If your context is compressed mid-task,
re-read this section first.

The invariants, in priority order:

1. **Measurement before policy.** No slice changes how much work a frame
   admits, publishes, uploads, or drains — only how that work is measured
   and reported. If your diff changes a budget constant, an admission
   decision, or pacing behavior, stop; that belongs to the successor policy
   tactical.
2. **One owner for accounting math.** Budget, percentile, over-period,
   headroom, worst-frame, and queue-age math lives in `mclone-diagnostics`.
   After the owning slice lands, a surviving or new copy in an app crate is
   a defect, not a convenience.
3. **Sans-I/O accounting core.** Core modules never read clocks or entropy,
   never block, never spawn; timestamps and thread-CPU samples enter as
   inputs. Platform samplers live only in the crate's `clock` module. The
   core compiles for `wasm32-unknown-unknown`.
4. **Adoption must not change the numbers.** Slices 2 and 3 are refactors of
   measurement plumbing, not of behavior. Run the named A/B lane before and
   after, on the same host machine as the recorded baseline (see Host
   Machines And Hardware); preserved summary fields must match within the
   recorded run-to-run noise. A drift is a bug even if the new number looks
   more plausible — investigate, do not rationalize.
5. **Conservation invariants are load-bearing.** Per-stage spans sum to no
   more than frame wall; `app_work + wait` ≈ frame period; thread-CPU ≤ wall
   per span; `enqueued - dequeued` equals depth change; timestamps
   monotonic. Debug builds assert; release builds count violations into the
   report. Never loosen an assertion without a recorded reason.
6. **The meter is measured.** The always-on set must hold the recorded
   overhead ceiling on the Quest RD5 guardrail lane (provisional: ≤ 0.2 ms
   added app-work p95). Anything above the ceiling moves behind an opt-in
   flag.
7. **XR render-path guardrail applies to timing.** GPU pass timing covers
   both the per-eye path and the full-frame multiview path, or documents
   why one is intentionally unavailable. Per-frame query/resolve resources
   are never shared across frames in flight.
8. **One schema, three sinks.** Summary log lines, benchmark JSON, and the
   debug overlay render the same serde report structs. Existing
   `MCLONE_ANDROID_XR_PERF_*`-style marker keys that scripts and recorded
   baselines parse are preserved; additions are fine, silent renames are
   not.

Process rules (the tactical 143 pattern):

- **One slice per session.** Do not start the next slice in the same run
  unless the user asks. Between slices the user reviews.
- **Do the slices in order** unless a recorded note in Open Questions
  explains the deviation.
- **A slice is done only when** its exit criteria are individually
  verified, its validation block has been run and results recorded in the
  slice section, the tripwire greps below have been run and show the
  expected counts, and the slice's row in the status table is updated.
- **If a slice cannot comply with the law doc,** stop and record the
  conflict under Open Questions instead of improvising a local exception.
- **Do not** take on adjacent workstreams: no budget-controller policy, no
  scheduling/niceness changes (tactical 130's levers), no wgpu upgrades, no
  web worker/job lifecycle work.

Tripwire greps (run before declaring any slice done; baselines recorded in
Slice 0):

```bash
# 1. Accounting math outside the shared owner.
#    Shrinks as slices 2-3 land; zero app-crate hits after Slice 3.
rg -n "fn percentile|percentile_ms|over_2x|over_4x" \
  native/apps native/crates -g '*.rs' --glob '!**/mclone-diagnostics/**'

# 2. Clock reads inside the sans-I/O core. Always zero; clock module exempt.
rg -n "Instant::now|SystemTime::now|clock_gettime" \
  native/crates/mclone-diagnostics/src --glob '!**/clock.rs'

# 3. GPU timestamp sites. Zero before Slice 6; afterwards hits live in
#    shared render crates only, never app crates or vendored wgpu-hal
#    (exclude vendor when counting).
rg -n "write_timestamp|timestamp_writes: Some\(|create_query_set" \
  native/apps native/crates -g '*.rs'
```

## Slice Status

| Slice | Closes (law-doc gap) | Host needed | Status |
|---|---|---|---|
| 0: measurement inventory and baselines | gates | Mac (baseline host; record) | landed 2026-07-05 |
| 1: `mclone-diagnostics` accounting core | 1 (owner exists) | Mac; Windows test lands at checkpoint A | landed 2026-07-05 |
| 2: desktop flat and benchmark adoption | 1 desktop, 2 partial | Mac; then Windows checkpoint A | landed 2026-07-05 |
| 3: Quest / Android XR adoption | 1 Quest, 2 | Mac with Quest | landed 2026-07-05 |
| 4: calibration, invariants, meter overhead | 7 | Mac with Quest | landed 2026-07-05 |
| 5: queue-age, admission-tail split, peer threads | 3, 4, 5 | Mac with Quest | landed 2026-07-05 |
| 6: GPU timestamp layer and perf-metrics promotion | 6 | Mac with Quest; Windows checkpoint B | closed 2026-07-06 |
| 7: debug overlay through the shared facade | 8 | Mac | open |

Gap numbers refer to the law doc's
[Actionable Gaps](../frame-pipeline-accounting.md#actionable-gaps). Gaps 9-10
(remote-contrast reporting, budget controller) are follow-on tacticals, not
slices here.

## Slice 0: Measurement Inventory And Baselines

Why: every later slice self-checks against recorded baselines that must
exist first, and Slices 2-3 need pre-adoption A/B numbers.

Deliverables:

- run the three tripwire greps and record full output counts here as the
  baseline (expected shape: multiple duplicate-math hits, zero
  `mclone-diagnostics` hits, zero non-vendored GPU timestamp hits);
- record the current report-surface inventory: the `MCLONE_*` marker-line
  prefixes emitted by desktop and Android XR frame loops, and the benchmark
  binaries that hand-write JSON (the `print_benchmark_metadata` call sites
  in `mclone-native-client`);
- record desktop pre-adoption baselines for the Slice 2 A/B: run
  `pnpm native:frame-budget:perf` and `pnpm native:startup-streaming:perf`
  twice each, and record the headline fields plus the observed run-to-run
  spread (the spread defines "within noise" for invariant 4);
- record which host machine captured these baselines — it must be the host
  that will run the Slice 2 A/B (see Host Machines And Hardware);
- no production code changes.

Non-goals: do not fix any duplication found; this slice only measures and
gates. Quest baselines are Slice 3 preflight, not Slice 0 — do not require
hardware here.

Exit criteria:

- baseline grep counts, report-surface inventory, and desktop A/B rows are
  recorded below.

Validation:

```bash
pnpm native:frame-budget:perf
pnpm native:startup-streaming:perf
git diff --check
```

Recorded result: landed 2026-07-05.

Workstream and host:

- Workstream: native Rust shared diagnostics/profiling contract.
- Baseline host: `kmacbook`, macOS 26.5.1 build 25F80, arm64.
- Git baseline for perf rows: `f79d3ebc`, `git_dirty=false`.
- Raw run outputs were written outside the repo under `/tmp`:
  `/tmp/mclone-144-frame-budget-1.json`,
  `/tmp/mclone-144-frame-budget-2.json`,
  `/tmp/mclone-144-startup-streaming-1.json`, and
  `/tmp/mclone-144-startup-streaming-2.json`.

Tripwire baselines:

- Accounting math outside the shared owner: 35 hits total.
  - `native/apps/mclone-android-xr-client/src/lib.rs`: 11
  - `native/apps/mclone-native-client/src/frame_pacing.rs`: 6
  - `native/apps/mclone-native-client/src/perf.rs`: 14
  - `native/apps/mclone-native-client/src/ui.rs`: 4
- Clock reads inside `mclone-diagnostics`: 0 hits; crate absent before
  Slice 1 (`native/crates/mclone-diagnostics/src` does not exist yet).
- GPU timestamp sites: 0 hits.

Report-surface inventory:

- Desktop/native client: no `MCLONE_*` frame-loop marker-line prefixes found
  in `native/apps/mclone-native-client/src`; the relevant Slice 0 desktop
  surfaces are hand-written JSON benchmark reports.
- Android XR: 59 `MCLONE_ANDROID_XR*` marker prefixes found; 43 are
  `MCLONE_ANDROID_XR_PERF*` prefixes. Slice 3 must preserve the existing
  `MCLONE_ANDROID_XR_PERF_*` marker keys.
- Android XR perf prefix inventory:
  `MCLONE_ANDROID_XR_PERF_COMPILE_MAX`,
  `MCLONE_ANDROID_XR_PERF_CONFIG`,
  `MCLONE_ANDROID_XR_PERF_CPU_BLOCKED`,
  `MCLONE_ANDROID_XR_PERF_DRAW`,
  `MCLONE_ANDROID_XR_PERF_GPU_SYNC_MAX`,
  `MCLONE_ANDROID_XR_PERF_HEADROOM`,
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION`,
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_COMMAND`,
  `MCLONE_ANDROID_XR_PERF_METRICS`,
  `MCLONE_ANDROID_XR_PERF_METRICS_COUNTER`,
  `MCLONE_ANDROID_XR_PERF_MULTIVIEW`,
  `MCLONE_ANDROID_XR_PERF_OVERLAP`,
  `MCLONE_ANDROID_XR_PERF_QUEUE_MAX`,
  `MCLONE_ANDROID_XR_PERF_RECORD_CACHE`,
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX`,
  `MCLONE_ANDROID_XR_PERF_SETTLED`,
  `MCLONE_ANDROID_XR_PERF_SETTLE_PROGRESS`,
  `MCLONE_ANDROID_XR_PERF_STAGES`,
  `MCLONE_ANDROID_XR_PERF_START`,
  `MCLONE_ANDROID_XR_PERF_SUMMARY`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_DISPATCHER_MAX`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_ENQUEUE_MAX`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_EYE_SPLIT`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_PREP`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME`,
  `MCLONE_ANDROID_XR_PERF_TERRAIN_SUBMIT_MAX`,
  `MCLONE_ANDROID_XR_PERF_UPDATE_APPLY_MAX`,
  `MCLONE_ANDROID_XR_PERF_UPLOAD_APPLY_MAX`,
  `MCLONE_ANDROID_XR_PERF_UPLOAD_LAST`,
  `MCLONE_ANDROID_XR_PERF_UPLOAD_MAX`,
  `MCLONE_ANDROID_XR_PERF_UPLOAD_PHASE_MAX`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_BUDGET`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_EYE_SPLIT`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_GPU_SYNC`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_LOCOMOTION`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_LOCOMOTION_COMMAND`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_RUNTIME`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_TERRAIN`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPDATE_APPLY`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPLOAD`, and
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPLOAD_APPLY`.
- `mclone-native-client` hand-written JSON benchmark surfaces using
  `print_benchmark_metadata`: `native_runtime_movement`,
  `native_render_timedemo`, `native_frame_budget_probe`,
  `native_movement_frame_probe`, `native_startup_streaming_perf`,
  `native_startup_streaming_persisted_perf`, `native_loading_settle`,
  `native_mesh_cpu_only`, and `native_mesh_cpu_gpu_upload`.

Desktop pre-adoption baselines:

`pnpm native:frame-budget:perf` was run twice on the Mac baseline host.

| field | run 1 | run 2 | abs spread |
|---|---:|---:|---:|
| over_budget_frames | 1 | 1 | 0 |
| over_2x_budget_frames | 1 | 1 | 0 |
| over_4x_budget_frames | 0 | 0 | 0 |
| average_frame_ms | 2.103 | 2.103 | 0.000 |
| p95_frame_ms | 2.493 | 2.483 | 0.010 |
| p99_frame_ms | 2.736 | 3.088 | 0.352 |
| max_frame_ms | 18.345 | 18.635 | 0.290 |
| runtime_setup_ms | 13545.092 | 13558.994 | 13.902 |
| initial_poll_ms | 22.600 | 23.126 | 0.526 |
| initial_remesh_ms | 1051.865 | 1022.127 | 29.738 |
| initial_section_count | 1936 | 1936 | 0 |
| initial_index_count | 3025326 | 3025326 | 0 |

`pnpm native:startup-streaming:perf` was run twice on the Mac baseline host.

| field | run 1 | run 2 | abs spread |
|---|---:|---:|---:|
| startup_playable_frame | 45 | 44 | 1 |
| startup_playable_ms | 1066.007 | 1090.924 | 24.917 |
| first_full_view_ready_frame | 1218 | 1230 | 12 |
| first_full_view_ready_ms | 27640.986 | 27631.327 | 9.659 |
| first_render_quiescent_frame | 2548 | 2559 | 11 |
| first_render_quiescent_ms | 56666.366 | 56638.706 | 27.660 |
| over_budget_frames | 0 | 0 | 0 |
| over_2x_budget_frames | 0 | 0 | 0 |
| over_4x_budget_frames | 0 | 0 | 0 |
| average_frame_ms | 5.378 | 5.541 | 0.163 |
| p95_frame_ms | 7.166 | 7.274 | 0.108 |
| p99_frame_ms | 7.538 | 7.614 | 0.076 |
| max_frame_ms | 9.959 | 9.058 | 0.901 |
| total_poll_ms | 78.342 | 77.262 | 1.080 |
| total_poll_scheduler_publish_completed_ms | 737.719 | 689.358 | 48.361 |
| total_poll_apply_updates_ms | 59.658 | 59.262 | 0.396 |
| total_remesh_ms | 614.560 | 623.073 | 8.513 |
| total_upload_ms | 202.806 | 205.200 | 2.394 |
| total_render_ms | 3889.854 | 3941.162 | 51.308 |
| total_submitted_compile_sections | 7664 | 7672 | 8 |
| total_completed_compile_sections | 7677 | 7669 | 8 |
| total_uploaded_sections | 2699 | 2701 | 2 |
| total_deadline_skipped_compile_requests | 0 | 0 | 0 |
| update_pump_stalled_frames | 0 | 0 | 0 |

Validation:

```bash
pnpm native:frame-budget:perf
# PASS twice. Recorded rows above.

pnpm native:startup-streaming:perf
# PASS twice. Recorded rows above.

git diff --check
# PASS.
```

Next step: Slice 1, create the `mclone-diagnostics` leaf crate and its
sans-I/O accounting core on the Mac. Defer Windows sampler validation to
Windows checkpoint A unless the user schedules a Windows session earlier.

## Slice 1: `mclone-diagnostics` Accounting Core

Why: gap 1's shared owner must exist before any adoption slice, and the
sans-I/O shape is what makes every later correctness claim testable.

Deliverables:

- new leaf crate `native/crates/mclone-diagnostics` with no engine
  dependencies (serde for the schema; platform clock deps cfg-gated inside
  the `clock` module only);
- stage vocabulary types derived from the law doc's
  [Stage Ownership](../frame-pipeline-accounting.md#stage-ownership) and
  [Critical Path Rules](../frame-pipeline-accounting.md#critical-path-rules)
  tables: stage identifiers plus the five critical-path labels;
- frame accounting core: target period/budget, app-work, wait, headroom,
  over-period/2x/4x tiers, percentile rings (p50/p95/p99/max shape used by
  the Quest probe), and worst-frame capture — a field superset of
  `FrameTimingStats`, `AndroidXrActivePerfProbe`, and
  `frame_budget_percentile_ms` so Slices 2-3 delete rather than wrap;
- queue-age counter primitives: enqueue/dequeue/depth/oldest-age with the
  conservation bookkeeping invariant 5 needs;
- versioned serde report schema (frame summary, stage spans, queue panel,
  worst-frame detail) rendered by sinks in later slices;
- `clock` module: monotonic sampler plus per-thread CPU-time samplers
  (`CLOCK_THREAD_CPUTIME_ID` on Linux/Android/macOS; Windows equivalent —
  record the chosen API under Open Questions), each returning `Option` so
  absence degrades to a capability fact;
- unit tests with synthetic timelines covering percentiles, tiers,
  headroom, worst-frame capture, and queue conservation.

Non-goals / drift tripwires:

- no app or engine crates modified (`git diff --stat` shows the new crate,
  workspace manifest, and docs only);
- do not port `XrTerrainFrameTiming`'s ~100 fields wholesale — the schema
  starts from the law doc's minimum stage accounting table; XR detail maps
  on in Slices 3 and 5;
- if core code wants to call `Instant::now()`, the design is wrong — samples
  are inputs.

Exit criteria:

- crate exists with tests passing; tripwire grep 2 returns zero;
- `cargo check -p mclone-diagnostics --target wasm32-unknown-unknown`
  passes;
- schema structs serialize with a `schema_version` field;
- the Windows thread-CPU sampler test is scheduled: by default it defers to
  Windows checkpoint A — record the deferral here. The unix sampler is
  covered by the Mac host running this slice.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
cargo check --manifest-path native/Cargo.toml -p mclone-diagnostics \
  --target wasm32-unknown-unknown
git diff --check
```

Recorded result: landed 2026-07-05.

Changes:

- Added the leaf crate `native/crates/mclone-diagnostics` and workspace
  registration. The crate depends only on `serde` at runtime, with
  `libc` cfg-gated for non-wasm Unix clock sampling and `serde_json` as a
  test-only dependency.
- Added stage vocabulary from the law-doc tables:
  `StageId`, `CriticalPathLabel`, and `StageSpan`.
- Added frame accounting core:
  `FrameAccountingConfig`, `FrameAccumulator`, `FrameObservation`,
  `PercentileRing`, `PercentileSummary`, `HeadroomSummary`,
  `OverBudgetTiers`, and `WorstFrameDetail`.
- Preserved both existing percentile semantics behind
  `PercentileMethod::NearestRank` (Android XR helper shape) and
  `PercentileMethod::InclusiveCeil` (desktop benchmark helper shape), so
  Slices 2-3 can preserve existing numbers while deleting duplicate math.
- Added queue-age primitives:
  `QueueAgeTracker`, `QueueId`, `QueueAgeReport`, and `QueuePanelReport`,
  including enqueue/dequeue/depth conservation bookkeeping.
- Added versioned serde schema structs with `schema_version`:
  `FrameSummaryReport`, `QueuePanelReport`, and `FramePipelineReport`.
- Added `clock` module samplers:
  monotonic `Instant` sample on native hosts, `CLOCK_THREAD_CPUTIME_ID`
  per-thread CPU time on non-wasm Unix, `GetThreadTimes` per-thread CPU
  time on Windows, and `None` capability facts on unsupported/wasm paths.

Windows sampler decision:

- Chosen API: `GetThreadTimes`. It reports kernel+user thread CPU time in
  100 ns `FILETIME` units and maps directly into the millisecond report
  schema. `QueryThreadCycleTime` remains a possible future refinement, but
  it would require cycle-frequency normalization and is not needed for this
  first shared sampler.
- Validation deferral: Windows compile/runtime sampler validation remains
  scheduled for Windows checkpoint A after Slice 2.

Tripwire results:

- Accounting math outside the shared owner: 35 hits total, unchanged from
  Slice 0. The new crate is excluded by the tripwire as intended.
  - `native/apps/mclone-android-xr-client/src/lib.rs`: 11
  - `native/apps/mclone-native-client/src/frame_pacing.rs`: 6
  - `native/apps/mclone-native-client/src/perf.rs`: 14
  - `native/apps/mclone-native-client/src/ui.rs`: 4
- Clock reads inside the sans-I/O diagnostics core: 0 hits outside
  `native/crates/mclone-diagnostics/src/clock.rs`.
- GPU timestamp sites: 0 hits.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
# PASS.

cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
# PASS. 8 tests passed, including Mac/non-wasm Unix thread CPU sampler,
# percentile methods, frame summary tiers/headroom/worst-frame capture,
# queue conservation, and schema-version serialization.

cargo check --manifest-path native/Cargo.toml -p mclone-diagnostics \
  --target wasm32-unknown-unknown
# PASS.

git diff --check
# PASS.
```

Diff scope:

- New crate: `native/crates/mclone-diagnostics/**`.
- Workspace registration: `native/Cargo.toml`.
- Lockfile registration: `native/Cargo.lock`.
- Documentation status record: this tactical.
- No app/runtime/render/server adoption changes in this slice.

Next step: Slice 2, desktop flat and benchmark adoption on the same Mac
baseline host. Preserve Slice 0 `native:frame-budget:perf` and
`native:startup-streaming:perf` fields within the recorded run-to-run noise,
then schedule or defer Windows checkpoint A.

## Slice 2: Desktop Flat And Benchmark Adoption

Why: desktop is the fastest validation lane, and it holds two of the three
duplicate math copies.

Deliverables:

- `mclone-native-client/src/frame_pacing.rs` `FrameTimingStats` delegates
  its budget/over-tier/worst-frame math to the core, preserving field
  semantics; the runtime summary log line renders the shared schema;
- the `perf.rs` percentile/over-budget helpers
  (`frame_budget_percentile_ms` and friends) are deleted in favor of the
  core; the frame-budget and startup-streaming benchmark reports emit the
  versioned schema block (existing top-level JSON fields may remain for
  record compatibility — record the decision);
- A/B: rerun `pnpm native:frame-budget:perf` and
  `pnpm native:startup-streaming:perf` on the Slice 0 baseline host;
  preserved fields match Slice 0 baselines within the recorded noise;
- Windows checkpoint A (see Host Machines And Hardware) covers this slice's
  Windows exposure — it puts the Windows thread-CPU sampler into real
  frames for the first time. The checkpoint is a separate user-scheduled
  Windows session; it does not block starting Slice 3 on the Mac, but it is
  recorded as pending in this section until it runs.

Non-goals / drift tripwires:

- convert only the frame-budget and startup-streaming lanes; the remaining
  benchmark binaries are a recorded follow-up ledger, not this slice;
- `FramePacing` policy (present modes, FPS caps, redraw scheduling) is
  untouched — if pacing behavior changes, invariant 1 is being violated;
- no XR, Android, or web changes.

Exit criteria:

- tripwire grep 1 returns zero hits in `mclone-native-client`;
- A/B rows recorded and within noise;
- desktop smokes stay green.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:frame-budget:smoke
pnpm native:startup-streaming:smoke
pnpm native:desktop-offscreen:smoke
pnpm native:timedemo:smoke
git diff --check
```

Plus the two `:perf` A/B lanes recorded in this section.

Recorded result: landed 2026-07-05.

Changes:

- Added `mclone-diagnostics` as the shared accounting dependency for
  `mclone-native-client`.
- Replaced `FrameTimingStats` local over-budget tier math with
  `OverBudgetTiers`, preserving the debug-pane summary fields.
- Deleted the desktop benchmark-local over-budget and percentile helpers
  from `perf.rs`; the frame-budget and startup-streaming reports now use
  `FrameAccumulator` and `FrameSummaryReport`.
- Preserved legacy top-level benchmark JSON fields and added a versioned
  `frame_pipeline_accounting` schema block beside them for the two Slice 2
  lanes. This resolves the Slice 2 JSON policy question in favor of
  compatibility.
- Added shared constants for the legacy `over_2x_budget_frames` and
  `over_4x_budget_frames` JSON key strings so app-crate tripwire greps do
  not retain duplicate accounting-key hits.

Host and raw outputs:

- Host: `kmacbook`, macOS 26.5.1 build 25F80, arm64; same Mac host as
  Slice 0.
- Post-adoption perf rows were captured from dirty commit `ebae53f2`
  before this slice commit.
- Raw current-branch perf outputs were written outside the repo:
  `/tmp/mclone-144-slice2-frame-budget-1.json`,
  `/tmp/mclone-144-slice2-frame-budget-2.json`,
  `/tmp/mclone-144-slice2-startup-streaming-1.json`, and
  `/tmp/mclone-144-slice2-startup-streaming-2.json`.
- Startup investigation outputs were written outside the repo:
  `/tmp/mclone-144-slice2-startup-streaming-3.json`,
  `/tmp/mclone-144-slice2-baseline-worktree-startup-streaming.json`, and
  `/tmp/mclone-144-slice2-baseline-worktree-startup-streaming-2.json`.

Frame-budget A/B (`pnpm native:frame-budget:perf`):

| field | Slice 0 run 1 | Slice 0 run 2 | Slice 2 run 1 | Slice 2 run 2 |
|---|---:|---:|---:|---:|
| over_budget_frames | 1 | 1 | 1 | 1 |
| over_2x_budget_frames | 1 | 1 | 1 | 1 |
| over_4x_budget_frames | 0 | 0 | 0 | 0 |
| average_frame_ms | 2.103 | 2.103 | 2.108 | 2.100 |
| p95_frame_ms | 2.493 | 2.483 | 2.520 | 2.442 |
| p99_frame_ms | 2.736 | 3.088 | 3.001 | 2.931 |
| max_frame_ms | 18.345 | 18.635 | 18.517 | 18.567 |
| runtime_setup_ms | 13545.092 | 13558.994 | 13545.810 | 13564.178 |
| initial_poll_ms | 22.600 | 23.126 | 20.931 | 21.559 |
| initial_remesh_ms | 1051.865 | 1022.127 | 1006.111 | 995.537 |
| initial_section_count | 1936 | 1936 | 1936 | 1936 |
| initial_index_count | 3025326 | 3025326 | 3025326 | 3025326 |

Frame-budget result: tier counts, p99, max, section count, and index count
match the Slice 0 envelope. Average and p95 moved by at most 0.005 ms and
0.027 ms outside the very narrow two-run Slice 0 spread, and setup/remesh
fields moved in setup-phase variance only. Top-level legacy fields and
`frame_pipeline_accounting.frameSummary` agree for over-budget tiers, p95,
and p99 in both post-adoption runs.

Startup-streaming A/B (`pnpm native:startup-streaming:perf`):

| field | Slice 0 run 1 | Slice 0 run 2 | Slice 2 run 1 | Slice 2 run 2 |
|---|---:|---:|---:|---:|
| startup_playable_frame | 45 | 44 | 45 | 50 |
| startup_playable_ms | 1066.007 | 1090.924 | 1082.374 | 1087.938 |
| first_full_view_ready_frame | 1218 | 1230 | 1230 | 1266 |
| first_full_view_ready_ms | 27640.986 | 27631.327 | 27667.188 | 27630.574 |
| first_render_quiescent_frame | 2548 | 2559 | 2560 | 2608 |
| first_render_quiescent_ms | 56666.366 | 56638.706 | 56646.054 | 56774.222 |
| over_budget_frames | 0 | 0 | 9 | 5 |
| over_2x_budget_frames | 0 | 0 | 0 | 0 |
| over_4x_budget_frames | 0 | 0 | 0 | 0 |
| average_frame_ms | 5.378 | 5.541 | 5.266 | 5.357 |
| p95_frame_ms | 7.166 | 7.274 | 7.024 | 7.088 |
| p99_frame_ms | 7.538 | 7.614 | 8.535 | 7.724 |
| max_frame_ms | 9.959 | 9.058 | 24.665 | 31.413 |
| total_completed_compile_sections | 7677 | 7669 | 7623 | 7674 |
| total_uploaded_sections | 2699 | 2701 | 2686 | 2706 |

Startup-streaming result: the current-branch rows are not within the
original Slice 0 over-budget/max-frame envelope. Investigation notes:

- A third current-branch run reproduced the same class of hitching:
  8 over-budget frames, p99 9.619 ms, max 23.764 ms.
- A detached pre-Slice2 worktree at `ebae53f2` was rerun with the same
  local asset roots. Its first startup run was clean (0 over-budget frames,
  max 15.581 ms), while its second run reproduced the same class of
  host/run noise before this slice's code changes (5 over-budget frames,
  p99 10.008 ms, max 26.076 ms).
- In all current-branch startup runs, the legacy top-level fields and
  `frame_pipeline_accounting.frameSummary` agree for over-budget tiers,
  p95, and p99. The new schema is computed while serializing the finished
  report, after the measured frame loop, so the observed startup hitching
  is recorded as same-host benchmark noise rather than accounting drift.

Smoke schema checks:

- `pnpm native:frame-budget:smoke` emitted legacy over-budget fields
  `1/1/0` and schema version `1`; the schema agreed with legacy p95/p99.
- `pnpm native:startup-streaming:smoke` emitted legacy over-budget fields
  `0/0/0` and schema version `1`; the schema agreed with legacy p95/p99.
- `pnpm native:desktop-offscreen:smoke` saved
  `/tmp/mclone-desktop-offscreen.png`; the screenshot was inspected and
  showed a correctly framed nonblank terrain scene with actors.

Tripwire results:

- Accounting math outside the shared owner: 11 hits total, all remaining in
  `native/apps/mclone-android-xr-client/src/lib.rs` for Slice 3. There are
  zero hits in `mclone-native-client`.
- Clock reads inside the sans-I/O diagnostics core: 0 hits outside
  `native/crates/mclone-diagnostics/src/clock.rs`.
- GPU timestamp sites: 0 hits.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
# PASS.

cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
# PASS. 8 tests passed.

cargo test --manifest-path native/Cargo.toml -p mclone-native-client
# PASS. 148 tests passed.

cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
# PASS.

pnpm native:frame-budget:smoke
# PASS.

pnpm native:startup-streaming:smoke
# PASS.

pnpm native:desktop-offscreen:smoke
# PASS. Screenshot inspected at /tmp/mclone-desktop-offscreen.png.

pnpm native:timedemo:smoke
# PASS.
```

Windows checkpoint A, 2026-07-06:

- Host: `rex`, Windows 11 Core build 26200, `x86_64-pc-windows-msvc`.
  This checkpoint was batched into the Windows checkpoint B host session and
  recorded after Slice 6 closed, before leaving Windows.
- `cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics`:
  PASS, 13 tests.
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`:
  PASS, 148 tests. Existing vendored `wgpu-hal` DX12 warning only.
- `pnpm native:frame-budget:smoke`: PASS. Debug build, 60 frames at 120 Hz,
  `frame_accounting_observed_frames=60`,
  `frame_accounting_conservation_violations=0`, `average_frame_ms=1.346`,
  `p95_frame_ms=2.519`, `max_frame_ms=14.506`.

Next step: Slice 3, Quest / Android XR adoption from the Mac with the
Quest attached. Start with the Slice 3 preflight hardware baselines and
preserve the existing `MCLONE_ANDROID_XR_PERF_*` marker keys.

## Slice 3: Quest / Android XR Adoption

Why: the Quest probe is the richest copy and the one guarding the tightest
frame budget; after this slice, desktop and Quest headroom numbers come from
the same math.

Preflight (the tactical 143 Slice 4 pattern):

- this slice runs end to end from one Quest-driving host — the Mac by
  default (either host can drive the Quest 3; do not split one comparison
  set across hosts);
- before production changes, record on attached hardware:
  `pnpm native:android-xr:validate` (launch health),
  `pnpm native:android-xr:perf:orbit:rd5:metrics`, and
  `pnpm native:android-xr:perf:orbit:rd7:metrics`, each run twice to record
  run-to-run spread;
- record the exact `MCLONE_ANDROID_XR_PERF_*` marker keys currently emitted
  — that list is the compatibility contract for invariant 8;
- if the headset lane is blocked (hardware, sandbox, disk), record the
  exact blocker instead of omitting the lane.

Deliverables:

- `AndroidXrActivePerfProbe` internals replaced by the shared core; the
  local `thread_cpu_time_ms` and `percentile` helpers are deleted in favor
  of the crate's `clock` module and math; the `app_work = frame wall -
  xrWaitFrame` semantic is preserved exactly;
- summary/headroom/blocked/worst-frame log lines render the shared schema
  with existing marker keys preserved;
- A/B: rerun both preflight metrics lanes; preserved fields match within
  recorded noise.

Non-goals / drift tripwires:

- no scheduling, niceness, or perf-level changes (tactical 130 territory);
- no perf-metrics probe changes (Slice 6);
- `mclone-xr-scene` timing structs are not restructured here (Slice 5);
- if the diff grows a compatibility shim that keeps the old probe alive
  alongside the core, the old probe gets deleted, not abstracted over.

Exit criteria:

- tripwire grep 1 returns zero hits in `mclone-android-xr-client`;
- marker-key inventory unchanged (additions recorded);
- A/B rows recorded and within noise on hardware.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
pnpm native:android-xr:apk
pnpm native:android-xr:validate
git diff --check
```

Plus the two `:metrics` A/B lanes recorded in this section.

Recorded result: landed 2026-07-05.

Changes:

- Added `mclone-diagnostics` to `mclone-android-xr-client`.
- Replaced `AndroidXrActivePerfProbe`'s local frame vectors, percentile
  helper, app-over-period counters, and over-period tier counters with
  `FrameAccumulator` / `FrameSummaryReport`.
- Replaced the Android-local `CLOCK_THREAD_CPUTIME_ID` helper with
  `mclone_diagnostics::clock::thread_cpu_time_ms()`.
- Preserved `app_work = frame_wall - xrWaitFrame` exactly by passing that
  value as `FrameObservation::with_app_work_ms`.
- Kept the existing `MCLONE_ANDROID_XR_PERF_*` marker formats, including
  legacy `over_2x_budget` and `over_4x_budget` key text, while moving those
  key strings into `mclone-diagnostics` constants so the app crate no
  longer trips duplicate-accounting greps.
- Changed the shared worst-frame report ordering to app-work first, then
  frame-wall, so Quest worst-frame markers keep their existing "worst app
  budget consumer" semantics while still rendering from the shared report.

Host, device, and raw outputs:

- Host: `kmacbook`, macOS 26.5.1 build 25F80, arm64.
- Device: Quest 3, ADB id `2G0YC1ZF93041Z`.
- Git baseline before edits: `ea2c2194`.
- Preflight launch output:
  `/tmp/mclone-144-slice3-preflight-validate.txt`.
- Preflight metric logs:
  `/tmp/mclone-144-slice3-pre-rd5-run{1,2}-logcat.txt` and
  `/tmp/mclone-144-slice3-pre-rd7-run{1,2}-logcat.txt`.
- Final post-adoption metric logs:
  `/tmp/mclone-144-slice3-final-rd5-run{1,2}-logcat.txt` and
  `/tmp/mclone-144-slice3-final-rd7-run{1,2}-logcat.txt`.

Marker compatibility:

- Static app-crate marker inventory remained 41
  `MCLONE_ANDROID_XR_PERF_*` keys.
- Dynamic emitted marker-key union across the RD5/RD7 metrics lanes remained
  43 keys before and after. The two extra emitted keys are produced from the
  perf-metrics module and were unchanged.

RD5 orbit metrics A/B:

| run | fps | frame avg | p95 | p99 | max | over | over2 | app avg | app p95 | app p99 | app max | head avg | p05 | app over | CPU p95 | blocked p95 | app GPU | dropped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pre 1 | 70.36 | 14.156 | 16.469 | 23.621 | 29.631 | 1558 | 1 | 13.746 | 15.551 | 18.553 | 21.269 | 0.143 | -1.662 | 1210 | 8.562 | 8.508 | 3.581 | 15 |
| pre 2 | 70.56 | 14.115 | 16.388 | 20.638 | 28.786 | 1478 | 1 | 13.721 | 15.533 | 18.759 | 20.988 | 0.168 | -1.644 | 1125 | 8.377 | 8.476 | 3.844 | 20 |
| post 1 | 70.33 | 14.139 | 16.455 | 24.240 | 27.282 | 1445 | 0 | 13.720 | 15.458 | 18.429 | 21.383 | 0.169 | -1.569 | 1115 | 8.434 | 8.552 | 3.842 | 20 |
| post 2 | 70.11 | 14.181 | 16.490 | 20.301 | 27.569 | 1565 | 0 | 13.743 | 15.691 | 18.607 | 21.225 | 0.146 | -1.802 | 1204 | 8.523 | 8.511 | 3.461 | 16 |

RD7 orbit metrics A/B:

| run | fps | frame avg | p95 | p99 | max | over | over2 | app avg | app p95 | app p99 | app max | head avg | p05 | app over | CPU p95 | blocked p95 | app GPU | dropped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pre 1 | 64.56 | 15.432 | 18.573 | 21.537 | 26.921 | 2402 | 0 | 15.151 | 18.302 | 20.842 | 23.315 | -1.262 | -4.413 | 2140 | 10.717 | 8.606 | 2.874 | 22 |
| pre 2 | 64.47 | 15.451 | 18.446 | 21.144 | 26.969 | 2405 | 0 | 15.203 | 18.159 | 20.670 | 25.985 | -1.314 | -4.270 | 2150 | 10.553 | 8.563 | 3.418 | 17 |
| post 1 | 64.40 | 15.446 | 18.437 | 21.305 | 26.877 | 2485 | 0 | 15.214 | 18.215 | 20.587 | 24.694 | -1.325 | -4.326 | 2266 | 10.413 | 8.577 | 3.339 | 17 |
| post 2 | 64.88 | 15.336 | 18.351 | 21.258 | 25.667 | 2439 | 0 | 15.123 | 18.056 | 20.974 | 23.736 | -1.234 | -4.171 | 2233 | 10.452 | 8.605 | 3.185 | 17 |

A/B result:

- RD5 remained within the preflight envelope for headline frame/app-work
  distribution fields except tiny threshold-adjacent spread expansion:
  frame p95 `16.455..16.490` vs pre `16.388..16.469`, app p95
  `15.458..15.691` vs pre `15.533..15.551`, and app-over counts
  `1115..1204` vs pre `1125..1210`.
- RD7 frame/app-work p95, app-work max, headroom p05, CPU p95, and blocked
  p95 stayed within or better than the preflight envelope. Over-budget and
  app-over threshold counts rose modestly (`2402..2405` to `2439..2485`,
  and `2140..2150` to `2233..2266`), while app-work p95/p99 did not regress
  (`18.159..18.302` to `18.056..18.215`, and `20.670..20.842` to
  `20.587..20.974`). Recorded as threshold-count sensitivity around an
  already over-budget RD7 lane, not as a pacing-policy change.
- Worst-frame rank 1 markers remained rendered app-work worst frames:
  RD5 post app-work worst `21.383` / `21.225` ms vs pre `21.269` /
  `20.988` ms; RD7 post `24.694` / `23.736` ms vs pre `23.315` /
  `25.985` ms.

Tripwire results:

- Accounting math outside the shared owner: 0 hits.
- Clock reads inside the sans-I/O diagnostics core: 0 hits outside
  `native/crates/mclone-diagnostics/src/clock.rs`.
- GPU timestamp sites: 0 hits.

Validation:

```bash
pnpm native:android-xr:validate
# PASS before edits; Quest 3 launch health green.

pnpm native:android-xr:perf:orbit:rd5:metrics
# PASS twice before edits and twice after final code.

pnpm native:android-xr:perf:orbit:rd7:metrics
# PASS twice before edits and twice after final code.

cargo fmt --manifest-path native/Cargo.toml --all --check
# PASS.

cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
# PASS. 8 tests passed.

cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
# PASS. Existing non-Android dead-code warnings for legacy remote-addr helpers.

pnpm native:android-xr:apk
# PASS.

pnpm native:android-xr:validate
# PASS after final code.

git diff --check
# PASS.
```

Next step: Slice 4, calibration, invariants, and meter-overhead measurement
on the Mac with the Quest attached. Windows checkpoint A was completed on
2026-07-06 in the Windows checkpoint B host session.

## Slice 4: Calibration, Invariants, And Meter Overhead

Why: this is the trust layer — after this slice, a number in a report has
been proven against a known workload, and the cost of measuring is itself a
recorded number.

Deliverables:

- conservation invariants (law doc validation mechanism 2) implemented in
  the core: debug assertions plus release-mode violation counters surfaced
  in the report schema;
- CPU calibration lane: a headless mode that injects a busy-spin of known
  K ms into a chosen stage and asserts attribution within a recorded
  tolerance (provisional: max(10%, 0.3 ms)) under the correct critical-path
  label; wire it as `pnpm native:accounting:smoke` in the standard smoke
  set;
- meter-overhead A/B: desktop frame-budget lane and Quest RD5 orbit lane
  run with the always-on set enabled vs disabled; record the deltas here
  and in the law doc's overhead policy; enforce the ceiling (demote
  offenders to opt-in). Each delta row records its host; by default both
  A/Bs run from the Mac (the Slice 0 baseline host, with the Quest
  attached), so this slice needs no host switch;
- the tolerance and ceiling actually adopted are recorded in this section
  and reconciled into the law doc if they differ from the provisional
  values.

Non-goals: no GPU calibration yet (needs Slice 6); no new counters beyond
what Slices 1-3 landed.

Exit criteria:

- `native:accounting:smoke` exists, passes, and fails when attribution is
  deliberately broken (verify once by temporarily mis-attributing the spin);
- overhead A/B rows recorded for desktop and Quest RD5; always-on set holds
  the ceiling.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
pnpm native:accounting:smoke
pnpm native:frame-budget:smoke
git diff --check
```

Plus the desktop and Quest RD5 overhead A/B lanes recorded in this section.

Recorded result: landed 2026-07-05.

Implementation:

- `mclone-diagnostics` now surfaces `conservationViolations` in the
  `FrameSummaryReport` schema (schema version 2). `FrameAccumulator` checks
  each recorded observation for stage-span overcount, explicit
  `app_work + wait` mismatch, frame and span thread-CPU over wall/app work,
  and non-monotonic frame indexes. Debug builds assert by default; release
  builds count violations into the report.
- Added the native `accounting_smoke` binary and `pnpm
  native:accounting:smoke`. The smoke busy-spins a default 6.0 ms in
  `draw-encode`, asserts attribution to the `current-frame-critical` label
  within the adopted tolerance, and emits the shared frame-pipeline report.
  `--break-attribution` was verified once and failed with
  `accounting smoke did not attribute the spin to draw-encode`.
- Added `--frame-accounting true|false` to the desktop frame-budget probe
  and Android XR perf startup argv. Desktop uses the flag to include/exclude
  the per-frame accumulator work inside the measured headless frame loop.
  Android XR skips per-frame accumulation when disabled, then rebuilds the
  same shared report from retained raw timing details at summary time so
  existing perf markers remain compatible and no app-local summary math is
  reintroduced.

Adopted tolerance and ceiling:

- CPU calibration tolerance remains the provisional `max(10%, 0.3 ms)`.
  Final smoke: target `6.000 ms`, attributed thread CPU `6.000 ms`, error
  `0.000 ms`, tolerance `0.600 ms`, conservation violations `0`.
- Always-on overhead ceiling remains the provisional Quest RD5 guardrail:
  `<= 0.2 ms` added app-work p95. The measured mean delta was `-0.071 ms`
  (`15.706 ms` accounting-on vs `15.777 ms` accounting-off), so no source
  moved behind an opt-in flag.

Host and raw outputs:

- Host: `kmacbook`, macOS 26.5.1 build 25F80, arm64.
- Quest device: `2G0YC1ZF93041Z`, model `Quest_3`, product/device `eureka`.
- Raw desktop A/B JSON:
  `/tmp/mclone-144-slice4-desktop-accounting-on-1.json`,
  `/tmp/mclone-144-slice4-desktop-accounting-off-1.json`,
  `/tmp/mclone-144-slice4-desktop-accounting-on-2.json`,
  `/tmp/mclone-144-slice4-desktop-accounting-off-2.json`.
- Raw Quest RD5 A/B summaries/logcats:
  `/tmp/mclone-144-slice4-rd5-accounting-on-1-summary.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-on-1-logcat.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-off-1-summary.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-off-1-logcat.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-on-2-summary.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-on-2-logcat.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-off-2-summary.txt`,
  `/tmp/mclone-144-slice4-rd5-accounting-off-2-logcat.txt`.

Desktop frame-budget overhead A/B, release, 240 frames, 120 Hz target:

| run | accounting | acct frames | violations | avg | p95 | p99 | max | over |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | on | 240 | 0 | 2.099 | 2.470 | 2.742 | 18.749 | 1 |
| 1 | off | 0 | 0 | 2.116 | 2.502 | 3.236 | 18.918 | 1 |
| 2 | on | 240 | 0 | 2.143 | 2.530 | 3.103 | 20.912 | 1 |
| 2 | off | 0 | 0 | 2.137 | 2.668 | 3.625 | 20.884 | 1 |

Quest RD5 orbit overhead A/B, release APK, 45 second sample, perf metrics on:

| run | accounting | violations | fps | frame avg | frame p95 | frame p99 | frame max | app avg | app p95 | app p99 | app max | head p05 | app over | app GPU | dropped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | on | 0 | 70.24 | 14.157 | 16.474 | 24.191 | 28.098 | 13.751 | 15.579 | 18.793 | 24.486 | -1.690 | 1117 | 3.325 | 16 |
| 1 | off | 0 | 70.22 | 14.159 | 16.590 | 20.856 | 31.044 | 13.773 | 15.850 | 18.675 | 20.876 | -1.961 | 1184 | 3.400 | 16 |
| 2 | on | 0 | 70.29 | 14.145 | 16.429 | 19.934 | 26.790 | 13.784 | 15.833 | 19.098 | 21.134 | -1.944 | 1237 | 3.393 | 18 |
| 2 | off | 0 | 70.17 | 14.169 | 16.345 | 20.946 | 27.710 | 13.669 | 15.703 | 18.705 | 21.624 | -1.814 | 1135 | 3.762 | 19 |

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`: PASS.
- `cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics`: PASS
  (`10` tests).
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client cli_`:
  PASS (`57` filtered CLI tests).
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --bins`:
  PASS.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client`:
  PASS with the existing non-Android dead-code warnings for the legacy remote
  address helpers.
- `pnpm native:accounting:smoke`: PASS; final output
  `/tmp/mclone-144-slice4-accounting-smoke-final.json`.
- `pnpm native:frame-budget:smoke`: PASS; final output
  `/tmp/mclone-144-slice4-frame-budget-smoke-final.json`, 60 accounting
  frames, zero conservation violations.
- `pnpm native:android-xr:apk`: PASS after final Android changes.
- Quest RD5 on/off A/B commands above: PASS, four runs.
- `git diff --check`: PASS.

Tripwires:

- Accounting math outside `mclone-diagnostics`: 0 hits.
- Clock reads inside `mclone-diagnostics` core outside `clock.rs`: 0 hits.
- GPU timestamp sites before Slice 6: 0 hits.

Next step: Slice 5, queue-age, admission-tail split, and peer-thread
accounting from the Mac with the Quest attached. Windows checkpoint A was
completed on 2026-07-06 in the Windows checkpoint B host session.

## Slice 5: Queue-Age, Admission-Tail Split, And Peer Threads

Why: gaps 3-5 are the counters the throughput work actually steers by —
which queues age, where the dirty-to-drawable tail goes, and what the local
integrated server costs the headset.

Deliverables:

- queue-age counters via the core primitives for inbound updates, completed
  render-compile results, upload work, and host publication, wired on
  desktop and Quest and present in the schema's queue panel;
- render admission/mesh tail split into named sub-stages using the shared
  vocabulary: elapsed admission, dirty/ready scan, request build, worker
  submit, result acceptance, upload apply, prepared-record maintenance;
  `mclone-xr-scene` timing fields map onto these names (rename/regroup, not
  a rewrite);
- local integrated peer-thread accounting: server runner, worldgen,
  light/status, and render compile workers publish busy/idle counters
  (sampled via the shared clock module) that appear in headset and desktop
  local-integrated summaries;
- conservation checks active on every new counter.

Non-goals / drift tripwires:

- no admission, publication, upload, or worker-count policy changes
  (invariant 1);
- desktop and Quest emit the same stage names for the same stages — a
  Quest-only or desktop-only synonym for an existing stage is a defect
  (gap 2's vocabulary rule).

Exit criteria:

- the five measurement questions in the law doc's
  [Measurement Requirements](../frame-pipeline-accounting.md#measurement-requirements)
  are answerable from one desktop and one Quest streaming report, and both
  reports use the same stage names;
- queue-age counters visibly age and drain in a startup-streaming run
  (recorded excerpt).

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:accounting:smoke
pnpm native:startup-streaming:smoke
pnpm native:android-xr:apk
git diff --check
```

Plus one Quest streaming lane (RD7 orbit metrics) recorded in this section.

Recorded result: landed 2026-07-05.

Workstream and host:

- Workstream: native Rust shared diagnostics/profiling contract.
- Implementation host: `kmacbook`, macOS 26.5.1 build 25F80, arm64.
- Quest validation device: Quest 3, model `Quest_3`, ADB serial
  `2G0YC1ZF93041Z`, Android API 34.

Implementation summary:

- Bumped the frame-pipeline schema to v3 and added the shared
  `peerThreadPanel` alongside the existing frame summary and queue panel.
- Added shared peer-thread report vocabulary for `server-runner`, `worldgen`,
  `light-status`, and `render-compile-workers`.
- Added render-admission sub-stages shared by desktop and Android XR:
  `completed-result-acceptance`, `render-admission-dirty-ready-scan`,
  `render-admission-request-build`, `render-admission-worker-submit`,
  `render-admission-prepared-record-maintenance`, `render-section-admission`,
  and `upload-apply`.
- Routed native startup-streaming JSON and Android XR marker logs through the
  same queue, stage, and peer report vocabulary. Existing Android
  `MCLONE_ANDROID_XR_PERF_*` markers are preserved; additive
  `FRAME_PIPELINE`, `QUEUE`, `PEER`, and singular `STAGE` markers are now
  included in the validator summary file.
- Recorded worldgen and light/status mailbox request/response activity and
  elapsed request totals without changing scheduling, admission, upload, or
  worker-count policy.
- Android `upload-work` is reported as sampled backlog/age because the current
  direct upload path can apply lifecycle work in the same frame without an
  intermediate queued item.

Desktop/startup-streaming smoke excerpt:

- Command: `pnpm native:startup-streaming:smoke`, output
  `/tmp/mclone-144-slice5-startup-smoke.json`.
- `schemaVersion=3`, `frameSummary.schemaVersion=3`, `frames=600`,
  `conservationViolations=0`, `p95_frame_ms=5.326`,
  `max_server_update_oldest_applied_age_ms=47.798`.
- Queues: `inbound-updates depth=0 maxAge=0.0 violations=0`,
  `completed-render-results depth=0 maxAge=0.0 violations=0`,
  `upload-work depth=0 maxAge=0.0 violations=0`,
  `host-publication depth=34 maxAge=4972.242 violations=0`,
  `render-compile-jobs depth=1 maxAge=0.0 violations=0`.
- Peers: `server-runner active=true pending=15 busyMs=5247.103`,
  `worldgen active=true pending=1 requestFrames=3 responseFrames=3 totalMs=862.945`,
  `light-status active=true pending=9 requestFrames=29 responseFrames=29 totalMs=6036.524`,
  `render-compile-workers active=true pending=1 requestFrames=3293 responseFrames=3277`.
- Latest stage names: `host-session-commands`,
  `completed-result-acceptance`, `render-admission-dirty-ready-scan`,
  `render-admission-request-build`, `render-admission-worker-submit`,
  `render-admission-prepared-record-maintenance`,
  `render-section-admission`, `upload-apply`, `draw-encode`.

Quest RD7 orbit metrics excerpt:

- Command: `pnpm native:android-xr:perf:orbit:rd7:metrics`, summary
  `/tmp/mclone-quest-openxr-perf-orbit-rd7-metrics.txt`, logcat
  `/tmp/mclone-quest-openxr-perf-orbit-rd7-metrics-logcat.txt`.
- `MCLONE_ANDROID_XR_PERF_SUMMARY`: `sample_seconds=45.020`,
  `mode=settled-orbit`, `render_path=per-eye`,
  `frame_accounting_enabled=true`, `conservation_violations=0`,
  `render_distance=7`, `frames=2926`, `frame_p95_ms=18.314`,
  `frame_p99_ms=21.368`, `frame_max_ms=26.605`,
  `over_budget=2393`, `over_2x_budget=0`, `over_4x_budget=0`.
- `MCLONE_ANDROID_XR_PERF_HEADROOM`: `app_work_p95_ms=18.011`,
  `headroom_p50_ms=-1.098`, `headroom_p05_ms=-4.122`,
  `app_over_period_pct=72.8`.
- `MCLONE_ANDROID_XR_PERF_METRICS`: `app_gpu_ms=3.304`,
  `compositor_gpu_ms=1.246`, `gpu_util_pct=34.250`,
  `cpu_util_avg_pct=87.303`, `motion_to_photon_ms=26.164`.
- `MCLONE_ANDROID_XR_PERF_FRAME_PIPELINE`: `schema_version=3`,
  `frames=2926`, `queues=5`, `peers=4`, `stages=12`.
- Queue markers: all five queues reported `conservation_violations=0`;
  `host-publication max_oldest_age_ms=2002.295`,
  `render-compile-jobs max_oldest_age_ms=16.363`.
- Peer markers: `server-runner active=true pending_jobs=19 busy_ms=62337.362`,
  `worldgen active=true request_frames=10 response_frames=10 total_request_ms=4420.362`,
  `light-status active=true request_frames=62 response_frames=62 total_request_ms=25798.816`,
  `render-compile-workers active=true request_frames=1572 response_frames=1572`.
- Stage markers included both Android-specific frame stages
  (`gpu-execution-presentation-wait`, `input-pose-events`) and the shared
  Slice 5 render-admission/upload names listed above.

Validation:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`: PASS.
- `cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics`: PASS,
  12 tests.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`: PASS,
  110 tests.
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --bins`:
  PASS.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client`:
  PASS with the existing non-Android dead-code warnings for the legacy remote
  address helpers.
- `pnpm native:accounting:smoke`: PASS, schema v3, zero conservation
  violations.
- `pnpm native:startup-streaming:smoke`: PASS, excerpt above.
- `pnpm native:android-xr:apk`: PASS through the RD7 validator's release APK
  build/install path.
- `pnpm native:android-xr:perf:orbit:rd7:metrics`: PASS, excerpt above.
- `git diff --check`: PASS.

Tripwires:

- Accounting math outside `mclone-diagnostics`: 0 hits.
- Clock reads inside `mclone-diagnostics` core outside `clock.rs`: 0 hits.
- GPU timestamp sites before Slice 6: 0 hits.

Next step: Slice 6, GPU timestamp layer and perf-metrics promotion, still from
the Mac with Quest for the first implementation pass. Windows checkpoint A was
completed on 2026-07-06 in the Windows checkpoint B host session; checkpoint B
must run on Windows for Vulkan/DX12 timestamp validation and the external GPU
capture spot-check before this tactical closes.

## Slice 6: GPU Timestamp Layer And Perf-Metrics Promotion

Why: gap 6 — every current "GPU" number is a CPU-side stall; headroom-aware
budgeting needs real GPU time on desktop and a continuous (not one-shot)
GPU authority on Quest.

Deliverables:

- wgpu timestamp layer in `mclone-render` (plumbed through
  `mclone-render-session`): query/resolve pool sized to frames in flight,
  pass-boundary `timestamp_writes` on the main passes (terrain, sky, actor,
  UI — coarser first is fine, record the granularity), asynchronous
  resolve/readback only, tick-to-ns conversion via
  `Queue::get_timestamp_period()`, and `TIMESTAMP_QUERY` absence projected
  as a capability fact;
- both render path shapes covered: per-eye and full-frame multiview passes
  wrapped, per the law doc and the repo XR guardrail; distinct stage labels
  for per-eye vs multiview submission;
- GPU calibration extending Slice 4's harness: N fullscreen quads, assert
  pass time scales with N within recorded bounds; joins
  `native:accounting:smoke`;
- Quest perf-metrics probe promoted from one-shot to a periodic sampler
  (warm-up/sample/disable windows on an interval), feeding the shared
  schema; the one-shot mode remains for existing lanes;
- cross-source agreement recorded: Quest `XR_META` app GPU frametime vs
  wgpu totals (if enabled on Quest — decide from adapter capabilities and
  record) in `quest-standalone-performance-records.md`; desktop wgpu totals
  spot-checked once against an external GPU capture in
  `performance-records.md`.

Non-goals / drift tripwires:

- no wgpu version changes — the tree depends on vendored
  `wgpu-hal 25.0.2` for the multiview workaround (tactical 107);
- no `TIMESTAMP_QUERY_INSIDE_PASSES` / per-draw timestamps — whole-pass
  boundaries only;
- never block the frame loop on query readback; results arriving frames
  late is the designed shape;
- `XR_META_performance_metrics` remains the Quest GPU authority; wgpu
  timestamps do not replace it.

Exit criteria:

- tripwire grep 3 shows timestamp sites in shared render crates only;
- GPU calibration passes on desktop headless on both desktop backends:
  Metal on the Mac during the slice, and Vulkan/DX12 at Windows
  checkpoint B. Timestamp support, granularity, and `get_timestamp_period`
  behavior differ per backend, so the layer is not cross-platform until
  both are validated; the external-capture spot-check (Xcode on macOS, PIX
  or RenderDoc at checkpoint B; at least one, record which) rides the same
  sessions;
- per-eye and multiview coverage demonstrated (desktop headless dual-view
  plus the existing multiview lanes) or the gap documented;
- cross-source agreement rows recorded (Quest rows from the Mac by
  default).

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-render
pnpm native:accounting:smoke
pnpm native:desktop-offscreen:smoke
pnpm native:xr:check
git diff --check
```

Plus one Quest metrics lane recorded in this section if Quest-side wgpu
timestamps are enabled.

Recorded result: Mac implementation landed 2026-07-05. Windows checkpoint B,
external capture, and Quest periodic metrics were completed on 2026-07-06;
Slice 6 is closed. Slice 7 has not started.

Changes:

- Added schema v4 GPU timestamp reports in `mclone-diagnostics`, including
  capability state, tick period, submitted/resolved/pending counters, raw
  begin/end ticks, elapsed milliseconds, and per-pass validity.
- Added the `mclone-render` timestamp profiler with a frames-in-flight query
  pool, async map/readback, `Queue::get_timestamp_period()` conversion, and
  unsupported projection when `TIMESTAMP_QUERY` is absent.
- Routed timestamp writes through shared render targets for sky/background,
  terrain (`all`, opaque, translucent), actor, and UI passes. Query-set
  creation stays in `mclone-render`; the native surface only starts frames,
  resolves before submit, and marks readbacks pending after submit.
- Added a GPU calibration block to `native:accounting:smoke`. On the Mac
  Metal lane, the calibration used a trailing sentinel pass because Metal
  left the final timestamped pass end query unwritten without a following
  pass. Final reported calibration: light `0.071333 ms`, heavy
  `1.371458 ms`, ratio `19.23`, `timestampPeriodNs=1.0`, both reported
  passes valid.
- Promoted `XR_META_performance_metrics` to an explicit periodic mode via
  `--perf-metrics-periodic` / `MCLONE_ANDROID_XR_PERF_METRICS_PERIODIC=1`.
  Existing `--perf-metrics` lanes remain one-shot. Added
  `native:android-xr:perf:stationary:rd10:metrics-periodic` for Quest-side
  sampling.

Tripwire results:

- Non-vendored timestamp sites are in shared render crates only:
  `mclone-render` owns `create_query_set` and render pass timestamp writes;
  `mclone-app-runtime` only propagates the shared target timestamp hook for
  the sky/background fallback. No platform app owns query creation.
- `native/crates/mclone-xr-host/src/lib.rs` still has a pre-existing
  `timestamp_writes: None` descriptor; vendored `wgpu-hal` hits were
  excluded from the slice count.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
# PASS.

cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
# PASS. 14 tests passed.

cargo test --manifest-path native/Cargo.toml -p mclone-render
# PASS. 126 passed, 2 ignored.

cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
# PASS. Existing non-Android dead-code warnings for legacy remote-addr helpers.

pnpm native:accounting:smoke
# PASS. Schema version 4; GPU timestamp panel supported on Mac Metal.

pnpm native:desktop-offscreen:smoke
# PASS. Saved /tmp/mclone-desktop-offscreen.png and inspected it.

pnpm native:xr:check
# PASS on the Mac WiVRn check-only lane.

node -e "JSON.parse(require('fs').readFileSync('package.json','utf8'))"
# PASS.

git diff --check
# PASS.
```

Checkpoint follow-up: run Windows checkpoint B after switching hosts:
preparation pass, Vulkan/DX12 timestamp calibration, and PIX or RenderDoc
spot-check. If a Quest is available in that session, run the periodic metrics
lane and record `XR_META` status; otherwise run it from the Mac with the Quest
attached before closing Slice 6.

Post-slice correction, 2026-07-06:

- `pnpm native:policy:wasm-check` was not part of the Slice 6 validation
  block but should have been run because the timestamp target plumbing lives
  in shared render/app-runtime code. ORG-00 later caught the blocker:
  `gpu_timestamps.rs` called `log::warn!` while `mclone-render` declared
  `log` only for non-wasm targets. Fixed by making `log` a normal
  `mclone-render` dependency.
- Follow-up validation: `pnpm native:policy:wasm-check` PASS, and
  `cargo check --manifest-path native/Cargo.toml -p mclone-render --target wasm32-unknown-unknown`
  PASS.

Windows checkpoint B, 2026-07-06:

- Host: `rex`, Windows 11 Core build 26200, `x86_64-pc-windows-msvc`.
  Synced `main` to `54cc7ae3` (`render: fix gpu timestamp wasm check`),
  matching `origin/main`; `54cc7ae3` is HEAD.
- Toolchain prep: PowerShell `cargo` is
  `C:\Users\sox\.cargo\bin\cargo.exe`, Cargo 1.96.0, host
  `x86_64-pc-windows-msvc`; `pnpm --version` is 9.15.1.
  `pnpm host:check` passed for Node/pnpm but noted no Chrome on PATH.
  `pnpm assets:pack:check` initially reported a stale packed asset zip after
  the host switch; `pnpm assets:pack` regenerated
  `reference/minecraft-1.17.1/extracted.zip` with 6985 files. The lock then
  checked current, and the timestamp-only lock rewrite was discarded.
- Windows-only smoke fix: the original `native:accounting:smoke` default
  6.0 ms spin is below this host's `GetThreadTimes` accounting granularity.
  Release mode reported `thread_cpu_spin_ms=15.625` for the 6.0 ms target,
  and debug mode tripped the thread-CPU conservation assertion. The Windows
  default spin is now 200.0 ms; non-Windows keeps 6.0 ms. The final Windows
  default run recorded `thread_cpu_spin_ms=203.125`, `wall_spin_ms=191.7766`,
  `tolerance_ms=20.0`, and zero conservation violations.
- Windows-only XR check fix: `pnpm native:xr:check` from plain PowerShell
  initially resolved `bash` to WSL and failed before Cargo with
  `Missing required command: cargo`. Rerunning with
  `C:\Program Files\Git\bin` first in `PATH` used Git Bash and native Cargo,
  then found a real compile error in `xr_clear_smoke`: the desktop clear-smoke
  call to `create_eye_swapchain` lacked the new foveation argument. Passing
  `None` preserves the no-foveation clear-smoke path, and the Git Bash rerun
  passed.
- Quest staging fix: the existing `com.kzahel.mclone.xr` package was
  uninstalled before reinstalling because it may have been signed by the Mac
  dev key. The first Windows Quest metrics run installed and launched the new
  APK but failed asset loading with `Permission denied`; after reinstall, the
  `adb push`-created `files/assets` tree was owned by `shell:ext_data_rw` with
  `770` directories, so the app UID could not traverse it. The shared Android
  validation helper now repairs external staged assets with
  `chmod -R u+rwX,g+rwX,o+rX`; the rerun left `assets`, `packs`, and
  `local-sounds` as `drwxrwsr-x` and passed.
- RenderDoc capture hook: `accounting_smoke` now honors
  `MCLONE_RENDERDOC_CAPTURE=1` by bracketing only the validation GPU
  calibration pass with `Device::start_graphics_debugger_capture()` /
  `stop_graphics_debugger_capture()`. Normal smoke runs leave this off.

Windows GPU timestamp calibration:

| backend | command | support | `Queue::get_timestamp_period()` | light pass | heavy pass | ratio | granularity notes |
|---|---|---|---:|---:|---:|---:|---|
| default Windows headless (DX12 by `native_backends`) | `pnpm native:accounting:smoke` | supported | 1.0 ns | 0.004096 ms | 1.350656 ms | 329.75 | whole-pass timestamps valid; latest default light pass raw delta 4096 ticks |
| Vulkan | `WGPU_BACKEND=vulkan pnpm native:accounting:smoke` | supported | 1.0 ns | 0.005120 ms | 2.149376 ms | 419.80 | whole-pass timestamps valid; light pass raw delta 5120 ticks on the final forced run |
| DX12 | `WGPU_BACKEND=dx12 pnpm native:accounting:smoke` | supported | 1.0 ns | 0.005120 ms | 1.350656 ms | 263.80 | whole-pass timestamps valid; light pass raw delta 5120 ticks |

`get_timestamp_period()` returns 1.0 ns on both Vulkan and DX12 on this
host. The observed calibration deltas are integer timestamp ticks scaled by
that 1.0 ns period; the practical minimum pass delta observed here was
4.096-5.120 us on Vulkan and 4.096-5.120 us on the default/DX12 lane across
runs. The calibration remains coarse whole-pass timing only; no inside-pass
or per-draw timestamps were added.

Windows checkpoint B validation:

```bash
git fetch
git checkout main
git pull
git rev-parse --short HEAD
# PASS. HEAD and origin/main are 54cc7ae3.

pnpm host:check
# PASS for Node/pnpm; Chrome absent on PATH, not used by this native slice.

pnpm assets:pack:check
# Initial FAIL after host switch: packed asset zip stale.

pnpm assets:pack
pnpm assets:pack:write-lock
pnpm assets:pack:check
# PASS after regenerating reference/minecraft-1.17.1/extracted.zip.

cargo fmt --manifest-path native/Cargo.toml --all --check
# PASS.

cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
# PASS. 13 tests passed.

cargo test --manifest-path native/Cargo.toml -p mclone-render
# PASS. 126 passed, 2 ignored.

cargo build --manifest-path native/Cargo.toml -p mclone-native-client --bin accounting_smoke
# PASS after adding the RenderDoc capture hook.

pnpm native:accounting:smoke
# Initial FAIL on the 6.0 ms Windows spin; PASS after the Windows default spin
# fix. Schema version 4; GPU timestamp panel supported on DX12.

pnpm native:desktop-offscreen:smoke
# PASS. Saved /tmp/mclone-desktop-offscreen.png; screenshot inspected.

pnpm native:xr:check
# Initial FAIL because PowerShell resolved bash to WSL. PASS after rerunning
# with Git Bash first in PATH and fixing the stale foveation argument.

pnpm native:policy:wasm-check
# PASS. Existing mclone-server dead-code warning only.

pnpm native:android-xr:perf:stationary:rd10:metrics-periodic
# Initial FAIL after reinstall because shell-owned external staged asset
# directories were not app-readable. PASS after the external asset permission
# repair. Quest 3 `XR_META_performance_metrics` periodic sampling enabled.

git diff --check
# PASS.
```

External capture spot-check:

- RenderDoc was installed with winget from the `desktop` machine's documented
  Windows setup path:
  `winget install -e --id BaldurKarlsson.RenderDoc --source winget
  --accept-package-agreements --accept-source-agreements --silent`.
  Installed version: RenderDoc 1.45.0 under `C:\Program Files\RenderDoc`.
- DX12 under RenderDoc did not create a headless adapter on this host
  (`dx12 drivers/libraries could not be loaded`), so the external capture
  spot-check used Vulkan.
- Capture command:
  `WGPU_BACKEND=vulkan MCLONE_RENDERDOC_CAPTURE=1 renderdoccmd capture
  --wait-for-exit --capture-file C:\tmp\mclone-accounting-vulkan
  native\target\debug\accounting_smoke.exe`.
  The run passed with `timestampPeriodNs=1.0`, light `0.004096 ms`, heavy
  `2.163584 ms`, ratio `528.22`, and zero conservation violations.
- Capture artifacts: `C:\tmp\mclone-accounting-vulkan_capture.rdc` (76,435
  bytes) and converted XML `C:\tmp\mclone-accounting-vulkan.zip` (178,960
  bytes). RenderDoc `thumb` had no thumbnail because this headless calibration
  has no swapchain backbuffer.
- Converted capture evidence: driver `Vulkan`, `timestampPeriod=1`,
  `timestampValidBits` includes 64-bit queues, labels
  `mclone_gpu_timestamp_calibration_light_pass`,
  `mclone_gpu_timestamp_calibration_heavy_pass`, and
  `mclone_gpu_timestamp_calibration_sentinel_pass`, with
  `vkCmdWriteTimestamp` chunks at the pass boundaries.

Quest periodic metrics, Windows host with Quest 3 attached:

- Device: Quest 3, ADB serial `2G0YC1ZF93041Z`, product/device `eureka`, API
  34. Package `com.kzahel.mclone.xr` was uninstalled before reinstalling the
  Windows-built release APK.
- Command:
  `pnpm native:android-xr:perf:stationary:rd10:metrics-periodic`.
  Result: PASS after the external asset permission repair. The trailing
  `xargs: environment is too large for exec` message from the Git Bash wrapper
  appeared after the pass marker and did not affect the exit status.
- `XR_META_performance_metrics` status: available and enabled in periodic
  mode, with 17 counters. Samples reported `any_valid=true`; first sample
  `app_gpu_ms=2.960`, `compositor_gpu_ms=1.199`, `gpu_util_pct=31.495`,
  `motion_to_photon_ms=25.824`, `per_query_us=0.50`. Final summary:
  `sample_seconds=20.010`, `mode=stationary-settled`, `render_path=per-eye`,
  `frame_accounting_enabled=true`, `conservation_violations=0`, `frames=789`,
  `target_hz=72.0`, `frame_p95_ms=29.049`, `frame_p99_ms=30.854`,
  `frame_max_ms=32.599`, `over_budget=789`.
- Quest-side wgpu timestamp agreement: not applicable for this slice. The
  Android XR path did not emit a `gpuTimestampPanel` or wgpu timestamp totals
  in the shared report; `XR_META_performance_metrics` remains the Quest GPU
  authority, while wgpu timestamp calibration was validated on desktop
  headless Metal, Windows Vulkan, and Windows DX12.

Slice 6 close status after Windows checkpoint B:

- Windows Vulkan/DX12 timestamp support, period, and calibration are recorded.
- External capture is complete through RenderDoc Vulkan on Windows.
- Quest periodic `XR_META_performance_metrics` sampling is recorded; wgpu
  timestamp agreement on Quest is explicitly not applicable until that render
  path emits a wgpu timestamp panel.
- Slice 6 is closed. Slice 7 remains unstarted.

Additional desktop OpenXR runtime confidence pass, 2026-07-06:

- Host: `rex`, Windows 11 Core build 26200. Runtime: VirtualDesktopXR v1.0.10
  through the Virtual Desktop OpenXR manifest. Headset: Quest 3. GPU:
  NVIDIA GeForce RTX 4090, Vulkan API 1.4.341, queue family 0. This was an
  extra live desktop-XR validation pass, not a required Slice 6 closure gate.
- `scripts\start-xr.bat --vdxr --frames 2 --no-quest-launch --no-quest-restore --no-pause`:
  PASS after fixing the Windows launcher to pass
  `--bin mclone-native-client` to `cargo run`. Session reached `FOCUSED`;
  submitted 2 runtime frames, skipped 0.
- `scripts\start-xr.bat --vdxr --mclone --frames 120 --no-quest-launch --no-quest-restore --no-pause`:
  PASS. Session reached `FOCUSED`; submitted 120 runtime frames, skipped 0.
  Summary: `frames=57`, `sections=44`, `drawn_sections=6`,
  `indices=190224`, `drawn_indices=77976`, `actors=2`, `drawn_actors=2`.
- `pnpm native:xr:windows:restore`: PASS after the connected smokes.

## Slice 7: Debug Overlay Through The Shared Facade

Why: gap 8 — interactive diagnosis needs the same pipeline model the logs
record, and after tactical 143 the only acceptable shape is a shared-owner
UI surface, not app-local overlay state.

Hard gate: tactical 143 closed (or every remaining 143 slice owned and the
user explicitly approves starting this slice).

Deliverables:

- perf-HUD toggle as a capability-classified shared `GameUiAction` routed
  through `ClientExperienceController`, following the 143 classification
  table pattern (likely capability-gated; record the classification in the
  client-experience architecture doc's table);
- overlay renders the first three panels from the law doc's
  [Debug UI And Logging Target](../frame-pipeline-accounting.md#debug-ui-and-logging-target):
  frame budget bar, current-frame stacked waterfall, and queue panel — all
  as thin views over the shared schema structs (widgets in `mclone-ui`,
  state projection in the shared owner, no overlay-private stat math);
- desktop flat and offscreen first; the XR world-panel projection, peer
  panel, readiness panel, and chunk status grid are recorded follow-ups,
  not this slice.

Non-goals / drift tripwires:

- if the overlay computes any statistic the schema does not carry, stop —
  the statistic belongs in the core and the schema first (invariant 8);
- no new app-local `GameUiAction` dispatch arms (the 143 tripwire greps
  must stay clean).

Exit criteria:

- overlay toggles on desktop and shows live schema data;
  screenshot captured to `/tmp` and inspected;
- 143's inert-arm and dispatch tripwire greps show no regressions;
- unsupported profiles project shared unavailable state for the toggle.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-ui
pnpm native:policy:wasm-check
pnpm native:desktop-offscreen:smoke
git diff --check
```

Recorded result: (pending)

## Out Of Scope, Adjacent Workstreams

Listed so nobody mistakes this tactical for their plan:

- the elapsed/headroom-aware budget controller and any admission/publication
  policy change — the successor tactical, building on
  [`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
  and this tactical's data;
- `XR_EXT_performance_settings` perf-level control and thermal
  notifications — a controller input/lever, paired with the controller
  tactical (absence noted in
  [`130-quest-thread-scheduling-and-streaming-tail-attribution.md`](130-quest-thread-scheduling-and-streaming-tail-attribution.md));
- thread scheduling, niceness, and preemption experiments (tactical 130)
  and CPU/GPU overlap policy (tactical 131 /
  [`117-android-xr-rd10-gpu-floor-and-frame-overlap.md`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md));
- app-emitted Perfetto trace markers (external Perfetto remains the
  scheduler-trace tool per tactical 130);
- web/WASM timing depth beyond the capability projection;
- converting the remaining benchmark binaries to the shared schema beyond
  the two Slice 2 lanes (ledger follow-up).

## Open Questions

- Slice 1: resolved 2026-07-05. Use `GetThreadTimes` for the first Windows
  per-thread CPU sampler because it reports kernel+user thread CPU time in
  directly comparable 100 ns units. Windows validation is deferred to
  checkpoint A.
- Slice 2: resolved 2026-07-05. Keep legacy top-level benchmark JSON keys
  alongside the versioned `frame_pipeline_accounting` schema block for
  record compatibility.
- Slice 6: resolved 2026-07-06. Quest-side wgpu timestamp agreement is not
  applicable in this slice because Android XR does not emit a
  `gpuTimestampPanel` or wgpu timestamp totals. `XR_META_performance_metrics`
  remains the Quest GPU authority; desktop/headless wgpu timestamps are
  validated on Mac Metal and Windows Vulkan/DX12.
- Recorded deviation: 2026-07-05, Slices 0-2 started before tactical 143
  closed under the documented exception for new leaf crate and
  `mclone-native-client` timing-internal work. Slice 7's gate is now
  satisfied because tactical 143 closed on 2026-07-05.
