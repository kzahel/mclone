# 069: Web Worldgen Lane Payload Reduction (resident dependency mirror + delta)

Status: **proposed — measured, ready to implement.** This is the highest-value web
performance improvement surfaced by the
[`068`](068-web-zero-copy-worker-lane-investigation.md) zero-copy investigation. It
is the **stable-toolchain** alternative that 068 recommends over a shared-Wasm-
linear-memory runtime: attack the worldgen **payload** (stop re-serializing the
529-chunk dependency neighbourhood through main every job) rather than the copy.
062 lineage; reuses the proven [`067`](067-shared-render-worker-architecture.md)
Stage 4 resident-mirror + delta pattern.

## Problem (measured)

On web, every worldgen feature job round-trips the scheduler-owned dependency
neighbourhood through `SharedArrayBuffer` as serialized bytes, because the web
worldgen worker is **stateless per job** (`compute_worldgen_job_frame` does
`OverworldFeatureDependencyCache::new()` every call). Desktop pays ~0 for the same
bounce — it *moves* `Vec<MutableChunkBlockBuffer>` over `mpsc`.

Baseline (`pnpm native:web:chunk-smoke`, this device, `shared-memory` transport;
host byte-exact codec bench reproduces these payloads to the byte — see 068):

| Worldgen job | request | response | serde (host) | serde fraction |
|---|---|---|---|---|
| cold (first load, 25 targets) | 232 B | **36.3 MB** | 56 ms (resp decode) | ~12% (work-bound: 398 ms generate) |
| **warm (movement)** | **~35 MB** (deps fed back in) | **36.3 MB** | **112 ms** (req decode 54 + resp decode 54) | **~77%** |

The warm job generates almost nothing new (~34 ms, mostly cache hits) yet pays
**~112 ms of pure serialize+copy** to ship the 529-chunk dependency neighbourhood
in and back out. This is off the render frame path (render holds sub-frame gaps,
067 FU2), so it is **chunk-load latency on movement**, not jank — but it is the
single biggest web serde cost, and it recurs on every boundary crossing.

## Root cause and the two coupling points

The dependency neighbourhood is **scheduler-owned** (tactical 019). Per job
(`scheduler.rs`):

1. `create_feature_job` → `seeded_dependency_buffers(&dependency_chunks)` **clones**
   the relevant held dep buffers and `enqueue_features(job_id, seed, targets,
   seeded_dependencies)` ships them to the worker (`scheduler.rs:1678-1691,
   1888-1907`).
2. The web worker rebuilds a fresh cache from those deps, generates, and returns
   **`retained_dependencies`** (the whole neighbourhood) back.
3. `publish_completed_worldgen_jobs` stores them back via
   `mark_dependency_ready(dependency)` (`scheduler.rs:1811-1812`).

So the scheduler **already holds these buffers resident** between jobs; the web
worker re-receives and re-returns them only because it discards its cache each job.

**Two coupling points any delta scheme must respect:**

- **A. Scheduler-owned 019 model.** The scheduler holds the dep buffers in its
  dependency holders and re-seeds them per job. A web-only resident worker cache
  must stay consistent with the scheduler's holders, including **evictions** (a dep
  unloaded on the scheduler side while the worker still mirrors it). 067 Stage 4
  solved the identical hazard with a mirror **generation/epoch** + eviction
  tracking + a desync tripwire (reject a non-reset delta whose generation does not
  match; full resync on any worker-reported failure). Copy that discipline.
- **B. Light input reads the returned dep set.** `publish_completed_worldgen_jobs`
  builds light's `neighbor_blocks` from the **job's returned**
  `retained_dependencies` (`PendingLightStatus::from_feature_publication(..,
  retained_dependencies.iter())`, `scheduler.rs:1786`). If the worker stops
  returning the full set, light neighbour construction must instead source dep
  bytes from the scheduler's **resident holders** (which already hold the full
  set). Reroute B before/with the response-side delta, or light breaks.

Desktop must keep its `mpsc`-move path untouched (no serialization, no mirror) —
confine all of this behind the `WorldgenMailbox` web backend, exactly as 067
confined the render mirror to `WebRenderSectionCompiler`.

## Target design (067 Stage 4, applied to worldgen)

- **Worker:** keep one resident `OverworldFeatureDependencyCache` (and its dep
  buffers) across jobs instead of `::new()` per call. Apply each job's delta to it
  before generating; generate from the resident cache.
