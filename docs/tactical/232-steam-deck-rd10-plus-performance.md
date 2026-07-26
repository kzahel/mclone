# 232: Steam Deck RD10+ Frame Pacing And Render Throughput

Status: completed 2026-07-24. The physical Steam Deck test bed, autonomous
matrix, ordered optimization campaign, release-instrumentation calibration,
final RD5/RD8/RD10/RD13 policy run, and shared platform closeout are complete.

Topic: `steam-deck-rd10-plus-performance`

## Originating Request

Use the dedicated Steam Deck as an autonomous performance laboratory, in the
same spirit as the earlier physical Quest work. Explain why the apparently
low aggregate CPU/GPU readings can coexist with poor frame pacing, compare the
current main-thread shape with Minecraft Java 1.17.1, record the resulting
optimization candidates durably, and then prove or reject them end to end.

The work must cover both:

- stationary high-altitude views, including the freshly settled fluid/remesh
  tail and a genuinely time-soaked steady state; and
- equal-distance traversal at RD5, RD8, RD10, and RD13, so an optimization
  cannot make a static screenshot fast while allowing generation,
  publication, update application, meshing, upload, or movement to damage
  frame pacing.

No candidate is accepted because it is architecturally attractive. Each slice
needs a controlled Steam Deck A/B, unchanged readiness/completeness, and an
explicit keep, revise, or revert decision.

## Scope And Ownership

This is a shared-engine performance campaign exercised first on Steam Deck,
not a Deck-specific renderer fork. Changes belong in their existing shared
owners:

- server ticket propagation and holder reconciliation in `mclone-server`;
- client replica and render-work state in `mclone-app-runtime`;
- frame orchestration and admission in `mclone-scene`;
- culling, draw preparation, GPU timing, and draw submission in
  `mclone-render`; and
- physical-device automation and result summarization in
  `scripts/steam-deck*`.

App-local work is limited to command-line diagnostic controls, winit surface
timing, and physical-test orchestration. Quest, browser, Android, XR, and
offscreen paths keep the same shared contracts. Any new renderer path must
remain correct for mono, per-eye stereo, and full-frame multiview, or document
an intentional capability boundary.

This tactical consumes the durable measurement rules in
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md), the
physical lane in
[`../topics/steam-deck-test-bed.md`](../topics/steam-deck-test-bed.md), and the
current priority index in
[`../topics/performance.md`](../topics/performance.md). It carries forward the
completed record-cache and cull-scratch work from
[`106`](106-android-xr-static-render-cpu-reduction.md) and does not repeat
those old pre-fix costs.

## Baseline And Diagnosis

The baseline authority is matrix run
`20260724T122203Z-93ce5851e860-perf-matrix-3546078`, captured through the
native 1280x800 Gamescope panel at 89.887 Hz. The complete bundle is under
`/tmp/mclone-steam-deck-results` on the paired development host; the durable
summary remains in the Steam Deck topic.

| Case | FPS | p95 frame | p95 encode | Draw sections | CPU cores | GPU |
|---|---:|---:|---:|---:|---:|---:|
| stationary top-down RD5 | 89.8 | 12.50 ms | 5.34 ms | 282 | 0.70 | 15.2% |
| stationary top-down RD10 | 86.1 | 16.29 ms | 13.32 ms | 1,053 | 1.28 | 27.5% |
| stationary top-down RD13 | 68.2 | 21.10 ms | 20.09 ms | 1,151 | 1.61 | 39.5% |
| traversal top-down RD5 | 89.8 | 13.12 ms | 6.71 ms | 320 | 1.74 | 29.7% |
| traversal top-down RD10 | 68.4 | 21.26 ms | 16.20 ms | 1,047 | 2.59 | 48.5% |
| traversal top-down RD13 | 46.1 | 28.02 ms | 26.09 ms | 1,178 | 2.96 | 55.9% |
| traversal oblique RD13 | 54.0 | 24.62 ms | 21.90 ms | 546 | 2.83 | 38.2% |

At RD13 traversal, a live thread sample measured the client/render thread at
93.6% of one logical CPU, the integrated server at 57.1%, and the sole render
compiler at 16.6%. That is only about 1.67 of the Deck's eight logical CPUs,
so low aggregate utilization does not imply spare capacity on the serial
render critical path.

The top-down/oblique contrast is the strongest current evidence. Top-down
approximately doubles drawn sections and raises encode p95 by 4.19 ms while
the GPU remains below saturation. Fill and geometry cost are real, but the
first-order failure is CPU draw preparation/encoding and main-thread-owned
integration, not a simple fragment-fill ceiling.

Freshly idle is also not render-static. Stationary RD13 rebuilt 578 sections
in 20 seconds. Its first-quarter FPS was 59.4 and last-quarter FPS was 77.9 as
fluid-driven remesh work decayed. Frames accepting no rebuilt section averaged
80.1 FPS; rebuild frames averaged 53.6 FPS. A prior ten-minute run eventually
held 90 Hz after the activity ended.

