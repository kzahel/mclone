# 065: Native Web Mobile Streaming Performance

Status: Superseded by [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md).
The render-worker streaming direction now lives in 067; this doc is retained for
its landed history. Historical status: diagnostics, movement perf harness, background compile,
deferred browser commands, budgeted worker-update drains, and stale-result
tolerance landed; shared Rust compile-queue/coalescing decision landed;
compile-scope diagnostics, all-air section mesh fast path, and
target-section-aware browser render-worker requests landed; shared-memory
render-worker architecture is tracked in
[`066-web-shared-memory-worker-architecture.md`](066-web-shared-memory-worker-architecture.md).
Render-compiler transport diagnostics and persistent worker asset/catalog state
landed there; the shared result arena prototype and request-scoped shared input
snapshots also landed. The next performance target is section-level dirty
planning and the final resident shared-worker compile lifecycle.

## Purpose

Fix the mobile web stall that appears when walking far enough to stream new
chunks in the native Rust/WASM app.

The observed user-facing behavior is:

- mobile controls work, but after moving for a short time the app shows
  `compiling`
- movement/rendering appears to pause for multiple seconds on phone refresh
  builds
- the pause repeats around chunk-center crossings, so normal exploration feels
  stop-and-go

This tactical is about the native web/WASM app only. Do not revive the retired
browser engine.

## Current Evidence

The original stalling path compiled the next camera-centered render view inside
the main animation-frame tick:

```text
requestAnimationFrame tick
  -> advanceCameraFrame(...)
  -> if camera center changed:
       await compileCameraView()
  -> renderCameraFrame(...)
```

Relevant current files:

- `native/apps/mclone-web-client/www/mclone-web-app.js`
  - `tickFrame(...)`
  - `compileCameraView()`
  - `RenderSectionWorkerCompiler`