- **Main (web `WorldgenMailbox`):** keep a shadow of which dep chunks (pos +
  revision/identity) the worker's cache holds, plus a mirror generation. Diff the
  scheduler's seeded deps against the shadow; ship only the delta (new/changed dep
  columns + evictions + generation). Advance the shadow on submit (apply-on-submit,
  so a requeued job ships a minimal delta).
- **Codec:** add a delta request frame (distinct magic, mirror generation, upsert
  list, eviction list) alongside the existing full-frame request (kept as the reset
  / first-job / post-failure full-resync path). Mirror 067's `MCWRCD1`-style delta
  frame and its reset semantics.

## Staged implementation (land + measure each)

**Stage 1 — request-side delta (lower risk; ~½ the win).**
Worker keeps the resident cache; main ships only the seeded deps the worker does
not already hold. Response unchanged (still full). Kills the ~27–35 MB request
bounce (~54 ms warm-job decode). No light change needed (coupling B untouched —
the worker still returns the full set). Measure: warm-job `requestBytes` collapses
toward a small delta; warm round-trip drops by roughly the request-decode time.

**Stage 2 — response-side delta (needs coupling B reroute).**
Worker returns only **newly generated** dep buffers (not the seeded ones the
scheduler already holds); reroute light `neighbor_blocks` to source dep bytes from
the scheduler's resident holders instead of the job's returned set. Kills the
36 MB response bounce (~54 ms warm-job decode, ~36 MB alloc/job). Measure: warm-job
`responseBytes` collapses toward the new-chunk set (~1.6 MB).

After both stages, a warm worldgen job should look like render after 067 Stage 4:
small delta in, small result out, work-bound (the irreducible cold-load generation
stays ~400 ms but ships its 36 MB once, then deltas).

## Acceptance & validation

- **Regression fence = the 068 baseline.** Re-run `pnpm native:web:chunk-smoke` and
  `pnpm native:web:movement-perf`; assert warm-job `worldgenJobFrameMetrics`
  request/response bytes drop sharply vs the baseline table above, with no overflow
  / fallback and the same rendered terrain (inspect the canvas PNG in `/tmp`).
- **Correctness:** generated terrain + features must be byte-identical to today
  (worldgen is parity-critical; the delta only changes *transport*, not generation).
  Add a desync-tripwire test (mismatched generation → reject, full resync) mirroring
  067 Stage 4's mirror tests.
- **Desktop must not regress:** desktop `WorldgenMailbox` keeps `mpsc` move and the
  full-frame path; run `pnpm native:movement:smoke` and `pnpm native:timedemo:smoke`
  plus `cargo test --manifest-path native/Cargo.toml`.
- **Web gates:** `cargo check -p mclone-web-client --target wasm32-unknown-unknown`,
  `node --check` on touched `www/*.js`, `pnpm native:web:build`,
  `native:web:app-smoke`, `native:web:chunk-smoke`.
- `wasm32-unknown-unknown` stays the target; **no nightly / `build-std`** — that is
  the whole point of preferring this over 068's shared-linear-memory option.

## Secondary follow-up (lower priority, separate doc when taken)

**Light worker residency.** The native light worker retains
`RetainedInitialLightState` across batches; the wasm worker recreates it per job
(`compute_light_status_job_frame` → `RetainedInitialLightState::new()`). Porting the
native residency to wasm could cut repeated `world_init`/setup work. **But light is
work-dominated** (068: ~98% work, ~2.4% serde), so this targets *work*, not the
serde tax, and its value is unmeasured — do not bundle it with this tactical. Take
it only if light *latency* is shown to matter on a slower device, with its own
before/after measurement.

## Relationship to other tacticals

- [`068-web-zero-copy-worker-lane-investigation.md`](068-web-zero-copy-worker-lane-investigation.md)
  — the measurement that chose this over shared linear memory; its Staged plan
  Stage 1 is this doc.
- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  — Stage 4 (resident snapshot mirror + delta-only request) is the template; reuse
  its mirror generation / eviction / desync-tripwire discipline.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) — parent;
  this is payload reduction on its worldgen lane, the recommended next step after
  the Stage 5 shared-memory queues.
- Tactical 019 (scheduler-owned dependency holders) — the model coupling point A
  must respect.