## Minecraft 1.17.1 Alignment

Minecraft Java 1.17.1 has the same broad ownership shape:

- one client/render thread owns input, client ticks, camera, client-world
  update application, visibility, render admission, GPU uploads, draw calls,
  and presentation;
- the integrated server has its own server thread;
- chunk mesh compilation uses workers, with nearby/player-dirty rebuilds
  allowed synchronously;
- completed GPU uploads return to the render thread; and
- terrain layers still issue one draw per non-empty section/layer.

The comparison identifies two places where the current engine repeats more
work than the reference:

1. `LevelRenderer` retains its visible `renderChunks` list while camera pose,
   render distance, dirty state, and compile state are unchanged. Mclone
   already caches prepared section records but reruns section culling and
   traversal every frame.
2. `DistanceManager` propagates ticket changes through incremental graph
   updates. Mclone reconstructs the complete propagated `active_levels`
   `BTreeMap` from every ticket radius each time a caller asks for it.

Vanilla also normally provisions up to four chunk-render workers, subject to
memory. Mclone defaults to one. The Deck baseline does not currently show a
compile-worker bottleneck, so worker count remains a measured A/B rather than
the first fix.

## Benchmark Contract

Every performance claim uses the physical Deck in Gaming Mode with:

- the internal panel awake and confirmed enabled;
- native 1280x800 output and world resolution unless resolution is the
  independent variable;
- Gamescope FIFO at the reported refresh rate;
- the same seed, camera, render distance, cadence, lighting, content options,
  and movement distance;
- the complete view-settled gate before measurement;
- recorded artifact hash, source commit/dirty state, power state, clock,
  temperature, memory, and queue state; and
- an automatic restoration of the interactive shortcut and panel-sleep
  policy.

The matrix keeps these distinct:

- **fresh stationary:** begin immediately after the complete view-settled gate;
- **soaked stationary:** wait until fluid/remesh activity reaches the stated
  quiet criterion, then measure;
- **frozen-fluid diagnostic:** freeze scheduled fluid ticks without changing
  generated terrain, solely to attribute the remesh tail;
- **traversal:** translate the camera and interest center at 16 blocks/second
  for a fixed wall-clock duration;
- **top-down versus oblique:** preserve position, travel, and options while
  changing only view direction; and
- **resolution scale:** preserve the same world and view while changing only
  internal world resolution.

Use alternating `A/B/A/B` or `B/A/B/A` order for small expected changes when
thermal drift can rival the effect. Report medians of complete rows as well as
individual runs. A row that fails the completeness gate is invalid, not fast.

### Acceptance And Guardrails

For a low-risk CPU slice to be kept:

- the targeted span or queue count must move in the predicted direction;
- the RD10 or RD13 pressure row must improve by more than normal paired-run
  variance, provisionally at least 5% in the primary metric;
- RD5 traversal frame p95, over-2x count, travel distance, publication totals,
  and readiness must not regress materially;
- rebuilt/uploaded results and stale-result rejection must conserve;
- no queue may be made artificially small by dropping work; and
- relevant unit, integration, and rendered-output checks must pass.

Larger renderer changes additionally require identical accepted mono pixels
for deterministic captures, stereo/multiview coverage, correct translucent
ordering, bounded GPU residency, and no resource-lifetime validation errors.

The 89.887 Hz period is approximately 11.125 ms. RD13 is a stress target, not
an unconditional promise that every current scene must already hold 90 Hz.
The campaign target is to identify the real scaling curve, recover avoidable
CPU cost, eliminate terrain-generation frame stalls, and make any remaining
GPU/content ceiling explicit.

## Hypotheses

| ID | Hypothesis | Disproof |
|---|---|---|
| H1 | Exact render-work accounting and loaded-set copying consume meaningful stationary and traversal CPU. | Removing the scans does not move their attributed spans or paired frame results. |
| H2 | Reusing a visibility/draw plan while camera and render generations are unchanged materially improves settled stationary RD10/RD13. | Cull work is already negligible or invalidation happens every frame. |
| H3 | Reusing or incrementally maintaining ticket distance levels lowers integrated-server CPU and traversal contention without changing holder decisions. | Ticket propagation is not a significant current span or the cache is almost always invalidated. |
| H4 | Scheduled-fluid mutations account for most of the fresh-stationary remesh/FPS decay. | Frozen and live-fluid rows have similar rebuild counts and frame distributions. |
| H5 | More render workers help cold/traversal tails only when compile queue age and worker occupancy prove demand. | Pending depth remains low and added workers do not improve readiness or frame tails. |
| H6 | Top-down RD10+ is primarily draw-count/CPU submission limited rather than fragment-fill limited. | Halving world resolution materially restores frame rate and GPU timestamps dominate while encode CPU stays flat. |
| H7 | A coarse spatial hierarchy reduces moving-view cull cost by rejecting groups before section-level tests. | Traversal cost is dominated by accepted/drawn sections rather than candidate scans. |
| H8 | Shared mesh arenas plus indirect/multi-draw submission materially reduce RD10+ encode time. | Backend support, buffer churn, or accepted draw count leaves command cost unchanged or worsens GPU time. |