- `native/apps/mclone-web-client/src/web_canvas.rs`
  - `begin_camera_render_compile_request_for_radius(...)`
  - `finish_camera_render_compile_request_with_packed_report(...)`
  - `GeneratedChunkRenderReport` diagnostics
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`
  - app-loop smoke
  - mobile app-loop smoke
  - render compiler worker correctness smoke

Quick production measurement on 2026-06-22, using no-clip movement to force
chunk crossings in mobile-sized Chrome, showed repeated compile windows around
`1.0-1.2s` on desktop hardware. Each crossing compiled roughly:

- `96-129` submitted render sections
- `10-12 MB` packed worker payload
- `44-56` uploaded sections after a one-chunk move

On a phone, the same path plausibly becomes the reported `3-4s` stall.

After the scheduler split on 2026-06-22, the same mobile movement perf lane
still spends roughly `0.9-1.0s` in the render compiler worker, but normal frame
progress continues while the worker runs. Movement compile windows now report
main-loop frame gaps around `10-22ms` locally instead of the previous
`190-460ms` gaps. The worker payload is still large, so this is a scheduling fix
rather than a mesh-cost fix.

After the first shared queue extraction on 2026-06-22, the JavaScript app no
longer owns compile request coalescing. `mclone-render-session` owns the
`RenderViewCompileQueue` decision, and the browser app applies the returned
start/queued/skipped result. Local movement perf still shows roughly
`0.9s` worker round trips and `11-17ms` movement compile frame gaps; remaining
large time is in chunk-view/request preparation and full-view mesh work, not in
the old JS queue object.

Compile-scope diagnostics added on 2026-06-22 show why this can still look like
a full-view compile even after queue convergence. `RenderSectionViewSync` marks
newly added chunks dirty, plus current chunks adjacent to removed chunks. A
camera-centered move can therefore dirty `8-9` loaded chunks; because the ready
plan currently expands each loaded dirty chunk to all section keys, the Rust
scheduler can legitimately submit `128-144` ready sections for a radius-1 view.
The browser worker result still reports a full set of `144` logical section
records and an `~11.6 MB` packed payload on these runs, even though only about
`66-68` sections contain geometry. That leaves a transport/result-size problem
after the main-thread scheduling fix.

After the target-section-aware worker protocol landed on 2026-06-22, worker
reports are bounded by the Rust-submitted target section list instead of always
rebuilding the full worker view. Small dirty-section compiles now return tiny
reports, for example `2` section records and a `195,876` byte packed payload.
Full dirty-chunk movement plans still legitimately submit `128-144` sections
and still produce `5.8-11.6 MB` packed payloads, so the remaining large cases
are now attributable to dirty planning and worker-side compile/runtime cost
rather than the worker blindly returning a full view for every request.

After the shared result arena landed on 2026-06-22, the browser render compiler
normal path no longer transfers packed report bytes through `postMessage`.
Movement perf now reports `shared-result-buffer`, `0` transferred response
bytes, and shared result byte lengths matching the packed report sizes. The
same full movement cases still produce `~9.7-11.6 MB` packed/shared results and
`~0.87-0.89s` worker round trips, confirming that the remaining divergence is
worker-side compile/input setup and coarse dirty planning, not response
transport.

After the shared input snapshot path landed on 2026-06-22, web compile
requests now pack the main Rust session's actual `ChunkSnapshot`s, copy them
into a request-scoped `SharedArrayBuffer` input arena, and the persistent worker
calls `WebRenderCompilerSession::compileSnapshotSectionsForTargets(...)`.
Browser diagnostics now expose snapshot input byte length, input chunk count,
shared input capacity, `snapshotInputCompileUsed`, and
`generatedViewFallbackUsed`. This removes the normal hot-path behavior where the
worker built a fresh `WebRuntime::local_integrated(SMOKE_SEED)` view for each
compile. It does not yet eliminate the main-side snapshot serialization/copy,
the worker-side wasm copy, or the packed result decode/apply path.

## Diagnosis

The CPU mesh build already runs in a Web Worker. The first scheduler fixes
removed the main animation-frame await for post-initial movement compiles, so
normal input/render frames can continue while the worker runs. The remaining
cost is the size and shape of each compile transaction:

1. Build a Rust-owned compile request for the new camera center.
2. Send all requested section inputs to the render compiler worker through a
   shared input buffer.
3. Wait for the worker to return a packed section report.
4. Decode the packed report in WASM.
5. Finish the render compile result and upload changed sections to WebGPU.
6. Apply the new cache once the background compile completes.

This is now responsive background streaming after first presentation, but it
still pays full-view worker and packed-result costs during movement.

## Target UX

On mobile web:

- moving across chunk boundaries should keep camera input responsive
- the existing view may remain visible while the next view compiles
- the debug/status UI may show background compile progress, but it should not
  imply gameplay is blocked unless the first view has not rendered yet
- a single one-chunk move should not create a multi-second main-loop pause
- repeated movement should avoid stacking stale compile work

The first target is subjective playability, backed by concrete timing metrics.
After instrumentation lands, set realistic budgets from measured data instead
of guessing.

## Non-Goals

- Do not change worldgen correctness or chunk publication semantics.
- Do not add Android, Gradle, or OpenXR scaffolding.
- Do not move DOM/touch handling into Rust.
- Do not port retired browser-engine runtime code.
- Do not implement a full render-distance settings UI in this tactical.
- Do not require perfect zero-stutter streaming before landing the first fix.

## Implementation Slices

### 1. Web Streaming Timing Instrumentation

Status: completed first pass.

Add per-compile timing data that is visible in runtime state and returned from
the browser performance smoke.

Measure at least:

1. chunk center that triggered the compile
2. loaded/rendered center before the compile
3. total compile window wall time
4. begin-request time
5. worker round-trip time
6. packed payload byte length
7. decode/finish/apply time
8. uploaded/removed section counts
9. accepted/stale section counts
10. largest observed frame gap during the compile window

Acceptance:

- app-loop smoke reports compile timing for initial load and movement load
- mobile smoke can expose the same diagnostics
- production debug HUD can show concise current/last compile duration
- the timing path does not change gameplay behavior

### 2. Web Movement Perf Harness

Status: completed first pass.

Add a dedicated scripted web movement performance lane, likely:

```text
pnpm native:web:movement-perf
```

The harness should:

1. use a mobile-sized viewport with touch enabled
2. load the native web app
3. wait for first render to settle
4. move across a fixed number of chunk boundaries
5. record compile windows and frame gaps
6. save a JSON report under `/tmp`
7. optionally capture a screenshot under `/tmp`

Start as a recording harness, not a hard budget gate. Add thresholds only after
several local and phone-shaped runs establish normal ranges.

Acceptance:

- the harness reproduces the current compile windows
- output identifies whether time is in worker build, decode/apply, or upload
- command is documented in `package.json`
- failures are actionable instead of just "timed out"

### 3. Unblock The Animation Loop During Streaming

Status: completed first pass.

Refactor `mclone-web-app.js` so crossing a chunk boundary starts compile work
without awaiting it inside the animation-frame tick.

Target shape:

```text
tickFrame(...)
  -> advance camera/input every frame
  -> if new center needs compile and none useful is in flight:
       startCompileCameraView(...)
  -> render current loaded view every frame

