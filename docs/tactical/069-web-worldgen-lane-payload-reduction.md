# 069: Web Worldgen Lane Payload Reduction (resident dependency mirror + delta)

Status: **landed (Stages 1 + 2).** This is the highest-value web
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

## Landed

### Stage 1 — request-side delta (landed)

The web worldgen worker now holds a **resident dependency session** across jobs and
the request is a **delta** against it, instead of re-shipping the seeded 529-chunk
neighbourhood every job. Web-only, behind the `WorldgenMailbox` web backend —
desktop still moves `Vec<MutableChunkBlockBuffer>` over `mpsc` with zero
serialization and is untouched.

- **Resident session (worker).** `mclone-server` gains `WorldgenJobSession`
  (`job_codec.rs`): one `OverworldFeatureDependencyCache` + a `mirror_generation`
  epoch held across jobs, with `compute_delta_job_frame`. The web client wraps it
  as `#[wasm_bindgen] WebWorldgenJobSession` (`web_server_worker.rs`), replacing the
  stateless `mclone_web_compute_worldgen_job_frame` free function; the server-job
  worker (`www/mclone-server-job-worker.js`) holds it resident
  (`worldgenSession ??= new module.WebWorldgenJobSession()`) exactly as the render
  worker holds `compilerSession`. Light-status stays stateless (separate worker
  instance).
- **Delta on submit (main).** The web `WorldgenMailboxBackend` keeps a
  `BTreeSet<ChunkPos>` shadow of the dependency columns the worker mirror holds,
  plus a mirror generation. `enqueue_features` ships only the seeded deps whose pos
  is **not** in the shadow (`encode_worldgen_delta_request`, distinct
  `WORLDGEN_DELTA_REQUEST_MAGIC`); the first job (uninitialised mirror) is a **reset**
  full-resync under a fresh generation. The shadow advances **on submit** by the
  shipped upserts (so a job enqueued before the prior job's response is drained still
  ships a minimal delta) and is **corrected on receipt** to the response's authoritative
  retained set.
- **Desync tripwire.** Each delta carries the mirror generation. A `reset` clears the
  mirror and adopts the generation; a non-reset delta whose generation does not match
  the resident mirror's is rejected loudly (`"worldgen worker mirror desync …"`) rather
  than generated against a partial mirror. Unit-tested
  (`worldgen_delta_session_rejects_generation_desync`).

**Divergence from desktop, and why parity is preserved.** Desktop recreates the cache
per job and relies on the free `mpsc` move; this resident-mirror + delta is confined to
the web mailbox transport, exactly as 067 confined render's mirror to the web compiler.
The engine's scheduler-owned-dependency contract (tactical 019) is unchanged. **Two
worldgen-specific simplifications over 067 Stage 4, both safe:** (1) **no eviction list** —
`generate_features_chunks_with_dependencies` already retains the cache to each job's plan
deps, so the worker self-bounds memory, and the shadow is resynced to the worker's
authoritative retained set from each response; (2) **correctness is delta-independent** —
dependency buffers are deterministic functions of `(seed, pos)` and are *never*
feature-mutated (the cache snapshot is cloned **before** feature decoration, which mutates
separate region clones), so any column the worker is not sent is regenerated
byte-identically. The delta only changes transport; generation is unchanged. Proven by
`worldgen_delta_session_matches_stateless_full_frame` and
`worldgen_delta_session_reuses_resident_mirror_across_jobs` (a warm job ships 0 upserts,
reuses the resident mirror for the overlap, and produces output byte-identical to a cold
stateless generation).

Measured (this device, vs the 068 baseline above):

| worldgen job | metric | baseline | Stage 1 |
|---|---|---|---|
| **warm** (chunk-smoke job 2, `report − firstReport`) | request | **27,145,638 B** | **81 B** |
| warm | response | 28,985,785 B | 28,985,785 B (unchanged — Stage 1 keeps full response) |
| cold (chunk-smoke job 1, `firstReport`) | request | 232 B | 241 B (+9 B generation/reset header) |
| movement-perf (3 boundaries, 5 jobs) | cumulative request | 107,009,128 B | **5,901,775 B** (−94.5%) |
| movement-perf | cumulative response | 152,272,922 B | 152,272,922 B (unchanged) |

The warm-job request collapses to an 81-byte header (the worker's resident mirror already
holds the entire overlapping dependency neighbourhood, so zero upserts ship). The residual
5.9 MB across the movement-perf traverse is the one-time mirror seed (the reset job ships
the deps the scheduler already held at spawn) plus the genuinely-new dependency strip at
each boundary crossing. No overflow / fallback (`sharedBufferFallbackResponseFrames` 0,
`shared-memory` transport). The deterministic chunk-smoke canvas PNG is **byte-identical**
to the baseline (sha256 match), and `cargo test` (incl. the three new
`worldgen_delta_session_*` tests), `cargo check` wasm (warning-clean), `node --check`,
`native:web:app-smoke`, `native:movement:smoke`, and `native:timedemo:smoke` are all green.

### Stage 2 — response-side delta (landed)