## Ordered Execution

### Slice 0: Measurement Trust And Controlled A/B Knobs

- [x] Attribute frame-accounting scans, traversal-ready stamp construction,
  update integration, record preparation, cull, per-layer draw encoding,
  upload publication, surface acquire, submit, present, and device wait.
- [x] Add supported wgpu terrain/pass timestamp queries with asynchronous
  readback and explicit unsupported capability reporting.
- [x] Add matrix controls for native/half world resolution, scheduled-fluid
  freeze, one/two/derived render workers, fresh/soaked stationary, and focused
  candidate A/B selection.
- [x] Extend matrix summaries with GPU pass time, accounting/stamp/cull spans,
  fluid mutation/rebuild correlation, compiler occupancy/queue age, and ticket
  propagation/reconciliation time.
- [x] Calibrate measurement overhead and retain release-shaped default rows.

Exit: H6 can be tested without using GPU busy percentage as a proxy, and every
later candidate has a named primary span.

### Slice 1: Constant-Time Frame Bookkeeping

- [x] Replace per-frame exact `pending_render_chunk_count` set construction
  with mutation-maintained counts or cached summaries.
- [x] Preserve an explicit exact audit for tests, settle gates, and periodic
  diagnostic verification.
- [x] Replace full loaded-chunk-set copies in traversal-ready stamps with
  monotonic client/load and render generations.
- [x] Prove counter/generation conservation under load, unload, dirty,
  inflight, stale-result, and removal transitions.
- [x] Carry the accepted bookkeeping through the final RD5/RD10/RD13
  stationary and traversal guardrail matrix.

Exit: H1 is accepted or rejected. Pixel output and scheduling policy are
unchanged.

### Slice 2: Vanilla-Shaped Static Visibility Reuse

- [x] Define a cache key containing camera/view/projection, topology, render
  options, render distance, prepared-record generation, traversal readiness,
  and any state that changes the drawable list or ordering.
- [x] Reuse the culled/drawn section plan when the key is unchanged.
- [x] Invalidate exactly on section add/remove/mesh/connectivity/readiness,
  camera or projection changes, and relevant option changes.
- [x] Keep uniform writes and actual frame drawing current; cache commands or
  pixels only in a separately measured later slice.
- [x] Validate stationary, camera nudge/rotation, chunk mutation, fluid
  mutation, render-distance change, and topology cases.

Exit: H2 is accepted or rejected. Traversal is expected to remain unchanged.

### Slice 3: Ticket Distance Reuse Then Incremental Propagation

- [x] Instrument `active_levels`, holder reconciliation, unload processing,
  and their call counts per server tick.
- [x] First cache the propagated map behind a ticket/topology generation and
  share one immutable result across same-generation callers.
- [x] Prove exact equality against the current full reconstruction across
  add/remove/timeout/forced/player movement and plane/cylinder topology tests.
- [x] Run stationary and traversal A/Bs.
- [x] Reject changed-source graph propagation because physical attribution
  shows `active_levels` at effectively 0.00 ms p95; retain the unchanged
  holder/runtime-target plan that addresses the measured repeated work.

Exit: H3 is accepted or rejected without changing holder levels, generation
order, unload decisions, or published chunks.

### Slice 4: Fluid-To-Remesh Attribution And Coalescing

- [x] Add the frozen-fluid stationary rows and record executed ticks, mutated
  blocks, dirty sections, superseded/stale compiles, rebuilt/uploaded sections,
  and visible/offscreen classification.
- [x] Identify whether repeated dirtying, neighbor fan-out, stale completions,
  or unchanged render facts cause redundant meshes.
- [x] Confirm existing update-batch dirty deduplication and compile revision
  supersession; decline another coalescing layer because stationary runs have
  zero stale compiles and already combine multiple fluid mutations per mesh.
- [x] Avoid only provably unchanged render work; preserve Vanilla fluid
  simulation and client-visible state.
- [x] Validate cross-section and cross-chunk water/lava boundaries as well as
  fresh/soaked stationary Deck rows.

Exit: H4 is quantified, and any kept fix reduces redundant work rather than
silencing simulation.

### Slice 5: Render Compiler Capacity

- [x] Compare one worker, two workers, and shared derived capacity with equal
  max-pending policy.
- [x] Record worker busy time, queue age/depth, completed/stale results,
  publication/application pressure, frame tails, and total settle/travel work.