background compile promise
  -> worker compile
  -> finish/apply result
  -> update loadedCenter when ready
```

Important details:

- keep the current rendered cache visible while the next view is compiling
- keep accepting movement/look input during the compile
- coalesce duplicate compile requests for the same center
- do not let stale results replace a newer loaded center
- still block first presentation until the initial view has rendered
- maintain error propagation so real compile failures remain visible

Acceptance:

- movement input keeps updating during a later compile
- `frameCount` and `renderCount` continue advancing during background compile
- the status badge does not cover normal play as if the app is stuck
- mobile movement smoke still passes
- new movement perf report shows lower main-loop stall/frame-gap time

### 4. Match Desktop Scheduling Shape On Web

Status: completed first pass; Rust compile-queue extraction landed, broader
shared coordinator extraction still pending.

The first fix keeps the JavaScript app loop responsive by matching the desktop
submit/drain shape more closely:

```text
frame
  -> apply input
  -> submit pose/chunk-view commands without awaiting full server exchange
  -> drain a bounded number of completed worker updates
  -> render current cache
  -> kick queued compiles when idle
```

Implemented pieces:

1. `WebRuntime::send_gameplay_command_deferred(...)` for routine camera pose
   commands on web-worker hosts
2. `WebRuntime::drain_pending_runner_updates_budgeted(...)`
3. budgeted `WebIntegratedServerRunner` update-frame drains with accurate queue
   depth diagnostics
4. split chunk-view begin into request/poll/compile so movement can keep
   rendering while server updates arrive
5. waiting responses for "not ready yet" render plans instead of page errors
6. stale web render compile completions treated as no-op reports when all
   submitted sections are obsolete
7. idle RAF kicks for queued compile work so interaction-triggered compiles do
   not get stuck behind older movement compiles
8. `RenderViewCompileQueue` in `mclone-render-session` owns web compile
   start/skip/queue/coalescing decisions, including preserving forced requests
   while busy

Acceptance:

- `pnpm native:web:movement-perf` keeps frame progress during chunk crossings
- mobile movement compile gaps are around one frame locally
- app/mobile smokes still pass block interaction, touch controls, and screenshots

Remaining architectural work: move the rest of the browser chunk-view
request/poll and browser-worker transport coordination from app glue toward the
shared Rust engine/render-session coordinator tracked by tactical 061.

### 5. Reduce Per-Move Compile Scope

Status: partially complete; diagnostics, a safe all-air mesh fast path, and
target-section-aware worker request/result transport landed. Finer dirty
planning and worker-side runtime/cache reuse are still pending.

Investigate why one chunk-center move submits around `128-144` sections for
radius `1`. The first measurement shows this is not only JavaScript queue
divergence: view-diff dirtying can mark `8-9` loaded chunks, and loaded dirty
chunks currently expand to every section in those chunks.

Candidate reductions:

1. avoid submitting sections whose mesh input revision is unchanged
2. preserve and reuse worker-packed mesh outputs when only camera center changes
3. split chunk-level view dirtying from section-level face-neighbor dirtying,
   so removed-neighbor pressure does not always expand to every Y section
4. prioritize near/in-frustum sections before hidden or low-value sections
5. stop recreating the worker-side asset/runtime view for every compile request;
   the target-aware protocol makes payload size scale down, but worker
   round-trip time still stays around `0.85-0.93s` even for tiny targeted
   compiles

Acceptance:

- one-chunk moves submit materially fewer sections when terrain is already
  loaded and unchanged
- worker `sectionCount` and packed byte length scale with requested target
  sections instead of always returning the full radius-1 section set
- stale/duplicate compile work remains near zero during normal walking
- desktop/native render-section correctness tests remain green

### 6. Split Or Budget Decode And GPU Upload

Status: optimization follow-up; first update-drain budgeting landed, packed
decode/report finish and GPU upload can still be split further if measured
phone runs show spikes.

If timing shows decode/apply/upload remains a large main-thread spike after
background compile, split the apply phase.

Possible approaches:

- chunk packed reports into smaller section batches
- upload only a limited number of sections per frame
- keep a ready queue for compiled sections
- render with mixed old/new section cache while uploads drain

Acceptance:

- apply/upload work is spread across frames
- no single frame absorbs a large packed result
- partially applied caches do not show broken neighbor seams beyond existing
  accepted streaming behavior

## Validation

Run the normal correctness gates after each behavioral change:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
```

