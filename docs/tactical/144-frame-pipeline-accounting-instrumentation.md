# 144: Frame Pipeline Accounting Instrumentation

Status: open, gated on tactical 143. Opened 2026-07-05. This tactical is the
executable checklist for
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
  closes. 143's contract gives convergence priority, and its remaining
  slices touch `mclone-xr-scene`, `mclone-android-client`, and app dispatch
  surfaces that Slices 3, 5, and 7 here would collide with.
- Slice 7 (debug overlay) is hard-gated on 143 regardless: its HUD toggle
  must land as a capability-classified shared `GameUiAction` through
  `ClientExperienceController`, not as new app-local UI state.
- Exception: if 143 stalls on a decision only the user can make, Slices 0-2
  and 4 touch no 143 surface (new leaf crate, `mclone-native-client` timing
  internals, headless lanes) and may start early. Record any such deviation
  under Open Questions with the reason.

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
| 0: measurement inventory and baselines | gates | Mac (baseline host; record) | open |
| 1: `mclone-diagnostics` accounting core | 1 (owner exists) | Mac; Windows test lands at checkpoint A | open |
| 2: desktop flat and benchmark adoption | 1 desktop, 2 partial | Mac; then Windows checkpoint A | open |
| 3: Quest / Android XR adoption | 1 Quest, 2 | Mac with Quest | open |
| 4: calibration, invariants, meter overhead | 7 | Mac with Quest | open |
| 5: queue-age, admission-tail split, peer threads | 3, 4, 5 | Mac with Quest | open |
| 6: GPU timestamp layer and perf-metrics promotion | 6 | Mac with Quest; Windows checkpoint B | open |
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

Recorded result: (pending)

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

Recorded result: (pending)

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

Recorded result: (pending)

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

Recorded result: (pending)

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

Recorded result: (pending)

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

Recorded result: (pending)

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

Recorded result: (pending)

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

- Slice 1: which Windows per-thread CPU API serves the busy-vs-blocked
  split — `GetThreadTimes` (coarse ~15.6 ms quanta) or
  `QueryThreadCycleTime` (cycles, needs frequency normalization)? Decide at
  implementation and record here.
- Slice 2: schema-vs-legacy JSON field policy for
  [`../performance-records.md`](../performance-records.md) lanes — keep
  legacy keys alongside the schema block, or version-break and re-baseline
  the records?
- Slice 6: are wgpu timestamps enabled on Quest at all (validation-only),
  or desktop/headless-only behind capability facts? Decide from adapter
  capabilities plus `XR_META` agreement and record.
- Recorded deviations from slice order or the 143 gate, if any, go here
  with reasons.
