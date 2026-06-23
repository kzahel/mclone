# 068: Web Zero-Copy Worker-Lane Investigation (shared Wasm linear memory)

Status: **MEASURED — NO-GO for now.** This is a scoping/measurement tactical in
[`062`](062-shared-threading-topology.md)'s lineage (shared threading topology),
**not** a render follow-up — [`067`](067-shared-render-worker-architecture.md) is
landed and closed, and render is explicitly excluded as a target (it is already
frame-budget-healthy). The deliverable is a per-lane cost decomposition and a
go/no-go with numbers. The outcome is a measured **decline**: the heavy browser
job lanes are **work-dominated**, the one lane where serialization dominates has a
cheaper toolchain-free fix, and the shared-linear-memory build cost (permanent
nightly + `-Z build-std`) is severe. A staged plan and revisit conditions are
recorded below. No runtime was built (Phase 2 was correctly skipped because Phase
0/1 did not justify it).

## Question

"Zero copy" here means: instead of today's transport — N independent wasm
instances (main + one per worker lane), each with a **private** linear memory,
communicating by **serializing into explicit `SharedArrayBuffer` job buffers**
(encode → copy into SAB → worker copies SAB into its wasm heap → decode, both
directions) — build the engine wasm with a **shared** linear memory (the wasm
threads model: a `SharedArrayBuffer`-backed `WebAssembly.Memory`, one module
instantiated across workers), so engine data lives in one shared heap and crosses
threads **by pointer**: no serialize, no copy — exactly what desktop already does
by moving owned `Vec`/`BTreeMap` over `mpsc`.

Three facts framed the investigation (all re-verified against the live code, not
assumed):

1. **Desktop is already zero-copy.** Native worldgen/light/render workers receive
   owned data over `mpsc` and move results back with zero serialization
   (`worldgen_mailbox.rs` native backend; `light_mailbox.rs` native backend). The
   serialize+copy tax is **web-only**, so the win ceiling for any zero-copy work
   is exactly "the serialize+copy wall-time the web lanes currently spend."