Run the new performance lane once it exists:

```text
pnpm native:web:movement-perf
```

For rendered-output validation, inspect screenshots saved under `/tmp`. Do not
write screenshots into the repo.

## First Pass Landed

Implemented on 2026-06-22:

- `mclone-web-app.js` now records compile timing windows in
  `runtime.state.activeCompileTiming`, `lastCompileTiming`, and
  `compileTimings`, including request center, previously loaded center, total
  wall time, begin-request time, worker round trip, packed payload bytes,
  decode/finish/apply time, upload/removal/accepted/stale counts, and max frame
  gap.
- the debug HUD shows a compact current/last compile line while keeping the
  status badge hidden for normal background streaming after first render.
- post-initial chunk-center changes start render compile in the background
  instead of awaiting it inside the animation-frame tick; the current rendered
  cache stays visible while the worker compile runs, duplicate requests coalesce
  to the latest queued center, and first presentation still awaits the initial
  render.
- `browser-smoke.mjs --movement-perf` and
  `pnpm native:web:movement-perf` run a mobile-shaped Chrome movement harness,
  save `/tmp/mclone-native-web-movement-perf.json`, and capture screenshots
  under `/tmp`.
- app-loop and mobile smokes now assert compile timing diagnostics are present;
  mobile smoke waits for background streaming to settle before final screenshot
  assertions.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after the first pass:

- movement compile windows still total about `1.1-1.4s`
- worker round trip remains about `0.9s`
- decode/finish/apply is currently small, about `11-15ms`
- submitted sections remain high, around `97-144`
- max frame gaps dropped for the worker portion, but begin-request still
  creates visible `~200-500ms` gaps because the async chunk-view request holds
  the WASM session before the worker compile begins

Next likely step from that first pass was the scheduler-shape split and shared
queue extraction recorded below. The current next step is now reducing per-move
compile scope and making dirty reasons visible enough to explain full-view
submissions.

## Shared Queue Extraction Landed

Implemented on 2026-06-22:

- Added `RenderViewCompileQueue` to `mclone-render-session` so start/queue/skip
  compile decisions are Rust-owned and unit-tested.