- [x] Keep a new default only if queue evidence and multiple rows show a
  repeatable net win without starving the render or server critical paths.

Exit: H5 is accepted or rejected. Available core count alone is not evidence.

### Slice 6: Moving-View Spatial Culling

- [x] Separate candidate-section scan cost from accepted traversal and draw
  encoding.
- [x] Prototype region/column bounding volumes or another generation-cached
  hierarchy over prepared records.
- [x] Preserve current frustum, readiness, occlusion-connectivity, topology,
  and observer-nearest lift semantics.
- [x] Measure the RD13 pressure row and reject the hierarchy when maintenance
  cost erases moving-view savings.

Exit: H7 is accepted or rejected. Static cache performance is evaluated
separately.

### Slice 7: Reduced Draw Submission

- [x] Use H6, GPU timestamps, cull spans, and per-layer draw counts to choose
  between render bundles, shared vertex/index arenas with indirect draws, or
  a smaller state-change batching step.
- [x] Keep per-section unique geometry while moving chunk transforms and draw
  ranges into stable GPU-visible records.
- [x] Preserve opaque/cutout ordering, translucent back-to-front behavior,
  mesh replacement, stale-result rejection, topology lifts, and bounded
  allocation.
- [x] Feature-detect multi-draw for mono while retaining arena-backed direct
  draws for per-eye stereo and full-frame multiview; cover all three paths.
- [x] Compare CPU encode, GPU terrain time, residency, uploads, frame tails,
  and visual output across the full Deck matrix.

Exit: H8 is accepted or rejected. A path that merely moves CPU time into GPU
time without improving frame pacing is not a win.

### Slice 8: Closeout And Durable Policy

- [x] Run the complete stationary/traversal matrix on a clean SteamRT4
  artifact, including fresh, soaked, fluid-control, resolution-control, and
  selected worker rows.
- [x] Record before/after tables, thermal/run variance, failed hypotheses,
  reverted experiments, and remaining ceilings here and in the Steam Deck
  topic.
- [x] Update `docs/topics/performance.md` with the new priority order.
- [x] Re-run shared platform gates for every changed owner and record any
  pending physical Quest/browser/Android acceptance honestly.
- [x] Decide supported Deck render-distance/default policy from measured
  native-panel results rather than treating RD13 stress as the default.

## Results Ledger

Append each tested slice here with:

- commit and binary hash;
- exact Deck run IDs and matrix rows;
- primary and guardrail deltas;
- completeness/conservation/pixel result;
- decision: keep, revise, reject, or revert; and
- the next hypothesis changed by the evidence.

### 2026-07-24: Tactical Opened

- Baseline and thread-attribution evidence recorded above.
- Minecraft Java 1.17.1 ownership comparison completed.
- No optimization has yet been applied.
- Next action: Slice 0 controls and subphase instrumentation, beginning with
  the existing live-window report and matrix rather than a second benchmark
  harness.

### 2026-07-24: Slice 0 Attribution Matrix

- Instrumentation commit: `fc7fbbd0`.
- SteamRT4 binary SHA-256:
  `6a9335fe68cf887e6c9692292a1c0ac4af40ad13694963d81eddbc66271bdbd0`.
- Run:
  `20260724T132612Z-fc7fbbd05e7f-perf-matrix-attribution-3647962`.
- All five rows reached the then-current complete idle gate. Both traversal
  rows covered approximately 320 blocks in 20 seconds.
- RD13 stationary native/half/frozen-fluid measured 64.1/69.3/89.7 FPS.
  Native and frozen GPU terrain p50 remained close at 2.48/2.46 ms while
  surface-encode p95 fell from 20.95 to 9.51 ms when fluid-driven rebuilds
  fell from 679 to zero.
- RD13 traversal native/half measured 48.2/47.3 FPS with equal travel.
  Native GPU terrain p50 was 2.75 ms against 24.23 ms surface-encode p95;
  halving world resolution did not improve the row.
- Native stationary/traversal p95 subphases were respectively:
  pending render accounting 1.06/1.30 ms, traversal-ready refresh
  3.45/3.85 ms, prepared-record rebuild 3.63/4.69 ms, and cull 4.70/4.99 ms.
- Decision: accept H6. RD13 top-down is CPU preparation/submission limited,
  not primarily fragment-fill limited.
- Decision: accept H4's attribution. Scheduled-fluid mutation is the cause of
  the fresh-stationary remesh tail; freeze remains diagnostic rather than
  gameplay policy.
- Instrumentation defect: compiler completion/busy counters were compiled
  out of this release artifact even though worker timing was requested.
  Matrices now request the `perf-diagnostics` Cargo feature and record it in
  the SteamRT4 receipt before the worker-capacity experiment.
- Next action: run the corrected worker matrix, then implement bookkeeping,
  static-cull, active-level, and fluid/remesh slices against these named
  spans.