2. **Render is not where the bytes or the time are.** Render-compile is ≤1.75 MB
   packed, 4.3–7.2 ms round-trip, 9.3 ms worst-case frame gap under a 12-boundary
   burst (067 follow-up 2). The bytes and the latency are in the **worldgen and
   light lanes** (062's lanes). Those are the only targets.
3. **The browser shared-memory env is proven, but the engine wasm does not use a
   shared linear memory.** Toolchain is **stable Rust, plain
   `wasm32-unknown-unknown`, no `rust-toolchain` pin, no `.cargo` atomics/
   shared-memory flags** (verified). Each lane's wasm instance has a private heap;
   the existing `SharedArrayBuffer` lanes are explicit job buffers, not a shared
   linear memory.

## TL;DR decision

**Do not adopt shared Wasm linear memory now.** Measured per-lane serialize+copy
ceiling (fraction of job wall-time that zero-copy could remove):

| Lane / scenario | serialize+copy ceiling | verdict |
|---|---|---|
| **Light** (any) | **~2–5%** | marginal — work-dominated; decline |
| **Worldgen, cold** (first load) | **~12%** | modest — work-dominated; decline |
| **Worldgen, warm** (movement) | **~77%** | large — but a cheaper toolchain-free fix exists |

The only place serialization dominates is the **warm worldgen job**, and its tax is
the 529-chunk dependency neighbourhood being re-serialized through main every job.
That is better attacked by **payload reduction** (a resident worker-side dependency
mirror shipping per-job deltas — the proven [`067`](067-shared-render-worker-architecture.md)
Stage 4 pattern) on the **current stable toolchain**, which eliminates the payload
itself rather than just copying it faster, and beats zero-copy on wall-time. See
**Staged plan** below.

## Methodology

Two measurement surfaces, cross-checked:

- **Browser baseline** (`pnpm native:web:chunk-smoke`, this device,
  cross-origin-isolated, `shared-memory` transport): the real per-lane
  `WorkerFrameMetrics` — request/response bytes and round-trip `lastRequestUs`.
  This is the in-browser, real-wasm ground truth for payload sizes and end-to-end
  latency.
- **Byte-exact host micro-benchmark** (temporary, `mclone-server`, release): drives
  the *actual* job codecs (`encode_worldgen_request` / `compute_worldgen_job_frame`
  / `decode_worldgen_response`, and the light equivalents) over a request built the
  *same way the scheduler builds it* (25 targets at the origin, seed 12345;
  `GeneratedChunk::to_chunk_snapshot` + `PendingLightStatus::from_feature_publication`
  for the light batch). It times **encode / decode / work** separately —
  the split the browser's single `lastRequestUs` cannot give. Code + commands in the
  Reproducibility appendix.

**Why a host harness for the CPU split, not in-browser timers:** the host bench
reproduces the browser payloads **to the byte** (worldgen response 36,330,584 B;
light request 24,574,311 B; light response 573,770 B — all identical to the
chunk-smoke `firstReport`), so it exercises identical serialization work, with
`std::time::Instant` precision instead of the browser's coarse, COOP/COEP-clamped
clock. The result is then scaled to wasm and checked against the in-browser
round-trip (below).

**Existing instrumentation gap (important).** The live `lastRequestUs` is **not**
a clean measure of anything: `WasmJobWorker::post_frame` starts the timer *after*
`post_shared_frame` (so it excludes main-side encode **and** copy-in), and the
message closure stops it *before* `shared_response_frame` (so it excludes main-side
copy-out **and** decode). It captures roughly `schedule + worker(decode + work +
encode + copy-to-SAB) + schedule`. The true job wall-time is **larger** than
`lastRequestUs` by the main-side decode (the 36 MB worldgen response decode is
~54 ms). This is why the decomposition is built from the codecs, not from
`lastRequestUs`.

**Copy model.** Encode/decode are CPU (measured). The SAB copies are pure memcpy:
the request pays ~2× its bytes (main `copy_from`→SAB, then bindgen `Uint8Array`→
wasm), the response ~3× (bindgen wasm→JS, JS `.set`→SAB, main `to_vec`→`Vec`).
Host memcpy measured at 37–63 GB/s; a conservative wasm estimate of ~10 GB/s is
used for the wasm copy addend.

**wasm scaling cross-check.** Host light work = 103 ms; browser light round-trip
(`firstReport`) = 198 ms ⇒ wasm factor ≈ 1.9× on identical 24.6 MB input. Host
worldgen cold work+encode ≈ 400 ms; browser worldgen round-trip = 698 ms ⇒
factor ≈ 1.7×. The conclusions below are robust to this factor (and to the larger
~5× memcpy penalty), because the *fractions* move by single-digit points under it.

**Caveat — device.** Captured on fast desktop hardware (Apple Silicon). The 067
FU2 revisit condition was "a slower device." A mid/low device would scale **work
and serde together** (both are CPU); the memcpy addend would grow fastest. This
does not change which lanes are work- vs serde-dominated, but it raises every
absolute latency. Re-measure on a throttled/mobile device before any future
reversal.

## Baseline — browser per-lane metrics (chunk-smoke, this device)

Transport `shared-memory`, pooled responses, zero fallback on the normal path.
`lastRequestUs` excludes main-side encode/copy-in/copy-out/decode (see above).

| Lane | scenario | request B | response B | round-trip |
|---|---|---|---|---|
| worldgen | `firstReport` job 1 (cold) | 232 | **36,330,584** | 698 ms |
| worldgen | `report` job 2 (warm) | ~27 MB | 36,330,584 | 144 ms |
| worldgen | stress fallback probe | 104 | 30,501,823 | 582 ms |
| light | `firstReport` (cold) | **24,574,311** | 573,770 | 198 ms |
| light | `report` job 2 | — | — | 68 ms |
| light | stress fallback probe | 8,015,904 | 147,017 | 107 ms |
| render | (067 FU2) | 24 B–122 KB | ≤1.75 MB | 4.3–7.2 ms |

The handoff's "104 B → 30.5 MB worldgen" and "8 MB light / 103 ms" came from the
smaller `sharedTopologyStress.fallbackRunner` probe; the **real** first-load jobs
are bigger (36.3 MB worldgen response, 24.6 MB light request). The single
worldgen `firstReport` job moves **36 MB** and the warm `report` job's request is
**~27 MB** — the dependency neighbourhood bouncing back in.

## Per-lane cost decomposition (host, byte-exact, release)

### Worldgen — 25 targets at origin → 25 generated chunks + **529 retained dependency chunks** (36.3 MB response)

| phase | cold job (targets-only req) | warm job (deps fed back) |
|---|---|---|
| encode request | ~0.000 ms (232 B) | 2.40 ms (34.7 MB) |
| **decode request** (worker) | ~0 ms | **53.9 ms** (34.7 MB) |
| **work** (generate) | **398 ms** | **33.6 ms** (cache hits) |
| encode response (worker) | 1.87 ms (36.3 MB) | 1.87 ms |
| **decode response** (main) | **54.2 ms** (36.3 MB) | 54.2 ms |
| serde subtotal (CPU, no copy) | **56.1 ms** | **112.4 ms** |
| **serde / (serde+work)** | **12.3%** | **77.0%** |

- **Cold (first load) is work-dominated** — 398 ms generating terrain+features for
  25 targets and their 529-chunk dependency neighbourhood; serialization is only
  the 36 MB response decode (54 ms) ⇒ **~12%**.
- **Warm (movement) is serde-dominated** — generation is mostly cache hits (34 ms),
  but the 529-chunk dependency neighbourhood is re-serialized **both directions**
  (decode 34.7 MB request 54 ms + decode 36.3 MB response 54 ms) ⇒ **~77%**. This
  tax exists *only* because the dependency neighbourhood round-trips through main
  each job.

### Light — 25-status batch (24.6 MB request, real terrain + 3×3 neighbour grid)

| phase | time |
|---|---|
| encode request (24.6 MB) | 1.87 ms |
| decode request (worker, 24.6 MB) | 0.54 ms |
| **work** (propagation, worker `total_us`) | **103 ms** |
| decode response (main, 573 KB) | 0.14 ms |
| serde subtotal (CPU, no copy) | **2.55 ms** |
| **serde / (serde+work)** | **2.4%** |

The 24.6 MB light request looks alarming but is **almost free to serialize**: it is
dominated by raw `u8` block vectors (`raw_blocks` + the 3×3 `neighbor_blocks`),
which encode/decode as bulk `extend_from_slice`/`to_vec` — pure memcpy at ~50 GB/s.
Light is **97.6% work** (BFS sky/block propagation). Zero-copy cannot touch it.

### Adding the SAB copy addend (wasm estimate)

| lane / scenario | serde (CPU, wasm ≈1.8×) | copy (memcpy, wasm ~10 GB/s) | work (wasm) | **ceiling** |
|---|---|---|---|---|
| light | ~4.6 ms | ~5 ms (50 MB) | ~196 ms | **~5%** |
| worldgen cold | ~101 ms | ~11 ms (109 MB) | ~677 ms | **~14%** |
| worldgen warm | ~202 ms | ~18 ms (178 MB) | ~57 ms | **~79%** |

The copy addend does not change any verdict.

## The zero-copy ceiling, stated plainly

Shared linear memory removes **all** of encode + copy-in + worker-decode +
worker-encode + worker-copy-to-SAB + main-copy-out + main-decode — i.e. the entire
"serialize+copy" column — leaving only **work + schedule** (exactly desktop's
`mpsc`-move shape). So the achievable win **is** the serde+copy fraction above:

- **Light: ~2–5%.** Marginal. A 103 ms light job would drop to ~100 ms.
- **Worldgen cold: ~12–14%.** Modest. A 700 ms first-load job would drop to
  ~600 ms — still dominated by 400 ms of generation that no transport can remove.
- **Worldgen warm: ~77–79%.** Large in fraction. A ~145 ms warm job's serde could
  be removed — but see the cheaper alternative.

Crucially, these jobs run **off the client frame path** (the whole point of 062).
Render holds sub-frame gaps (9.3 ms, 067 FU2) regardless, so the worldgen/light
serde tax does **not** drop frames — it adds to **chunk-load latency** (time from
"I moved" to "the new column is visible"). That is a real but secondary,
non-janking cost.

## Phase 1 — implementation cost of shared linear memory (researched, current)

This is the load-bearing reason the modest/marginal lanes are a decline and even
the warm-worldgen win is not worth it via this mechanism. Sourced from the
wasm-bindgen guide, `wasm-bindgen-rayon`, the wasm threads proposal, and rust-lang
docs/issues (late-2025/early-2026):

- **Permanent nightly + `-Z build-std`, no stable path.** `wasm32-unknown-unknown`
  ships a std built **without** atomics, so threading requires rebuilding std with
  `+atomics` — and `-Z build-std` is nightly-only with **no stabilization**. Adds a
  **pinned nightly** (e.g. `wasm-bindgen-rayon` tracks `nightly-2025-11-15`) +
  `rust-src` permanently to CI, plus the flag block
  `-C target-feature=+atomics,+bulk-memory,+mutable-globals` and link args
  `--shared-memory --import-memory --max-memory=<N>` + the TLS exports. Nightly +
  build-std + atomics breaks periodically (e.g. rust #145101, Aug 2025) → must pin,
  not float. **Today the project has no toolchain pin and builds web on stable**;
  this is the single biggest blast-radius item.
- **Allocator becomes a global lock.** With one shared heap, std's `dlmalloc` takes
  a lock on every `malloc`/`free`. Alloc-heavy parallel code can scale **negatively**;
  the main thread (which cannot `Atomics.wait`) can occasionally throw on allocation
  under contention. Mitigation is a sharded allocator (mimalloc) — more scope.
- **`wasm-bindgen`'s `WasmRefCell` is not thread-safe.** Passing `#[wasm_bindgen]`
  objects across threads is a hazard, and this engine leans heavily on
  `Rc<RefCell<…>>` (e.g. `SharedRetainedLightWorld(Rc<RefCell<…>>)`, the entire
  `WasmJobWorker` plumbing). Under a shared linear memory the inverted model
  ("globals are per-thread TLS, the heap is shared") makes any accidental cross-thread
  share UB. This is real `unsafe` shared-state discipline, not the current safe
  serialize-by-value.
- **Max memory declared up front.** `--shared-memory` forbids growth past a declared
  `--max-memory` (1 GiB is the common default). The worldgen lane alone uses a 64 MB
  resident response buffer today; a shared heap must hold every lane's working set at
  once. **iOS Safari caps wasm memory far more aggressively than desktop** — a desktop
  max may not instantiate on iPhone (a future target per `CLAUDE.md`).
- **Desktop must not regress.** Desktop's zero-copy `mpsc` path must stay on stable
  and untouched. A shared-memory web build must be a separate, isolated lane.
- **Deployment.** COOP/COEP is already required here (good), but stays mandatory.
- **Playbox has no precedent.** The sibling engine is native-only (stable toolchain,
  no wasm-bindgen / atomics / build-std) — no pattern to lift.

## The cheaper, toolchain-free alternative (recommended)

The only lane where serialization dominates — the **warm worldgen job** — is heavy
because the **529-chunk dependency neighbourhood round-trips through main every
job** (request re-feeds it in, response re-ships it out). That is a **payload**
problem, and 067 already solved the identical shape for render in **Stage 4**: give
the worker a **resident mirror** of the data and ship only the **per-job delta**.
Applied to worldgen (web-only, behind the existing `WorldgenMailbox` seam, on the
current stable toolchain):

- The web worldgen worker keeps the `OverworldFeatureDependencyCache` (or just its
  retained-dependency buffers) **resident across jobs**. Main keeps a `(ChunkPos →
  revision)` shadow of what the worker holds (exactly `WebRenderSectionCompiler`'s
  shadow in 067 Stage 4).
- Each job's request ships only **new targets + the dependency columns the worker
  does not already have** (a delta), and the response ships only the **newly
  generated chunks**, not the whole 529-chunk neighbourhood.
- Expected effect, by analogy to render Stage 4 (which cut request input from
  ~206 KB to 24 B–122 KB): the warm-job **request collapses from ~35 MB to a small
  delta** and the **response from 36 MB toward ~1.6 MB** (the 25 new chunks). That
  removes the warm-job serde tax **and** the allocation/copy of 70 MB/job — a
  strictly larger wall-time win than zero-copy, with **no nightly, no build-std, no
  `unsafe` shared-state, no allocator/`WasmRefCell`/max-memory hazards**.
- **Parity note (per `CLAUDE.md` reference-porting policy).** Desktop recreates the
  cache per job and relies on the free `mpsc` move + the scheduler-owned dependency
  model (tactical 019); this divergence is confined to the **web mailbox transport**
  (a resident mirror + delta), exactly as 067 confined render's mirror to the web
  compiler. The engine's scheduler-owned-dependency contract is unchanged, so the
  parity path stays clear.

The **light** lane needs neither zero-copy nor this: it is work-dominated. (If light
job *latency* ever matters, the lever is the **work** — note the native light worker
already retains `RetainedInitialLightState` across batches while the wasm worker
recreates it per job; porting that residency to wasm could cut repeated
`world_init`/propagation setup. That is a 062 lane-residency improvement, independent
of zero-copy, and out of scope here.)

## Decision & staged plan

**No-go on shared Wasm linear memory now.** Rationale:

1. The lanes that cost the most wall-time are **work-dominated**: worldgen first
   load is ~88% generation; light is ~98% propagation. Zero-copy cannot touch work.
2. The one serde-dominated case (warm worldgen, ~77%) has a **cheaper, proven,
   stable-toolchain fix** (resident mirror + delta, 067 Stage 4) that removes the
   payload, not just the copy — a bigger win without the build cost.
3. The shared-linear-memory build cost is **severe and permanent** (nightly +
   build-std with no stable path, global-locked allocator, non-thread-safe
   `WasmRefCell` against heavy `Rc<RefCell>` use, up-front max-memory vs iOS caps),
   and must not perturb the stable desktop build.
4. Render (067) already declined shared linear memory and proved healthy without it.

**Staged plan (in 062's lineage), do in order, measure between:**

1. **Worldgen payload reduction (resident mirror + delta).** Apply the 067 Stage 4
   pattern to the web worldgen lane. Stable toolchain. Acceptance: warm-job request
   and response collapse to deltas; re-run `native:web:chunk-smoke` /
   `native:web:movement-perf` and confirm the warm worldgen bytes and round-trip
   drop, with desktop `mpsc` untouched (`native:movement:smoke` /
   `native:timedemo:smoke` green). **Implementation plan:**
   [`069-web-worldgen-lane-payload-reduction.md`](069-web-worldgen-lane-payload-reduction.md).
2. **(Optional) Light worker residency.** Port the native light worker's resident
   `RetainedInitialLightState` to the wasm light worker if light *latency* (not
   serde) is found to matter on a slower device. Attacks work, not transport.
3. **Re-measure the residual.** After (1)/(2), re-decompose. If a user-felt path
   (chunk-load latency on a throttled/mobile device) still shows a meaningful
   serialize+copy residual **and** the project has otherwise committed to nightly
   (e.g. for wasm SIMD), only then revisit shared linear memory — at which point its
   marginal cost over the already-paid nightly is lower and its residual benefit is
   the much-smaller post-delta serde. `CLAUDE.md` already anticipates shared wasm
   memory "once that slice is implemented"; this analysis says **payload reduction
   comes first**, and zero-copy's residual value is small after it.

**Revisit conditions (explicit):** reverse this decline only if *all* hold — (a)
post-payload-reduction serde+copy is still a measured ≥~20% of a **user-felt**
chunk-load path, (b) measured on a representative slower/mobile device, and (c) the
nightly + `-Z build-std` toolchain is already required for another reason.

## Relationship to other tacticals

- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) — parent.
  This investigates the "Stage 5: worker queue upgrade to shared memory" frontier
  and its ultimate "shared Wasm linear-memory thread runtime" option for the
  worldgen/light lanes. Result: decline shared linear memory; do payload reduction
  on the worldgen lane first.
- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  — render is excluded as a target (already healthy) and its Stage 4 resident-mirror
  + delta pattern is the recommended cheaper alternative for the worldgen lane. 067's
  Non-Goals already keep shared Wasm linear memory out of scope; this tactical
  measures *why* that remains the right call for 062's lanes too.

## Reproducibility appendix

### Browser baseline

```
pnpm native:web:chunk-smoke > /tmp/chunk-smoke.json 2>/dev/null
# per-lane metrics live at result.canvas.{firstReport,report}.{worldgen,lightStatus}JobFrameMetrics
# and result.canvas.sharedTopologyStress.fallbackRunner.diagnostics.* (the smaller probe).
```

### Host byte-exact CPU split (temporary instrumentation — removed after capture)

A `#[cfg(test)] mod zero_copy_serde_bench` was added to
`native/crates/mclone-server/src/job_codec.rs` and **removed** after capture (it is
not committed; the handoff specified temporary instrumentation). It builds the real
job requests (25 targets at origin, seed 12345; light batch via
`GeneratedChunk::to_chunk_snapshot` + `PendingLightStatus::from_feature_publication`,
the scheduler's own construction) and times encode/decode/work separately. Run with:

```
cargo test --release -p mclone-server -- --ignored --nocapture zero_copy
```

Key bench shape (reconstruct in `job_codec.rs`'s module so it can reach `FrameWriter`/
`FrameReader`/the codec fns):

```rust
// worldgen cold: targets-only request, 36 MB response
let targets = (-2..=2).flat_map(|x| (-2..=2).map(move |z| ChunkPos::new(x, z))).collect::<Vec<_>>();
let (result1, work1) = { let mut c = OverworldFeatureDependencyCache::new();
    let t = Instant::now(); let r = c.generate_features_chunks_with_dependencies(12345, targets.iter().copied(), vec![]); (r, t.elapsed()) };
let resp1 = /* replicate compute_worldgen_job_frame's response FrameWriter */;
// time encode_worldgen_request, encode resp1, decode_worldgen_response(&resp1)
// worldgen warm: feed result1.retained_dependencies.values().cloned() back as the request deps,
//   time encode/decode of that ~35 MB request + the 36 MB response decode.
// light: one PendingLightStatus per result.chunks via from_feature_publication(...),
//   encode_light_status_request -> compute_light_status_job_frame -> decode_light_status_response;
//   read work from completed[i].timing.total_us.
```

Measured (this device, release):

```
WORLDGEN cold: work 398 ms, serde 56 ms (resp decode 54 ms) -> 12.3%
WORLDGEN warm: work 34 ms, serde 112 ms (req decode 54 + resp decode 54) -> 77.0%
LIGHT:         work 103 ms, serde 2.55 ms -> 2.4%
memcpy: 8 MB 0.23 ms / 32 MB 0.63 ms / 64 MB 1.07 ms (37-63 GB/s)
```