- Added `WebChunkRenderSession::requestCameraRenderCompile(...)` and
  `takeQueuedCameraRenderCompile(...)` so browser JS asks the WASM session for
  scheduling decisions instead of storing a parallel queued compile object.
- Kept JS responsible for DOM input, promise wiring, browser worker transport,
  and WebGPU presentation only.

Validation run:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo fmt --manifest-path native/Cargo.toml --all -- --check
node --check native/apps/mclone-web-client/www/mclone-web-app.js
pnpm native:web:build
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- movement compile frame gaps stayed around `11-17ms`
- worker round trip remained about `0.9s`
- decode/finish/apply stayed small, about `7-15ms`
- some movement compiles still submit a full `144` sections after crossing
  farther chunk boundaries

That next step is refined by the compile-scope diagnostics below.

## Compile Scope Diagnostics Landed

Implemented on 2026-06-22:

- `WebCompileScopeReport` now carries view dirty/removal chunk counts,
  loaded/removal/stale dirty counts, ready/deferred section counts, and budgeted
  chunk counts from the Rust sync planner into compile requests, final render
  reports, runtime diagnostics, and browser smoke summaries.
- Worker mesh summary diagnostics now expose visibility-graph build count and
  timing in the same compile timing object.
- `mclone-mesh` now fast-paths all-air textured render sections by emitting an
  empty all-visible section record without running face emission or visibility
  graph scanning for that section. This preserves renderer occlusion data while
  avoiding pointless work for empty vertical bands.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- movement compile frame gaps stayed low: `15.3-19.8ms`
- worker round trip stayed high: `913.5-930.3ms`
- decode/finish/apply stayed small: `10.5-14.1ms`
- view dirty chunks were `8` and `9`
- ready/submitted compile sections were `128` and `144`
- worker reports still contained `144` section records, `66-68` non-empty
  sections, and a max packed payload of `11,623,508` bytes

## Target-Section-Aware Worker Protocol Landed

Implemented on 2026-06-22:

- `WebChunkRenderSession` now includes a flat `targetSections` `Int32Array` in
  browser compile requests, using the exact `RenderSectionKey` set selected by
  the Rust ready plan.
- `mclone-render-compiler-worker.js` forwards those target keys to a new
  `mclone_web_compile_generated_chunk_sections_for_targets(...)` WASM export.
  The old full-view export remains as fallback.
- The worker result path now treats target keys missing from the packed worker
  report as stale instead of silently accepting them, so dirty work is not
  cleared by an empty or partial result.
- Browser smoke assertions now require worker section counts to match the
  submitted target count for the chunk-smoke worker protocol instead of
  hard-coding `144`.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-render-compiler-worker.js
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/www/mclone-web-smoke.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- frame progress stayed responsive: movement compile max frame gaps were about
  `14.4-16.1ms`
- a tiny dirty-section compile submitted `2` sections, returned `2` worker
  section records, and packed `195,876` bytes
- a partial dirty-chunk movement compile submitted `128` sections, returned
  `80` worker section records, packed `5,770,628` bytes, and requeued `51`
  missing target sections as stale
- a full dirty-chunk movement compile still submitted/returned `144` section
  records and packed `11,623,508` bytes
- worker round trips still stayed high, around `854-918ms`, even for tiny
  targeted compiles, which points to worker-side asset/runtime setup and fresh
  worker-view generation as the next bottleneck

Subsequent slices moved the render compiler toward that shared-memory plan:
persistent worker assets, shared result buffers, and shared input snapshots
landed. The remaining follow-up is dirty-scope reduction and the final resident
shared worker lifecycle.

## Render Compiler Transport Diagnostics Landed

Implemented on 2026-06-22:

- render compiler worker responses now report their transport kind as
  `message-transfer`, shared-memory availability, worker wasm init count,
  worker compile count, request asset-pack bytes, target-section bytes, and
  transferred packed response bytes.
