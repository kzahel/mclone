# Tactical 191: Guarded Generation Planning Refactor

Status: active 2026-07-17; Slices 0-2 are complete. Slice 3 is next. Worldgen
now solely owns the Overworld footprint calculation; scheduler policy and
observable work remain unchanged.

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

#### Slice 0 result: clean baseline at `0ae6b924`

The baseline was captured on 2026-07-17 from clean commit `0ae6b924` on an
arm64 Apple M4 Pro host running macOS 26.5.1. Raw JSON and logcat output remain
under `/tmp/mclone-191-*`; those transient paths are evidence pointers, not
repository artifacts.

The new Android canary is deliberately bounded and opt-in. It resets the
production `FramePipelineAccountant` after a five-second warmup, alternates
the authoritative mono camera between chunk X 0 and 16 every three seconds,
samples for 15 seconds at RD5 and seed 12345, and emits exactly one compact
`MCLONE_ANDROID_PACING_PERF_SUMMARY` marker. The paired wrapper cold-boots the
same `jstorrent-tablet` arm64-v8a AVD without a saved snapshot and clears app
data before each mode.

The GPU-mode hypothesis did not hold: `-gpu host` and `-gpu auto` resolve to
different Vulkan adapters on this AVD and produce materially different pacing
and pixels.

| AVD mode | Vulkan adapter | Frames | App avg | App p95 | App p99 | Max | > period | >2x / >4x | Queue depth / age |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `host` | Apple M4 Pro | 901 | 16.516 ms | 19.180 ms | 24.491 ms | 53.332 ms | 40.07% | 1 / 0 | 186 / 2,424 ms |
| `auto` | SwiftShader Device | 331 | 45.516 ms | 81.208 ms | 88.853 ms | 138.477 ms | 99.09% | 243 / 53 | 238 / 11,499 ms |

Both modes reported zero frame- and queue-conservation violations. `auto` was
about 2.76x slower by average app work and 4.23x slower at p95. The inspected
`host` screenshot rendered the terrain normally; the inspected `auto`
screenshot contained large black regions. Logcat contained no fatal or wgpu
validation error, while the emulator reported SwiftShader unsupported Vulkan
extension warnings. This is an emulator/backend distinction, not evidence of
an Mclone planner regression.

Consequently, `host` is the primary hardware-Vulkan AVD pacing canary for this
series. `auto` remains a separate software-backend compatibility/stress lane.
Each candidate is compared only with the matching adapter baseline; their
numbers must not be averaged or treated as interchangeable product evidence.
One paired scouting sample does not establish binding numeric timing gates,
but changed work, conservation failures, persistent queue growth, or new
severe-frame tiers remain blockers.

The existing release host baselines completed from the same clean commit:

| Probe | Pre-refactor result |
|---|---|
| raw worldgen, radius 1 | surface 701.859 chunks/s; features cold 116.515 target chunks/s; features warm 471.115 target chunks/s |
| raw worldgen dependency cache | cold 49 requested / 0 hit / 49 generated / 49 retained; warm 49 / 49 / 0 / 49 |
| scheduler, three radius-1 steps | 670.358 ms total; step max polls 0.657 / 0.382 / 0.370 ms; 3 feature jobs; 9 + 3 + 3 snapshots; 0 + 3 + 3 unloads; peak RSS 56,768 KiB |
| scheduler exact work | initial job 25 targets / 49 feature centers / 81 dependencies; moved jobs each 5 / 21 / 45; final cache 72 hits / 99 misses / 171 retained |
| startup streaming, RD10 at 60 Hz | playable 514.305 ms; full view 6,147.597 ms; render quiescent 49,647.138 ms; frame avg/p95/p99/max 4.916/7.421/9.063/14.315 ms; 0 over budget |
| startup queue/work | max scheduler publish poll 11.239 ms; 1,280 feature chunks and 1,446 light statuses published; 7,917 submitted / 7,907 completed compiles; 2,970 uploads; 0 update-pump stalls |
| movement, RD5 at 120 Hz | 240 frames; avg/p95/p99/max 2.231/3.047/3.300/3.388 ms; 0 over budget; frame-accounting app-work p95 1.504 ms |