### 2026-07-24: Compiler Worker Capacity Rejected

- Diagnostic-build commit: `43d6964d`.
- SteamRT4 binary SHA-256:
  `248a99d96d64ac97042a99b76eacb8a6e926fc05e91c1777a5b760bb12785c36`.
- Run: `20260724T133622Z-43d6964d5639-perf-matrix-workers-3670064`.
- RD10 one/two/derived-worker traversal measured 72.0/71.4/70.7 FPS and
  20.13/21.26/21.44 ms frame p95.
- RD13 one/two/derived-worker traversal measured 46.9/46.8/46.9 FPS and
  27.47/28.50/27.90 ms frame p95.
- Equal-distance and work guardrails held: every row traveled approximately
  320 blocks; RD10 rebuilt 7,633-7,663 sections and RD13 rebuilt
  8,685-8,735.
- One worker was busy for 8.19 seconds at RD10 and 8.03 seconds at RD13, but
  the compile queue stayed bounded at four and three jobs. More workers only
  lowered instantaneous queue depth; they did not improve completion totals,
  travel, or frame pacing.
- Decision: reject H5 and retain the one-worker default. The limiting path is
  main-thread integration/preparation/submission, not compiler throughput.

### 2026-07-24: Bookkeeping Cache Kept

- Candidate commit: `40abdccc`.
- SteamRT4 binary SHA-256:
  `b9ace21e3c076efdbc98395b2a6f2164b91edbf5614fdac8ba39997bfd001f45`.
- Run:
  `20260724T140444Z-40abdccc0fc5-perf-matrix-attribution-3731787`.
- Against the `fc7fbbd0` attribution control, native RD13 pending-render
  accounting p95 fell from 1.06 to 0.82 ms stationary and from 1.30 to
  0.98 ms during equal-distance traversal. Traversal-ready construction fell
  from 3.45 to 3.24 ms stationary and from 3.85 to 3.65 ms traversing.
- Exact audits and mutation-generation tests passed. The 320.2-block
  traversal published 493 feature chunks and 480 light statuses and rebuilt
  8,742 sections.
- Overall traversal did not improve: 46.4 FPS and 27.68 ms p95 versus the
  control's 48.2 FPS and 26.78 ms. Fresh-fluid work differed between runs, so
  the stationary 64.1-to-69.5 FPS change is not attributed to bookkeeping.
- Decision: keep H1's low-risk targeted reduction, but reject bookkeeping as a
  primary RD13 frame-rate fix. RD5/RD10 guardrail rows remain for closeout.

### 2026-07-24: Same-Generation Ticket Reuse Kept

- Candidate commit: `8ffeb803`.
- SteamRT4 binary SHA-256:
  `2c3d9ba6d40ef4e5fe27ba67dc25306312edaf0147b36d5f5ea008fbe6248d3e`.
- Run:
  `20260724T141138Z-8ffeb80309f1-perf-matrix-attribution-3739759`.
- Against the bookkeeping-only parent, native RD13 traversal improved from
  46.4 to 49.3 FPS and frame p95 fell from 27.68 to 25.32 ms.
- Equal-work guardrails held: 320.1 blocks traveled, 493 feature and 493 light
  publications completed, and 8,826 sections rebuilt.
- Stationary FPS is not used for this decision because the measured live
  fluid tail rebuilt only 381 sections versus 549 in the parent run.
- The later detailed build still measures holder reconciliation at
  approximately 4 ms p95 during traversal. Same-generation reuse therefore
  helps, but does not remove the cost when player movement changes tickets.
- Decision: keep H3's immutable cache and continue to changed-source
  incremental propagation only after separating active-level construction
  from the rest of holder reconciliation.

### 2026-07-24: Stable Visibility And Record Reuse Kept

- Candidate commit: `c261efcf`.
- SteamRT4 binary SHA-256:
  `caaa03244ef75ad9d794b7f96243b384e6f4d486434e47a8963223e98af13700`.
- Run:
  `20260724T141728Z-c261efcfbf41-perf-matrix-attribution-3747451`.
- Against its ticket-cache parent, native RD13 traversal improved from 49.3
  to 71.4 FPS and frame p95 fell from 25.32 to 21.62 ms while traveling
  319.9 blocks, publishing 493/493 feature/light results, and rebuilding
  9,337 sections.
- Prepared-record p95 fell from 4.55 to effectively 0.00 ms and terrain-cull
  p95 fell from 5.03 to 3.08 ms. The traversal row recorded 889 cache hits in
  2,318 lookups. Only 41 of 9,380 submitted compile sections became stale, so
  compiler supersession is not the dominant remaining work.
- Frozen-fluid stationary recorded 3,595 hits in 3,596 lookups, held 89.9
  FPS, and reduced surface-encode p95 to 6.95 ms. Live stationary reached
  85.9 FPS with 459 rebuilt sections; no-rebuild frames averaged 91.4 FPS
  while rebuild frames averaged 70.9 FPS.
