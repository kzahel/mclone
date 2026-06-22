# 065: Native Web Mobile Streaming Performance

Status: active first pass landed.

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

This tactical is about the native web/WASM app only. Do not revive or edit the
legacy TypeScript browser engine.

## Current Evidence

The web app currently compiles the next camera-centered render view inside the
main animation-frame tick:

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

## Diagnosis

The CPU mesh build already runs in a Web Worker, but the app loop still waits
for the entire compile/apply path before it can continue the frame:

1. Build a Rust-owned compile request for the new camera center.
2. Send all requested section inputs to the render compiler worker.
3. Wait for the worker to return a packed section report.
4. Decode the packed report in WASM.
5. Finish the render compile result and upload changed sections to WebGPU.
6. Only then resume normal frame flow.

This is correctness-first streaming. It avoids rendering from a missing view,
but it makes movement depend on render-section rebuild latency.

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
- Do not port legacy TypeScript runtime code.
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

### 4. Reduce Per-Move Compile Scope

Status: next likely step after begin-request unblocking.

Investigate why one chunk-center move submits around `96` sections for radius
`1`. Some of that may be legitimate neighbor-boundary rebuild pressure, but it
needs measurement.

Candidate reductions:

1. avoid submitting sections whose mesh input revision is unchanged
2. preserve and reuse worker-packed mesh outputs when only camera center changes
3. make render-section dirty reasons visible in diagnostics
4. verify loaded-neighbor boundary invalidation is no broader than needed
5. prioritize near/in-frustum sections before hidden or low-value sections

Acceptance:

- one-chunk moves submit materially fewer sections when terrain is already
  loaded and unchanged
- stale/duplicate compile work remains near zero during normal walking
- desktop/native render-section correctness tests remain green

### 5. Split Or Budget Decode And GPU Upload

Status: optimization follow-up.

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

Next likely step: split or restructure the browser chunk-view request so it
does not hold the render session across the async server exchange. Once
begin-request gaps are small, move to slice 4 and reduce per-move compile
scope.

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