Startup streaming ended with 529/529 target chunks ready, no pending server
jobs or publications, and an empty update queue. Both paced host probes
reported zero frame-accounting conservation violations. The startup lane's
standard 6,000-frame run continued after render quiescence and ended at
130,516 ms; the quiescence timestamp, not final wall time, is the comparison
metric.

Slice 0 therefore passes as an initial regression baseline. Physical Quest
RD5 orbit/churn remains pending and is still the final standalone-XR pacing
authority.

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

#### Slice 1 result

`mclone-worldgen::levelgen::ChunkGenerationPlan` now holds three deterministic
sets: exact requested outputs, generator backend-work chunks, and typed
`ChunkStatusRequirement` prerequisites. It has no clocks, I/O, scheduler or
worker handles, priorities, admission policy, serialization, or dynamic
dispatch. Overworld and target-only constructors establish the existing
3x3/5x5 and independent-chunk shapes without changing a production caller.

Exact fixtures cover a single target, a contiguous radius-one region,
negative coordinates, duplicate/reversed targets, partitioned-target union,
and target-only Flat/Island semantics. A server-side equivalence assertion
compares the vocabulary with the still-live scheduler calculation, and a
separate assertion proves scheduling all 25 Surface prerequisites for one
target leaves every holder non-client-visible.

Validation at the Slice 1 commit boundary:

- all 220 active `mclone-worldgen` library tests passed (one pre-existing
  parity gauntlet remains ignored);
- all 468 `mclone-server` library tests passed;
- formatting and diff checks passed.

This is vocabulary and executable contract only. Slice 2 performs the first
production ownership move.

### Slice 2: One owner for Overworld dependency planning

- make `mclone-worldgen` the sole owner of Overworld feature/dependency
  footprint computation;
- remove `scheduler::feature_job_positions` and its duplicate tests;
- adapt scheduler ordering only after it receives the unordered deterministic
  plan so priority remains scheduler policy;
- prove the plan is computed once and no new collection/buffer clone appears.

Gate: exact schedule trace, cache report, oracle fixtures, raw worldgen, and
server scheduler A/B rows remain inside the locked envelope.

#### Slice 2 result

Commit `9d0ceb07` removed `scheduler::feature_job_positions`. Both Overworld
feature execution/cache retention and scheduler job preparation now call the
single `mclone-worldgen` plan implementation. The scheduler receives unordered
deterministic sets and still applies its existing view-priority order afterward.
Flat and Island still use the same target-only shape. No worker request,
response, job, publication, or cache wire field changed.

All 220 active worldgen tests and all 468 server library tests passed again,
including the Java scheduler-trace order and generated-output fixtures. The
clean release A/B from baseline `0ae6b924` to candidate `9d0ceb07` recorded:

| Metric | Baseline | Slice 2 | Result |
|---|---:|---:|---|
| raw feature cold | 116.515 chunks/s | 138.725 chunks/s | no regression |
| raw feature warm | 471.115 chunks/s | 468.922 chunks/s | -0.47%, noise |
| scheduler total | 670.358 ms | 670.299 ms | flat |
| scheduler step max polls | 0.657 / 0.382 / 0.370 ms | 0.520 / 0.420 / 0.342 ms | no tail regression |
| peak RSS | 56,768 KiB | 56,624 KiB | flat |

Exact work was identical: jobs remained `25/49/81`, then `5/21/45` twice for
targets/feature centers/dependencies; snapshots remained `9/3/3`, unloads
`0/3/3`, cache totals `72` hits / `99` misses / `171` retained, and the final
client-visible/loaded/dependency-holder/ready-dependency counts remained
`9/35/806/99`. Raw worldgen cold and warm cache rows stayed exactly
`49/0/49/49` and `49/49/0/49`, and both generated the same 191,610 non-air
feature blocks. Slice 2 therefore passes its behavior and raw-performance gate.

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