- Native/half traversal remained 71.4/71.1 FPS with GPU terrain p50
  2.68/2.26 ms, reinforcing H6: remaining pressure is still CPU-side.
- Decision: accept and keep H2. H4's dominant redundant work was full
  readiness/record/cull recomputation after bounded mesh changes, not stale
  compiler results. Continue with incremental readiness publication, moving
  spatial culling, and draw submission.

### 2026-07-24: Column Readiness Publication Kept

- Candidate commit: `fa5f442e`.
- SteamRT4 binary SHA-256:
  `193127c89aabdca30ca9ec72909e6dcabd7064eee0fa98b3179c09261ff548a1`.
- Run:
  `20260724T143430Z-fa5f442e3610-perf-matrix-attribution-3774334`.
- Against the stable-record parent, native RD13 traversal improved from 71.4
  to 76.6 FPS and frame p95 fell from 21.62 to 19.65 ms. Surface-encode p95
  fell from 17.90 to 14.38 ms.
- Traversal-readiness p95 fell from 3.55 to 0.37 ms by publishing readiness
  once per changed chunk column and letting the renderer patch only sections
  in those columns. Prepared-record p95 remained effectively zero.
- Work guardrails held: travel was 320.0 blocks, feature/light publication
  was 493/493, 9,411 sections rebuilt, and only 30 compile results became
  stale.
- Native/half traversal measured 76.6/78.0 FPS with GPU terrain p50
  2.68/2.29 ms. Native stationary held 84.6 FPS while rebuilding 383
  sections; the frozen-fluid control held 89.8 FPS.
- Decision: keep the bounded readiness path. The next measured CPU spans are
  cull at 3.23 ms p95, terrain encode at 4.18 ms p95, and ticket-holder
  reconciliation at 3.91 ms p95.

### 2026-07-24: Coarse Moving-View Hierarchy Rejected

- Candidate commit: `c1906d35`.
- SteamRT4 binary SHA-256:
  `7e2ead6fd10021d59afd718524a548632e32650859f9e1325c8647f46c75b430`.
- Run:
  `20260724T144549Z-c1906d35e33d-perf-matrix-attribution-3801055`.
- The prototype maintained generation-cached 64-block regions, conservatively
  rejected regions before exact section tests, retained an exact fallback for
  finite topologies, and passed exact-result and rendered-pixel checks.
- It worked mechanically: the native RD13 traversal row tested an average of
  254 regions, rejected 191, and performed 3,988 exact section tests.
  Frustum and occlusion-traversal p95 were 0.94 and 2.28 ms.
- It did not meet the keep bar. Cull p95 changed only from 3.23 to 3.19 ms
  and traversal changed from 76.6 FPS/19.65 ms p95 to 77.3 FPS/19.81 ms.
  The small FPS delta is run variance, not a measured hierarchy win.
- Travel was 319.9 blocks and 9,397 sections rebuilt. Native/half traversal
  remained 77.3/77.0 FPS, again excluding fragment fill as the limit.
- Decision: reject H7 and revert the hierarchy. The accepted/drawn section
  population and occlusion traversal dominate enough that coarse ordered-map
  rejection does not repay its indirection. Proceed to draw submission and
  finer holder-reconciliation attribution rather than adding another CPU
  hierarchy.

### 2026-07-24: Shared Terrain Arenas And Multi-Draw Kept

- Candidate commits: `4d544f65`, `dbd039f5`, and `e5a45346`.
- Final SteamRT4 binary SHA-256:
  `82a0ebf512d2f7ee0b220a3b44669665b73256c6819f8a88c934d2e8df42c897`.
- Focused run:
  `20260724T152436Z-e5a453469c10-perf-matrix-attribution-3864793`.
- Full guardrail run:
  `20260724T153005Z-e5a453469c10-perf-matrix-3868806`.
- Terrain meshes now occupy growable, range-managed shared GPU vertex pages
  and one shared index arena. Supported adapters encode solid and cutout
  sections as multi-draw-indirect groups per vertex page. Translucent draws
  remain direct and retain exact global back-to-front order. Stereo and
  multiview use the same arena storage with direct draws.
- The first one-buffer prototype exposed two useful limits rather than
  producing a valid comparison. Run
  `20260724T151512Z-4d544f65edf5-perf-matrix-attribution-3857969`
  found a false power-of-two growth failure at 4,194,304 vertices. Exact
  limiting fixed that defect, then run
  `20260724T151721Z-dbd039f57c80-perf-matrix-attribution-3860553`
  reached the Deck adapter's real 256 MiB per-buffer limit near 6.71 million
  vertices. Bounded vertex paging removed that single-buffer ceiling. Both
  failed runs restored the shortcut, slept the panel, and left Gamescope,
  SteamOS, and SSH healthy.