- the app and smoke render-worker clients now track worker construction count,
  compile count, asset-pack send count, per-request transfer byte lengths, and
  cumulative transferred request/response byte totals.
- app compile timings and movement perf JSON now expose both quick fields such
  as `renderCompilerTransportKind`,
  `renderCompilerRequestAssetPackByteLength`, and
  `renderCompilerTransferredResponseByteLength`, plus a nested
  `renderCompilerMetrics` object.
- browser smoke assertions now fail if accepted app compile timings or direct
  render-worker smoke results do not expose the temporary transfer path.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-render-compiler-worker.js
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/www/mclone-web-smoke.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- movement compile transport kind was `message-transfer`
- worker round trips stayed high: `894.4-908.3ms`
- movement compile max frame gaps stayed low: `11.6-15.3ms`
- each render compile still resent the packed asset zip:
  `5,828,345` transferred request bytes per compile
- a small targeted compile still transferred the full asset pack for only `2`
  target sections and a `195,876` byte response
- the full movement compile transferred an `11,623,508` byte packed response
- cumulative render compiler transfer after four worker compiles reached
  `23,313,380` request bytes and `28,900,904` response bytes
- runner, worldgen, and light-status metrics stayed on `shared-memory`, making
  the render compiler transport divergence explicit

## Persistent Render Worker Assets Landed

Implemented on 2026-06-22:

- added `WebRenderCompilerSession` as a worker-callable wasm-bindgen class that
  owns loaded terrain mesh assets and compiles generated render sections through
  a resident mesh catalog.
- `mclone-render-compiler-worker.js` now has an explicit
  `init-render-compiler` handshake. The asset pack is transferred once during
  worker init, parsed once, and retained in the worker session.
- app and smoke render-worker clients now wait for worker readiness before
  compile submission and no longer send `assetPack.slice()` in normal compile
  requests.
- browser smoke assertions now require `persistentAssetCatalog: true`,
  `assetPackSendCount: 1`, nonzero `workerAssetPackInitByteLength`, nonzero
  `workerAssetLoadCount`, and zero per-compile
  `requestAssetPackByteLength`.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-render-compiler-worker.js
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/www/mclone-web-smoke.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- movement compile transport kind remained `message-transfer`
- worker asset load count stayed at `1`
- asset pack send count stayed at `1` across four worker compiles
- per-compile asset request bytes dropped from `5,828,345` to `0`
- one-time worker asset init transfer was `5,828,345` bytes
- worker round trips improved slightly but remained high:
  `858.1-883.9ms` for movement compiles
- movement compile max frame gaps stayed low: `9.3-17.0ms`
- the full movement compile still transferred an `11,623,508` byte packed
  response
- cumulative render compiler response transfer after four worker compiles was
  `28,900,904` bytes, so the remaining transport target is the packed result
  path, not the asset-pack request path

## Shared Result Arena Landed

Implemented on 2026-06-22:

- app and smoke render-worker clients now allocate a per-compile shared control
  header plus a 16 MiB shared response arena and pass those buffers to
  `mclone-render-compiler-worker.js`.
- the worker writes packed compile-report bytes into the shared arena, publishes
  byte count/capacity/status with `Atomics`, and posts only completion metadata
  plus the shared-buffer reference. If the arena is too small, the worker
  reports an overflow `SharedArrayBuffer` instead of falling back to a large
  transferred result.
- runtime compile timing diagnostics now separate packed byte length,
  transferred response byte length, shared result byte length, shared result
  capacity, shared response count, and overflow count.
- browser smoke assertions now require `shared-result-buffer`,
  `sharedResultBufferUsed: true`, and `0` transferred render-compiler response
  bytes on the normal path.

Validation run:

```text
node --check native/apps/mclone-web-client/www/mclone-render-compiler-worker.js
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/www/mclone-web-smoke.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Observed local movement perf after this chunk:

- initial compile submitted `144` sections, produced `11,310,892`
  packed/shared result bytes, transferred `0` response bytes, and had an
  `887.7ms` worker round trip
- first full movement compile submitted `129` sections, produced `9,740,708`
  packed/shared result bytes, transferred `0` response bytes, and had an
  `866.0ms` worker round trip
- a tiny stale movement compile submitted `2` sections, produced a `36` byte
  empty packed/shared report, transferred `0` response bytes, and still had an
  `830.1ms` worker round trip
- later full movement compile submitted `144` sections, produced `11,623,508`
  packed/shared result bytes, transferred `0` response bytes, and had an
  `890.1ms` worker round trip
- max movement compile frame gaps stayed low locally, around `10.3-22.0ms`
- no shared result overflow occurred with the 16 MiB arena

## Shared Input Snapshot Arena Landed

Implemented on 2026-06-22:

- `WebChunkRenderSession` now collects the main session's actual client
  `ChunkSnapshot`s for the compile request and passes them into
  `mclone-render-session::submit_prepared_sync_plan(...)`, matching the desktop
  request shape more closely.
- The JS-facing compile request includes encoded snapshot input bytes,
  `snapshotInputByteLength`, and `snapshotInputChunkCount`. The smoke report
  strips the raw byte array and keeps only diagnostics.
- `RenderSectionWorkerCompiler` in both the app and web smoke page copies those
  bytes into a request-scoped shared input arena, publishes ready/byte/capacity
  control words with `Atomics`, and posts only SAB handles plus target-section
  metadata to the worker.
- `mclone-render-compiler-worker.js` reads the shared input bytes and prefers
  `WebRenderCompilerSession::compileSnapshotSectionsForTargets(...)` over the
  generated-view fallback.
- Browser smoke assertions now require `sharedInputBufferUsed: true`,
  `snapshotInputCompileUsed: true`, `generatedViewFallbackUsed: false`, and
  zero transferred render-compiler request/response bytes on the normal path.

What this proves:

- The normal browser render compiler no longer compiles from a worker-local
  generated view; it compiles the target section list from the main session's
  explicit snapshot set.
- Worker request size is now visible as snapshot input bytes instead of hidden
  inside generated worker runtime setup.
- The remaining large movement cases are now about dirty-section scope,
  snapshot/input serialization cost, worker mesh cost, packed result size, and
  decode/apply/upload work.

Observed local movement perf after this chunk:

- all accepted compile timings used `snapshotInputCompileUsed: true` and
  `generatedViewFallbackUsed: false`
- snapshot input bytes were about `206-215 KB` for the radius-1 view, copied
  through the request-scoped shared input arena with `0` transferred request
  bytes
- initial compile still submitted `144` sections and produced `11,310,892`
  packed/shared result bytes, but worker round trip dropped to about `65.9ms`
- movement compiles submitted `96` and `144` sections, produced `7,874,812`
  and `11,623,508` packed/shared result bytes, and reported worker round trips
  around `34.1-54.2ms`
- total movement compile windows still spent `261.7-473.7ms`, mostly before or
  after worker execution, so dirty planning, request preparation, packed result
  size, decode/apply, and upload remain the next targets
- movement compile max frame gaps stayed low locally, around `12.1-19.2ms`

Current next likely step: reduce the remaining full `128-144` section dirty
plans by splitting chunk-level view dirtying from section-level
neighbor-boundary dirtying, then replace this request-scoped snapshot copy with
the final resident shared worker lifecycle from tactical 066.

## Deployment Check

Because this problem was observed on a real phone, validation is not complete
until the production deploy is checked on mobile web after the fix.

At minimum:

1. deploy with the cache-busted native web path
2. open `https://mclone.kzahel.com/` on phone
3. refresh normally
4. walk across several chunk boundaries
5. confirm movement/look remains responsive while background compile happens
6. record any remaining compile-window durations from the debug HUD or runtime
   diagnostics

## Original Sequencing Note

The original first-pass plan was to land slices 1 and 2 together, then slice 3.
That first pass is now reflected in the landed notes above.