The worker response no longer ships the whole 529-chunk retained dependency
neighbourhood back. It ships the generated target chunks (unchanged) plus a
**response-dependency subset** — only the columns the main side actually consumes —
plus a positions-only `retained_dependency_positions` list for the shadow.

- **Response subset (`worldgen_response_subset_positions`, `job_codec.rs`).** A
  retained dependency column is shipped back only if it is **new to the worker this
  job** (a cache miss / freshly-upserted column the scheduler may not already hold —
  needed for `mark_dependency_ready` storage) **or** within the **light-neighbour
  ring** of a target (`provisional_sky_light_includes_chunk`, the 3×3 ring). The
  session snapshots `cache.resident_positions()` *before* generating to know which
  columns are new. On the stateless full-frame path (and the native `mpsc` worker)
  `resident_before` is empty, so the subset is the whole retained set (cold/desktop
  unchanged).
- **Shadow correction.** The web mailbox now sets its shadow from
  `retained_dependency_positions` (the full set, positions only ~8 B each) rather
  than the response dependency map's keys, so the shadow stays in lockstep with the
  worker's full mirror even though only a subset of buffers crosses.

**Coupling B — light — preserved by a cleaner mechanism than the proposed
holder-sourcing, and why.** 069 originally proposed rerouting light's `neighbor_blocks`
to source dependency bytes from the scheduler's resident holders. **That is not
eviction-safe:** dependency holders are scheduled (not ticketed), and
`process_pending_unloads` runs every tick while completed-job publication is
budget-limited to **1 target/tick** (`DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET`), so a
target's light-ring neighbour can be unloaded from the holders *before* that target's
light status is built — silently dropping a neighbour and changing edge sky light.
Instead, the **worker returns the light ring in the response subset**, so light sources
its neighbour bytes from the job's own returned set exactly as before. Concretely:
`publish_completed_feature_job` feeds `completed.retained_dependencies` (now the subset)
to *both* `mark_dependency_ready` and `PendingLightStatus::from_feature_publication`,
and the latter already filters to the 3×3 ring — so **the scheduler needs no change**,
because the subset is guaranteed to contain that ring. This is strictly more robust than
holder-sourcing (no eviction window) and keeps the whole change confined to the web
codec + mailbox, like Stage 1. Proven byte-for-byte by
`worldgen_delta_session_reuses_resident_mirror_across_jobs` (asserts every light-ring
dependency is present in the subset with identical bytes) and by the byte-identical
chunk-smoke canvas.

**Coupling A — scheduler-owned model — preserved.** The scheduler's holders now hold a
*subset* of dependency buffers on web (the worker mirror is a superset), but the shadow
tracks the worker's full mirror and the generation tripwire guards desync, so seeding
stays consistent: a future job seeds only what the shadow says the worker lacks, sourcing
bytes from whatever holders retain and **regenerating any gap deterministically**.
Non-subset dependency holders stay Surface-scheduled without a buffer until unloaded —
benign: they do not gate publication or settling (`pending_job_count` counts only
queued/running feature jobs + light work, not dependency holders), do not trigger feature
jobs (only Features-targeted chunks do), and are cleaned up by the normal unload path.
Desktop is unaffected (it returns the full set over `mpsc`, contains the ring).

Measured (this device, vs the 068 baseline above):

| worldgen job | metric | baseline | Stage 1 | Stage 2 |
|---|---|---|---|---|
| **warm** (chunk-smoke job 2) | request | 27,145,638 B | **81 B** | 81 B |
| **warm** | response | 28,985,785 B | 28,985,785 B | **3,217,524 B** (−88.9%) |
| cold (chunk-smoke job 1) | response | 36,330,584 B | 36,330,584 B | 36,334,820 B (+4,236 B positions list) |
| movement-perf (3 boundaries) | cumulative response | 152,272,922 B | 152,272,922 B | **55,106,044 B** (−63.8%) |

The warm response is now the generated target chunks (~1.7 MB) plus the small subset
(new columns + light ring, ~1.5 MB). It does not reach the ~1.6 MB "generated chunks
only" target from the proposal because the light ring is shipped for guaranteed light
parity rather than sourced from the eviction-prone holders — a deliberate
correctness-over-bytes trade, still an 88.9% warm-response reduction. No overflow /
fallback; the deterministic chunk-smoke canvas PNG stays **byte-identical** to the
baseline (sha256 match — light *and* terrain parity through the real render).
`cargo test` (674 passed), `cargo check` wasm (warning-clean), `node --check`,
`native:web:app-smoke`, `native:web:movement-perf` (maxFrameGap 9.3 ms),
`native:movement:smoke`, and `native:timedemo:smoke` all green.

**Combined (Stages 1 + 2), warm worldgen job:** request **27 MB → 81 B**, response
**29 MB → 3.2 MB**. The ~112 ms warm-job serde tax 068 measured (req decode 54 ms +
resp decode 54 ms) is collapsed onto deltas; the cold first load still ships its 36 MB
neighbourhood once, then deltas — render after 067 Stage 4, applied to worldgen.

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