- Against `fa5f442e`, native RD13 top-down traversal improved from 76.6 to
  84.6 FPS. Terrain encode p95 fell from 4.18 to 0.85 ms and surface encode
  p95 from 14.38 to 11.43 ms. Frame p95 changed only from 19.65 to 19.46 ms,
  showing that the remaining tails now lie outside terrain draw encoding.
- That traversal averaged approximately 417 direct translucent draws, four
  multi-draw calls, and 1,677 underlying indirect solid/cutout draws. Vertex
  residency was 424.2 MiB used in 512 MiB capacity; index residency was
  63.6 MiB used in 128 MiB capacity. GPU terrain p50 stayed approximately
  2.62 ms rather than absorbing the CPU reduction.
- Deterministic no-actor parent/candidate captures had zero differing pixels.
  Mono, per-eye stereo, full-frame multiview, placed-terrain, allocator,
  indirect-offset, and one-world resource-ownership checks passed.
- The exact-hash guardrail run held 89.9/89.8/89.0/84.3 FPS during equal
  RD5/RD8/RD10/RD13 top-down traversal. All rows traveled approximately
  320 blocks and published 253/343/403/493 feature and light results.
  Corresponding frame p95 was 12.69/12.92/13.10/20.28 ms. RD13 oblique
  traversal reached 87.2 FPS and 14.54 ms p95.
- Native stationary RD5/RD8/RD10/RD13 all held 89.5-90.0 FPS. Adaptive
  admission remained worse at RD13, 83.2 versus 84.3 FPS, and stays an
  opt-in diagnostic rather than the recommended Deck policy.
- Decision: accept H8 and keep the shared arena/multi-draw path. Terrain
  command construction is no longer the primary Deck limit. Next attribute
  holder reconciliation, then distinguish render-upload/remesh bursts from
  the remaining cull/occlusion cost.

### 2026-07-24: Holder Reconciliation Plan Reuse Kept

- Attribution commit: `ba8c44ec`.
- Attribution SteamRT4 binary SHA-256:
  `08eed7373255fb76824b42f2abc610a7e5a65198b4ee3b930464efc6fdae378a`.
- Attribution run:
  `20260724T155751Z-ba8c44ecf359-perf-matrix-attribution-3892656`.
- Native RD13 traversal split the old 3.77 ms reconcile p95 into effectively
  0.00 ms active-level lookup, 2.56 ms holder updates, and 1.24 ms runtime
  target enqueue. The active-level map hit its cache on every sampled call;
  a tick still revisited approximately 3,132 holders and 1,014 runtime
  targets.
- Decision: reject a changed-source distance graph as the next H3 step.
  Distance propagation is not the measured tick cost. Keep the earlier
  immutable map cache and target unchanged holder policy instead.
- Candidate commit: `d7344628`.
- Candidate SteamRT4 binary SHA-256:
  `dc3ea20d378adbf366890ad1a4879150b5a0dffff347215dc7ce59cd098c7f8a`.
- Focused candidate run:
  `20260724T160927Z-d7344628d640-perf-matrix-attribution-3906378`.
- Against the attribution parent, native RD13 traversal reconciliation p95
  fell from 3.77 to 1.10 ms, holder-update p95 fell from 2.56 ms to zero,
  and frame p95 fell from 19.69 to 16.53 ms. FPS moved from 84.7 to 85.6.
  The candidate still traveled 319.9 blocks, published 493/493 feature/light
  results, rebuilt 9,571 sections, and reported 50 stale compiles.
- The plan is regenerated when ticket membership, priority centers, topology,
  or lighting target policy changes. Unchanged ticks retain the sorted
  runtime-target plan and skip holder level/visibility/dependency reapply,
  but still scan runtime targets each tick so completed persistence,
  generation, and light work continues to advance.
- Decision: keep H3's generation-gated holder plan. The primary span improved
  by 71% and the frame p95 by 16%; the remaining approximately 1.1 ms is the
  real per-tick runtime-admission scan.

### 2026-07-24: Fluid Coalescing Closed Without Another Policy

- In the candidate's native fresh-stationary row, 2,105 fluid block mutations
  produced 418 rebuilt sections and zero stale compile results. Other final
  stationary rows also recorded zero stale compiles.
- Existing server-update batching deduplicates repeated dirty sections before
  one revision bump. The compile lifecycle rejects superseded revisions, and
  focused render-session tests cover duplicate section dirtying plus
  cross-section/cross-chunk boundaries. Server water/lava boundary and
  scheduled-fluid suites pass.
- Frozen-fluid stationary remains the attribution control: it holds 89.9 FPS
  with zero rebuilds, while fresh live-fluid rebuild frames are materially
  slower. That work represents visible changing terrain, not evidence of an
  obsolete compile backlog.
