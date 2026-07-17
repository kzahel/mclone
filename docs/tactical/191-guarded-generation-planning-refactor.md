# Tactical 191: Guarded Generation Planning Refactor

Status: active 2026-07-17; Slice 0 benchmark harness and clean baseline in
progress. No planner or scheduler behavior has changed yet.

Topics: `world-generation-profiles`, `performance`

Workstream: shared native Rust world generation and server scheduling, with
desktop release evidence first, flat-Android AVD pacing canaries, unchanged
native-web worker contracts, and deferred physical-Quest confirmation.

## Objective

Move generator-specific dependency-footprint planning out of
`mclone-server::ChunkScheduler` without changing generation output, requested
chunk semantics, dependency readiness, job batching, worker transport,
publication pacing, lighting, persistence, or render admission.

The target boundary is intentionally smaller than a plugin system or task DAG:

```text
exact interest targets
  -> generator-owned pure plan
       requested outputs
       backend work footprint
       chunk/status prerequisites
  -> existing scheduler readiness, priority, cache, and budget policy
  -> existing worker request and result publication
```

This is a guarded ownership refactor. It does not add new island terrain,
change Overworld parity, introduce a DSL, load dynamic modules, serialize a
general plan, or unify worldgen and meshing schedulers.

## Current Evidence and Smell

The current scheduling machinery is already dependency- and budget-aware:

- interest remains an exact set of view-requested chunks;
- Overworld feature jobs expand targets to a bounded feature-center and
  mutable dependency-buffer halo;
- missing dependency holders are advanced to `ChunkStatus::Surface`;
- retained dependency buffers, priority ordering, worker capacity,
  publication grants, lighting, and final visibility stay scheduler-owned;
- flat grass and small island are target-only generator cases;
- render compilation independently waits for horizontal neighbor snapshots
  and uses dirty-neighborhood invalidation.

The ownership defect is narrower. The Overworld 3x3 feature-center and 5x5
dependency expansion is implemented both by
`mclone-server::scheduler::feature_job_positions` and by
`mclone-worldgen::levelgen::feature_batch::FeatureBatchPlan`. The scheduler
also matches concrete generation profiles to choose that footprint. A future
island cave, volcano, river, or structure pass would therefore need to teach
the scheduler generator policy rather than merely declare prerequisites.

## Locked Non-Regression Contract

Every implementation slice preserves these facts exactly unless a later
product tactical explicitly changes one:

1. Interest supplies exact requested chunks. Prerequisites never become
   client-visible or persisted target outputs merely because they were
   scheduled.
2. Overworld target, feature-center, dependency, ordering, random-draw, cache,
   generated-chunk, biome, tick, and oracle results remain unchanged.
3. Flat grass and small island remain target-only.
4. The scheduler continues to own status readiness, deduplication, priority,
   job admission, publication budgets, worker capacity, lighting, unloads,
   and persistence.
5. Planning occurs once per admitted batch. It is not repeated per poll,
   frame, target publication, or dependency-ready transition.
6. The refactor adds no worker round trip, no plan serialization, no
   per-chunk trait-object dispatch, and no general graph traversal.
7. Existing target/dependency collections are moved or borrowed across the
   seam. The refactor may remove duplicate work but must not add full-buffer
   clones, additional sorts, or equivalent transient allocation pressure.
8. Native and browser workers retain the current descriptor-keyed resident
   cache/reset contract and request/response wire shapes through this tactical.
9. No admission floor, cap, ramp, batch size, worker count, cadence, or
   platform default changes in the planner commits.

## Performance Evidence Hierarchy

Physical Quest remains the final authority for standalone Android XR pacing.
It is unavailable for this tactical's initial work, so evidence is layered:

1. **Exact behavior and schedule locks** catch changed target/dependency work
   before timing is interpreted.
2. **Raw worldgen and server scheduler release probes** catch planner cost,
   throughput, cache, retention, and poll-tail changes without GPU noise.
3. **Desktop paced streaming and movement probes** catch shared publication,
   client apply, mesh admission, upload, and frame-tail regressions.
4. **Flat-Android AVD pacing probes** exercise the production Android app,
   local integrated server, shared scene host, Vulkan surface, queues, and
   frame accountant under deterministic view churn.
5. **Quest RD5 settled-orbit and chunk-view-churn lanes** remain a deferred
   post-refactor hardware gate. AVD evidence must never be recorded as Quest
   evidence.

AVD measurements are relative canaries, not absolute mobile budgets. Emulator
CPU/GPU scheduling, host load, ABI, SurfaceFlinger, refresh behavior, and
thermal behavior differ from Quest; flat Android also lacks OpenXR,
stereo/multiview, and Meta compositor timing. Compare each emulator GPU mode
to its own before/after baseline.

### AVD GPU-mode matrix

Run the identical cold-world workload with explicit emulator modes:

- `-gpu host`;
- `-gpu auto`.

Record the app's reported Vulkan adapter for both. If both modes resolve to
the same adapter, materially different pacing is a harness/environment warning
that must be explained before the lane becomes binding. If they resolve to
different adapters, keep separate before/after envelopes rather than treating
their direct difference as a product regression.

Each run must clear app data, boot the same AVD without a saved snapshot, use
the same optimized Rust APK, stage the same asset pack, and run the same seed,
render distance, warmup, sample duration, churn interval, and churn offset.

### Metrics

Record at least:

