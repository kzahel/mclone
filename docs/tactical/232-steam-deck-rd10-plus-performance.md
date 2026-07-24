# 232: Steam Deck RD10+ Frame Pacing And Render Throughput

Status: active parent tactical, opened 2026-07-24. The physical Steam Deck
test bed, autonomous stationary/traversal matrix, and first RD5/RD8/RD10/RD13
baseline are complete. This tactical owns the ordered proof and implementation
campaign that follows from that evidence.

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
- the complete idle gate before measurement;
- recorded artifact hash, source commit/dirty state, power state, clock,
  temperature, memory, and queue state; and
- an automatic restoration of the interactive shortcut and panel-sleep
  policy.

The matrix keeps these distinct:

- **fresh stationary:** begin immediately after the complete idle gate;
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

- [ ] Attribute frame-accounting scans, traversal-ready stamp construction,
  update integration, record preparation, cull, per-layer draw encoding,
  upload publication, surface acquire, submit, present, and device wait.
- [ ] Add supported wgpu terrain/pass timestamp queries with asynchronous
  readback and explicit unsupported capability reporting.
- [ ] Add matrix controls for native/half world resolution, scheduled-fluid
  freeze, one/two/derived render workers, fresh/soaked stationary, and focused
  candidate A/B selection.
- [ ] Extend matrix summaries with GPU pass time, accounting/stamp/cull spans,
  fluid mutation/rebuild correlation, compiler occupancy/queue age, and ticket
  propagation/reconciliation time.
- [ ] Calibrate measurement overhead and retain release-shaped default rows.

Exit: H6 can be tested without using GPU busy percentage as a proxy, and every
later candidate has a named primary span.

### Slice 1: Constant-Time Frame Bookkeeping

- [ ] Replace per-frame exact `pending_render_chunk_count` set construction
  with mutation-maintained counts or cached summaries.
- [ ] Preserve an explicit exact audit for tests, settle gates, and periodic
  diagnostic verification.
- [ ] Replace full loaded-chunk-set copies in traversal-ready stamps with
  monotonic client/load and render generations.
- [ ] Prove counter/generation conservation under load, unload, dirty,
  inflight, stale-result, and removal transitions.
- [ ] Run stationary and traversal A/Bs at RD5, RD10, and RD13.

Exit: H1 is accepted or rejected. Pixel output and scheduling policy are
unchanged.

### Slice 2: Vanilla-Shaped Static Visibility Reuse

- [ ] Define a cache key containing camera/view/projection, topology, render
  options, render distance, prepared-record generation, traversal readiness,
  and any state that changes the drawable list or ordering.
- [ ] Reuse the culled/drawn section plan when the key is unchanged.
- [ ] Invalidate exactly on section add/remove/mesh/connectivity/readiness,
  camera or projection changes, and relevant option changes.
- [ ] Keep uniform writes and actual frame drawing current; cache commands or
  pixels only in a separately measured later slice.
- [ ] Validate stationary, camera nudge/rotation, chunk mutation, fluid
  mutation, render-distance change, and topology cases.

Exit: H2 is accepted or rejected. Traversal is expected to remain unchanged.

### Slice 3: Ticket Distance Reuse Then Incremental Propagation

- [ ] Instrument `active_levels`, holder reconciliation, unload processing,
  and their call counts per server tick.
- [ ] First cache the propagated map behind a ticket/topology generation and
  share one immutable result across same-generation callers.
- [ ] Prove exact equality against the current full reconstruction across
  add/remove/timeout/forced/player movement and plane/cylinder topology tests.
- [ ] Run stationary and traversal A/Bs.
- [ ] If reconstruction still matters during continuous movement, port the
  Vanilla-shaped changed-source graph propagation behind the same observable
  contract and compare it with the cached full-map control.

Exit: H3 is accepted or rejected without changing holder levels, generation
order, unload decisions, or published chunks.

### Slice 4: Fluid-To-Remesh Attribution And Coalescing

- [ ] Add the frozen-fluid stationary rows and record executed ticks, mutated
  blocks, dirty sections, superseded/stale compiles, rebuilt/uploaded sections,
  and visible/offscreen classification.
- [ ] Identify whether repeated dirtying, neighbor fan-out, stale completions,
  or unchanged render facts cause redundant meshes.
- [ ] Coalesce revisions and supersede obsolete work at existing shared dirty
  and compile-queue boundaries.
- [ ] Avoid only provably unchanged render work; preserve Vanilla fluid
  simulation and client-visible state.
- [ ] Validate cross-section and cross-chunk water/lava boundaries as well as
  fresh/soaked stationary Deck rows.

Exit: H4 is quantified, and any kept fix reduces redundant work rather than
silencing simulation.

### Slice 5: Render Compiler Capacity

- [ ] Compare one worker, two workers, and shared derived capacity with equal
  max-pending policy.
- [ ] Record worker busy time, queue age/depth, completed/stale results,
  publication/application pressure, frame tails, and total settle/travel work.
- [ ] Keep a new default only if queue evidence and multiple rows show a
  repeatable net win without starving the render or server critical paths.

Exit: H5 is accepted or rejected. Available core count alone is not evidence.

### Slice 6: Moving-View Spatial Culling

- [ ] Separate candidate-section scan cost from accepted traversal and draw
  encoding.
- [ ] Prototype region/column bounding volumes or another generation-cached
  hierarchy over prepared records.
- [ ] Preserve current frustum, readiness, occlusion-connectivity, topology,
  and observer-nearest lift semantics.
- [ ] Measure top-down/oblique traversal at RD5/RD10/RD13 and reject the
  hierarchy if maintenance cost erases moving-view savings.

Exit: H7 is accepted or rejected. Static cache performance is evaluated
separately.

### Slice 7: Reduced Draw Submission

- [ ] Use H6, GPU timestamps, cull spans, and per-layer draw counts to choose
  between render bundles, shared vertex/index arenas with indirect draws, or
  a smaller state-change batching step.
- [ ] Keep per-section unique geometry while moving chunk transforms and draw
  ranges into stable GPU-visible records.
- [ ] Preserve opaque/cutout ordering, translucent back-to-front behavior,
  mesh replacement, stale-result rejection, topology lifts, and bounded
  allocation.
- [ ] Implement mono first only behind a diagnostic toggle, then cover
  per-eye and full-frame multiview before enabling it as shared policy.
- [ ] Compare CPU encode, GPU terrain time, residency, uploads, frame tails,
  and visual output across the full Deck matrix.

Exit: H8 is accepted or rejected. A path that merely moves CPU time into GPU
time without improving frame pacing is not a win.

### Slice 8: Closeout And Durable Policy

- [ ] Run the complete stationary/traversal matrix on a clean SteamRT4
  artifact, including fresh, soaked, fluid-control, resolution-control, and
  selected worker rows.
- [ ] Record before/after tables, thermal/run variance, failed hypotheses,
  reverted experiments, and remaining ceilings here and in the Steam Deck
  topic.
- [ ] Update `docs/topics/performance.md` with the new priority order.
- [ ] Re-run shared platform gates for every changed owner and record any
  pending physical Quest/browser/Android acceptance honestly.
- [ ] Decide supported Deck render-distance/default policy from measured
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