- Decision: H4 is quantified and the existing coalescing boundary is kept.
  Do not add delay or suppress Vanilla fluid simulation without a future
  counter proving duplicate accepted meshes for the same effective revision.

### 2026-07-24: Final Matrix And Measurement Calibration

- Final diagnostic run:
  `20260724T161657Z-d7344628d640-perf-matrix-3916887`.
- All 14 rows completed, restored the interactive shortcut, slept only the
  internal panel, and left Gamescope, SteamOS, and SSH healthy.
- Equal-distance top-down traversal measured:

  | RD | FPS | frame p95 | frame p99 | cull p95 | reconcile p95 |
  |---:|---:|---:|---:|---:|---:|
  | 5 | 89.9 | 12.60 ms | 14.47 ms | 1.32 ms | 0.26 ms |
  | 8 | 89.8 | 12.83 ms | 14.46 ms | 2.25 ms | 0.55 ms |
  | 10 | 89.5 | 12.98 ms | 14.61 ms | 2.83 ms | 0.71 ms |
  | 13 | 83.6 | 20.97 ms | 25.23 ms | 3.39 ms | 1.16 ms |

- Every traversal covered approximately 320 blocks. RD5/RD8/RD10 completed
  253/343/403 feature and light publications. RD13 completed 493 feature and
  489 light publications by the fixed measurement endpoint, with no failed
  readiness gate; focused adjacent runs completed 493/493.
- Stationary oblique/top-down RD5 through RD13 all averaged 89.7-89.9 FPS.
  RD13 oblique traversal measured 87.8 FPS and 13.80 ms p95. Adaptive
  admission tied native RD13 at 83.6 FPS and remains unrecommended.
- Release-shaped SteamRT4 binary SHA-256:
  `aff985906db3610dbe3dd5fe7e0f543bf9be0bc26e8194046808dab9b5322808`.
- Release-control run:
  `20260724T162954Z-d7344628d640-perf-matrix-attribution-3935581`.
- Diagnostics overhead was below run variance. Native RD13 traversal measured
  85.6 FPS/16.53 ms p95 with diagnostics and 85.0 FPS/18.21 ms without;
  half-resolution moved in the opposite direction, 85.4/16.96 to
  85.6/15.97. Stationary rows were effectively identical and work
  conservation held.
- Policy: use RD8 as the refresh-rate-average handheld default class. RD10 is
  a near-90 quality option with measured tail risk. RD13 is a supported
  stress/quality option in the 84-86 FPS range for this hardest view, not a
  90 Hz or 11.125 ms p95 promise.

### 2026-07-24: Exact-Commit Shared Closeout

- Validation used a detached clean worktree at implementation commit
  `d7344628`, the exact source used by the final physical Deck matrix.
- Formatting, the matrix summarizer syntax check, and `git diff --check`
  passed. The full `mclone-server` suite passed 549 tests. Shared
  `mclone-render`, `mclone-render-session`, `mclone-app-runtime`, and
  `mclone-scene` library suites passed; the initially combined run exposed
  one timing-sensitive background-asset test miss under host load, then that
  test and the complete 325-test app-runtime suite passed on rerun.
- The native client passed 189 application tests plus its auxiliary binary
  tests. The one-world ownership contract passed 15 tests with its one
  separately gated GPU characterization ignored.
- Explicit GPU proofs for placed terrain and complementary half-space
  terrain passed across mono, per-eye stereo, and full-frame multiview.
  `pnpm native:xr-emulation:smoke` produced a correct side-by-side stereo
  image with 249,588 differing eye pixels.
- `pnpm native:desktop-offscreen:smoke` produced a correct 2560x1600
  full-frame image. Direct inspection confirmed textured terrain, foliage
  cutouts, lighting, depth ordering, actors, and sky with no missing arena
  geometry or corruption.
- Browser `wasm32-unknown-unknown` check and `pnpm native:web:build` passed.
  `pnpm native:android-xr:apk` built the release shared library and APK
  successfully for arm64 API 28.
- The broad `native:thin-adapters:purity` command still flags the
  diagnostics-only aerial-camera reconcile delegate added by pre-campaign
  commit `853b142e`. That known adapter-policy exception predates Tactical
  232 and is not caused by the accepted renderer or scheduler changes.
- Physical Quest execution and a new headed browser pixel receipt were not
  rerun during this Deck campaign. Android/XR packaging, headset-free stereo,
  direct multiview GPU proofs, and browser compilation passed, but those
  checks are not represented as substitutes for later physical/runtime
  acceptance.
- Decision: close the tactical. The physical Deck hypotheses, correctness
  guards, shared implementation gates, measured support policy, and next
  bottleneck order are durable. Future work starts from the cull/occlusion,
  runtime-target admission, and bursty upload/GPU-tail evidence rather than
  reopening per-section draw submission or adding unproven fluid delay.