- raw feature chunks/second and warm/cold dependency-cache behavior;
- scheduler step time, poll time/max, job counts, target/dependency counts,
  snapshots, unloads, retained dependency/light state, and RSS where present;
- playable, full-view, and render-quiescent times;
- frame/app-work average, p50, p95, p99, max, headroom, and over-period tiers;
- inbound, host-publication, render-compile, completed-result, and upload queue
  depths/maximum ages;
- frame-accounting and queue conservation violations.

Raw output belongs under `/tmp`. Durable records identify commit, dirty state,
host/AVD, ABI, GPU mode, adapter, commands, and whether a result is exploratory
or binding.

### Comparison rules

- Capture baselines and candidates on the same host and clean commit.
- Run release/optimized code for claims. Debug smokes prove liveness only.
- Treat desktop deltas below five percent as noise and rerun before judging,
  following the existing performance-topic rule.
- Establish AVD variance with repeated paired runs before selecting numeric
  gates. Until then, new over-2x/over-4x frames, persistent queue growth,
  changed work counts, conservation failures, or a large mode-specific delta
  are blockers; small timing differences are exploratory.
- Do not compensate for a candidate regression by changing a budget or worker
  default in the same slice.
- The refactor may land without an attached Quest only while it remains a
  behavior-preserving ownership change and all proxy gates pass. Record Quest
  confirmation as pending; do not promote new pacing policy from proxy data.

## Slice Plan

### Slice 0: Benchmark contract and pre-refactor baseline

- add a bounded flat-Android pacing mode that reuses the production
  `FramePipelineAccountant` and emits one concise machine-readable log marker;
- use a warmup followed by deterministic alternating chunk-view centers so
  generation, publication, client apply, meshing, uploads, and eviction are
  exercised during the sample;
- add AVD controls for `-gpu host` versus `-gpu auto`, cold app data, and
  no-snapshot boot;
- capture one initial paired AVD scouting run and the existing host release
  worldgen, scheduler, startup-streaming, and movement-frame baselines;
- record limitations and any harness defects before changing planner code.

Gate: the benchmark is bounded, emits no marker outside explicit perf mode,
reports zero conservation violations, and paired modes complete the identical
workload. No worldgen/scheduler production behavior changes in this slice.

### Slice 1: Pure plan vocabulary and exact fixtures

- introduce the smallest value vocabulary for exact outputs, backend work,
  and `(ChunkPos, ChunkStatus)` prerequisites;
- keep it synchronous, deterministic, and free of clocks, I/O, scheduler
  handles, worker handles, and priority policy;
- add exact single-target, contiguous-region, negative-coordinate, duplicate,
  reversed-order, and partitioned-target fixtures;
- lock Overworld's existing 3x3/5x5 footprint and Flat/Island target-only
  behavior;
- add schedule-accounting assertions that prerequisites cannot publish.

Gate: fixture and oracle output is exact; pre-refactor target, dependency, and
job accounting matches.

### Slice 2: One owner for Overworld dependency planning

- make `mclone-worldgen` the sole owner of Overworld feature/dependency
  footprint computation;
- remove `scheduler::feature_job_positions` and its duplicate tests;
- adapt scheduler ordering only after it receives the unordered deterministic
  plan so priority remains scheduler policy;
- prove the plan is computed once and no new collection/buffer clone appears.

Gate: exact schedule trace, cache report, oracle fixtures, raw worldgen, and
server scheduler A/B rows remain inside the locked envelope.

### Slice 3: Generic scheduler prerequisite consumption

- replace the profile-specific dependency branch in feature-job creation with
  iteration over declared chunk/status requirements;
- retain explicit closed profile dispatch behind the planning facade;
- preserve authored-only's cheap procedural bypass;
- keep job IDs, batching, worker mailbox, publication, light, and persistence
  lifecycle unchanged.

Gate: scheduler unit/integration suites, native worker codec, browser worker,
persistence, and host performance rows pass without policy adjustment.

### Slice 4: Dispatch cleanup and platform closeout

- centralize only the remaining duplicated built-in planner dispatch that has
  a concrete second caller;
- retain the closed, versioned profile enum and current worker execution
  dispatcher;
- run native/web correctness plus the paired AVD modes;
- rerun and compare host raw, scheduler, paced-streaming, and movement rows;
- record Quest RD5 orbit/churn as pending if hardware is still unavailable.

Gate: all locked contracts pass. The closeout must state whether the result is
Quest-proxy-clean or physically Quest-validated; those are not interchangeable.

## Deferred Work

- a general task DAG or DSL;
- dynamically loaded native generators;
- WASM or remote generator modules;
- generator-plan serialization;
- a Mojang-style registry/codec framework;
- shared worldgen/meshing dependency abstractions;
- new island caves, volcanoes, structures, biomes, or cross-chunk features;
- pacing-policy or worker-capacity changes.

Those directions may reuse the planning seam later, but none is justification
for enlarging this refactor.

## Validation Commands

Host baseline:

```bash
pnpm native:worldgen:perf
pnpm native:scheduler:perf
pnpm native:startup-streaming:perf
pnpm native:movement-frame:perf
```

Android paired baseline uses the Tactical 191 AVD pacing command with explicit
`--gpu host` and `--gpu auto` variants. When Quest returns, rerun:

```bash
pnpm native:android-xr:perf:orbit:rd5:metrics
pnpm native:android-xr:perf:churn:rd5:metrics
```

