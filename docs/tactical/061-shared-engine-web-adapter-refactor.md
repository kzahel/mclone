# 061: Shared Engine / Web Adapter Refactor

Status: active web session uses shared render planner.

## Purpose

Refactor the current native desktop app-owned runtime/render glue into shared
engine interfaces that can be consumed by both desktop and browser/WASM without
duplicating section cache, asset, runtime polling, or render-prep logic.

Recent WASM bring-up proved important compatibility gates:

- browser WebGPU canvas presentation works through Rust `wgpu`
- packed vanilla assets can be fetched, parsed, and stitched into the shared
  terrain atlas/catalog
- server-generated chunks can be meshed and rendered in the browser
- a WASM session can hold onto packed assets and GPU texture state across more
  than one render

Those smokes are validation harnesses, not the final architecture. The next
work should converge from the native engine side: keep desktop working, extract
the platform-neutral runtime/render session shape, and make the WASM harness a
consumer of that shared shape.

## Problem

Native desktop currently has the mature path in app-local modules:

- `native/apps/mclone-native-client/src/scene_runtime.rs`
  - `WindowSceneRuntime`
  - integrated/remote server polling
  - chunk-view ownership
  - client replica updates
  - dirty render chunk/section tracking
  - render-section compile worker integration
  - cached section access
- `native/apps/mclone-native-client/src/render_cache.rs`
  - `CachedTexturedRenderSections`
  - `RenderSectionCacheUpdate`
  - `RenderSectionCompileWorker`
  - snapshot-to-render-section build helpers
  - desktop asset source discovery wrappers
- `native/apps/mclone-native-client/src/app.rs`
  - `winit` surface/device ownership
  - frame polling and GPU upload calls
  - render stats/debug UI wiring

The WASM path now has a smaller parallel shape in
`native/apps/mclone-web-client/src/web_canvas.rs`: it owns a browser session,
asset-pack parsing, section build/update decisions, GPU section resources, and
smoke counters. That was acceptable for bring-up, but it is the wrong place for
the engine policy to grow.

The duplication risk is not just code size. If desktop and WASM each learn their
own dirty-section, neighbor-readiness, compile-budget, upload-budget, and asset
cache rules, they will diverge exactly where renderer correctness and streaming
behavior need to be shared.

## Target Shape

The target is one shared engine/render-session layer with platform adapters:

```text
desktop platform adapter
  -> winit surface/device/events
  -> filesystem or packed asset source
  -> native worker policy
  -> shared engine/render session

web platform adapter
  -> wasm-bindgen canvas/events/fetch
  -> fetched packed asset source
  -> wasm inline/worker policy
  -> shared engine/render session

shared engine/render session
  -> client/server/transport runtime
  -> chunk-view management
  -> client replica updates
  -> render-section dirty/cache policy
  -> CPU section compile policy
  -> GPU section upload decisions
  -> frame render inputs/stats

mclone-render
  -> explicit view/target inputs
  -> persistent GPU section/atlas resources
```

This follows the Playbox pattern that matters for Mclone: platform code owns
surfaces, event loops, and platform-specific packaging; renderer-facing data
uses explicit view/target/resource inputs; shared runtime logic is not tied to a
desktop window or browser canvas.

## Boundary Rules

- Do not move `winit`, DOM, `web_sys`, browser fetch, pointer lock, or platform
  event loops into shared engine crates.
- Do not move GPU device/surface creation into the engine session.
- Do keep renderer-facing view/projection, target size, depth/color target, and
  render resource updates explicit.
- Do keep asset access abstract over `AssetSource`; platform adapters decide
  whether that source is loose files, packed files, or fetched bytes.
- Do keep the existing native desktop smokes and screenshots green during each
  extraction.
- Do keep the WASM smokes as compatibility gates, but stop growing them as a
  second app architecture.

## Proposed Shared Interfaces

Exact names can change during implementation, but the extraction should move
toward these responsibilities:

### Asset Bundle

Shared CPU-side terrain asset state:

```text
TexturedTerrainAssets
  catalog: TexturedMeshCatalog
  atlas: TextureAtlasImage
  atlas_sprite_count
```

This already exists in `mclone-mesh::terrain_assets`. Desktop and WASM should
continue to call this through an `AssetSource` selected by the platform adapter.

### Render Section Cache

Move the app-local CPU cache and update report into a shared crate or shared
native client module:

```text
RenderSectionCache
RenderSectionCacheUpdate
RenderSectionCompileRequest
RenderSectionCompileResult
```

This cache owns:

- current `RenderSectionKey -> TexturedRenderSectionMesh`
- removal keys
- rebuilt section stats
- visibility graph stats
- stale-result handling facts

It should not own `wgpu` resources. GPU upload remains in `mclone-render`.

### Render Section Compiler

Represent compile policy behind a small interface:

```text
trait RenderSectionCompiler {
    fn submit(...)
    fn drain_completed(...)
    fn pending_job_count(...)
}
```

Implementations:

- native threaded compiler using the current `std::thread` / `mpsc` worker
- browser/WASM worker compiler using Web Workers with shared Wasm memory
  (`SharedArrayBuffer`/atomics) once cross-origin isolation and packaging are in
  place
- synchronous inline compiler for WASM only as a smoke/fallback implementation
  behind the same interface

The shared session should be generic over this policy rather than containing
desktop-only thread ownership.

The web path must not become a reduced single-threaded engine. `wasm32-unknown-unknown`
is still the browser target, but the runtime threading model should match the
desktop job lifecycle: submit work off the frame path, drain completed results,
preserve stale/revision handling, and keep frame presentation responsive.

### Engine Render Session

Extract the platform-neutral part of `WindowSceneRuntime`:

```text
EngineRenderSession
  client: ClientRuntime
  local/remote server transport adapter
  chunk view state
  render-section cache
  dirty chunk/section sets
  render-section revisions
  compile policy
```

Responsibilities:

- set or update chunk view
- poll runtime/server/transport updates
- apply client replica updates
- mark dirty render sections from snapshots, unloads, and section-block deltas
- select ready render sections based on neighbor policy and camera position
- submit/drain compile work
- expose CPU section updates for GPU upload
- expose current cached sections for startup/headless validation

It should not own:

- `winit` event loops
- browser DOM/canvas objects
- `wgpu::Surface`, `Device`, `Queue`, or swapchain configuration
- desktop UI/debug panels
- JavaScript promises or browser fetch

### GPU Upload/Render Adapter

Keep using `mclone-render::chunk::TexturedSectionDrawResources`, but define a
small shared handoff shape around it:

```text
RenderSectionGpuUpdate {
  rebuilt_sections
  removed_section_keys
  stats
}

FrameRenderInput {
  render_view
  render_target
  sky/time/light options
}
```

Desktop and WASM both call the same GPU update methods, while platform adapters
provide the concrete target/view and frame lifecycle.

## Refactor Sequence

### 1. Extract Shared CPU Render-Section Cache

Move the following out of `mclone-native-client/src/render_cache.rs`:

- `CachedTexturedRenderSections`
- `RenderSectionCacheUpdate`
- snapshot-to-`TexturedChunkMeshInput` helpers
- build-report merge/removal logic

Preferred landing place:

- a new small crate such as `native/crates/mclone-render-session`, or
- a shared module under an existing native crate if that avoids premature crate
  churn.

Acceptance:

- native desktop still compiles and renders unchanged
- WASM smoke can call the shared section cache instead of its local mini cache
- no `wgpu`, `winit`, `web_sys`, or filesystem assumptions in the extracted code

First extraction result:

- Added `native/crates/mclone-render-session`.
- Moved shared snapshot-to-textured-section build helpers into that crate:
  - `build_client_textured_sections`
  - `build_render_sections_from_snapshots`
  - `snapshot_mesh_block_state_ids`
  - `MeshChunkBlocks`
- Moved CPU render-section cache/update data into that crate:
  - `CachedTexturedRenderSections`
  - `RenderSectionCacheUpdate`
  - `RenderSectionCompileRequest`
  - `RenderSectionCompileResult`
- Left desktop-only asset discovery and the native `std::thread` compile worker
  in `mclone-native-client` for now.
- Updated desktop to use the shared build/cache/update types with no intended
  behavior change.
- Updated WASM generated-chunk rendering to use the shared
  `CachedTexturedRenderSections` merge/removal path instead of its local section
  key bookkeeping.

### 2. Split Compile Policy From Cache Policy

Move `RenderSectionCompileWorker` behind a compiler policy boundary.

Acceptance:

- desktop uses the existing native thread worker
- WASM has the same compiler interface and stale/revision lifecycle as desktop
- WASM may use a synchronous compiler only as a temporary fallback/smoke path
- the intended browser implementation is a worker-backed compiler using Web
  Workers plus shared Wasm memory, with COOP/COEP requirements documented before
  enabling it by default
- stale-result and revision checks stay shared
- tests cover both inline and worker-shaped paths where practical

Compiler interface result:

- Added `RenderSectionCompiler` to `mclone-render-session` with the shared
  submit/drain/pending job lifecycle.
- Moved render compile accepted/stale revision partitioning into
  `RenderSectionCompileResult::partition_by_revision(...)`.
- Updated the desktop `RenderSectionCompileWorker` to implement the shared
  trait while keeping the existing `std::thread` / `mpsc` execution.
- Updated `WindowSceneRuntime` to use the shared revision partitioning for
  stale compile results instead of carrying desktop-only logic.

Browser worker payload result:

- Added a browser render-compiler worker that compiles packed
  `TexturedRenderSectionBuildReport` bytes off the main browser thread.
- Added a `WebChunkRenderSession` packed-section render entrypoint that decodes
  the worker payload on the main WASM instance, applies it through
  `CachedTexturedRenderSections`, uploads GPU section resources, and presents
  through the existing main-thread WebGPU path.
- Kept the transferred packed bytes private to the smoke page so the DOM status
  and Playwright result stay compact.
- The Playwright chunk smoke now proves the first browser render consumes the
  worker-built section payload.
- Replaced the one-shot worker probe with a persistent browser worker compiler
  client that tags compile jobs with request IDs, keeps pending-job state, and
  feeds both the initial upload and the second incremental render update through
  `renderChunkReportFromPackedSections`.
- The browser smoke no longer accepts inline compile fallback for the two-step
  chunk render. The remaining architectural gap is moving this orchestration out
  of the JS smoke harness and into the shared Rust render-session/compiler
  lifecycle with revision/stale-result handling.
- Added Rust-owned browser compile request lifecycle methods on
  `WebChunkRenderSession`: begin request, finish request, and pending job count.
  The browser page now asks the session for request IDs and target section
  counts instead of inventing worker request identity in JS.
- Finishing a browser worker payload now wraps the packed mesh report in
  `RenderSectionCompileResult` and uses the shared
  `partition_by_revision(...)` acceptance path before applying sections to
  `CachedTexturedRenderSections`.
- The Playwright chunk smoke now asserts request/result ID matching,
  submitted/accepted/stale section counts, and zero pending jobs on both the JS
  worker client and Rust web session after the two-step render.
- Extracted platform-neutral render-session primitives into
  `mclone-render-session`:
  - `RenderSectionViewSync`
  - dirty chunk / removal chunk diff helpers
  - snapshot-to-render-section-key helpers
  - dirty-chunk target section selection
  - `RenderSectionCompileRequestState`
  - `RenderSectionCompileAcceptanceReport`
- Updated the WASM web session to use those shared primitives for request IDs,
  pending job counts, section revision snapshots, dirty target section
  selection, and ready-key selection.
- Updated the native desktop runtime to use the shared render-section key and
  snapshot containment helpers, removing another local copy of that policy.
- Extracted `RenderSectionDirtyState` into `mclone-render-session` to own dirty
  chunks, dirty sections, inflight sections, and render-section revision
  snapshots.
- Updated native desktop `WindowSceneRuntime` to delegate dirty marking,
  section-block mutation dirtying, compile request construction, compile
  submission tracking, completed result acceptance, and stale revision
  partitioning to the shared dirty state.
- Added shared dirty-state tests covering chunk-neighborhood invalidation,
  inflight compile tracking, and stale compile result detection.
- Extracted shared dirty-work classification and ready/deferred compile-budget
  planning into `mclone-render-session`:
  - `RenderSectionDirtyWork`
  - `RenderSectionNeighborReadiness`
  - `RenderSectionReadyPlan`
  - `classify_render_section_dirty_work`
  - `plan_ready_render_sections`
- Updated desktop `WindowSceneRuntime::sync_render_sections_with_budget` to
  delegate stale/removal cleanup and ready/deferred section selection to those
  shared planner primitives while keeping native camera-distance ordering and
  neighbor snapshot probes in the desktop adapter.
- Added shared planner tests covering loaded/removal/stale classification,
  budgeted ready/deferred section selection, near-camera exceptions, inflight
  deferral, and dirty-state updates after a ready plan.
- Updated `WebChunkRenderSession` to use `RenderSectionDirtyState` plus the
  shared dirty-work classifier and ready planner for browser worker compile
  request selection.
- Kept the browser JS worker protocol stable while moving request target
  revisions to the shared dirty state; `RenderSectionCompileRequestState` now
  accepts a prepared compile request so it can remain the request-ID/pending
  queue without becoming a second revision authority.
- Browser worker results now finish through
  `RenderSectionDirtyState::accept_completed_compile_result`, requeue stale
  sections when needed, and discard removal dirty work only after the CPU cache
  update is applied for GPU upload.
- The Playwright chunk smoke still validates the same incremental behavior:
  first worker request builds 144 sections, the second streamed request targets
  96 sections, uploads only changed sections, removes stale resident sections,
  and leaves both JS and Rust pending job counts at zero.

### 3. Extract Platform-Neutral Runtime Session

Peel `WindowSceneRuntime` into:

- shared `EngineRenderSession`
- desktop `WindowSceneRuntime` adapter/wrapper
- web `WebRuntime` or browser session adapter/wrapper

The shared session should own chunk-view/runtime/render-cache policy. The
platform wrappers should own asset selection, platform IO, surfaces, input, and
presentation.

Acceptance:

- native desktop `ChunkApp` behavior is unchanged
- headless captures and movement/timedemo smokes still use the shared session
- WASM generated-chunk smoke no longer has local dirty-section policy

### 4. Add Browser App Loop Over Shared Session

Only after the shared session exists, replace the two-shot WASM smoke shape with
a minimal browser runtime loop:

- `requestAnimationFrame`
- persistent session
- camera position/orbit controls
- chunk-view updates from camera movement
- shared render-section sync/update
- WebGPU present each frame

Acceptance:

- Playwright can still run deterministic two-step validation
- a manual browser page can move the camera and see streamed terrain update
- counters come from the shared session, not web-only bookkeeping

### 5. Add Web Worker/Thread Policy

Browser workers are part of the target architecture, not optional polish. The
compiler/session interface may land before worker packaging, but any inline WASM
compiler must remain an explicit fallback/smoke path. The worker slice should
evaluate:

- `wasm-bindgen` worker packaging
- transferable payload shape
- `SharedArrayBuffer` requirements
- browser COOP/COEP headers
- cancellation/stale result behavior
- Playwright validation that CPU compile work does not block the frame path

First threading gate result:

- The native web smoke server now serves COOP/COEP/CORP headers so browser
  smokes run cross-origin isolated.
- The web smoke page validates that `SharedArrayBuffer`, shared
  `WebAssembly.Memory`, `Atomics`, and a module `Worker` are available.
- The smoke worker mutates shared Wasm memory through `Atomics`, and Playwright
  fails the smoke if the main page does not observe the worker mutation.
- `native:web:smoke`, `native:web:canvas-smoke`, and `native:web:chunk-smoke`
  now require this threading gate by default; `native:web:thread-smoke` is the
  explicit named lane.

Render compiler worker payload result:

- Added a packed render-section build-report codec in `mclone-render-session`
  for section keys, visibility bits, textured vertices, indices, and visibility
  graph stats.
- Added worker-callable WASM exports that compile generated chunk render
  sections from packed asset bytes and return the packed payload plus a compact
  summary decoder.
- Added `mclone-render-compiler-worker.js`, a browser module worker that imports
  the same wasm-bindgen bundle, runs the Rust section compiler off the page
  thread, and transfers the packed payload back as an `ArrayBuffer`.
- `native:web:chunk-smoke` now asserts that this worker compiles the radius-1
  generated area into 144 section payloads before the main-thread WebGPU render
  smoke runs. The main thread still owns GPU upload and presentation.

## Non-Goals

- No Gradle, Android, or OpenXR scaffolding in this slice.
- No legacy TypeScript engine edits.
- No JS renderer.
- No separate WASM-only render cache policy.
- No broad renderer rewrite.
- No attempt to make `winit` or browser event loops share one abstraction before
  the engine session boundary is clean.

## Validation

Every implementation slice under this plan should run the narrowest relevant
set, usually including:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-web-client -p mclone-native-client -p mclone-render
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:web:thread-smoke
pnpm --silent native:web:chunk-smoke
```

For rendered-output changes, inspect screenshots saved under `/tmp`, especially:

```text
/tmp/mclone-native-web-canvas.png
/tmp/mclone-native-web-smoke.png
/tmp/mclone-native-debug.png
```

Native rendered slices should continue to use the existing native screenshot and
movement/timedemo lanes as appropriate:

```text
pnpm native:movement:smoke
pnpm native:timedemo:smoke
```

## Completion Criteria

This parent plan is complete when:

- desktop and WASM use the same CPU render-section cache/update logic
- desktop and WASM use the same engine render-session policy for chunk-view
  updates, dirtying, and compile-result merging
- platform adapters only own platform IO, surface/device lifecycle, input, and
  presentation
- WASM smokes no longer duplicate desktop render-section policy
- native desktop smokes, WASM smokes, and screenshot validation stay green

## Next Tactical Slice

The next slice should extract the next layer up from duplicated desktop/web
session orchestration into a shared engine-session skeleton:

1. Define a shared render-session sync method that owns the sequence
   `mark dirty -> classify -> plan ready -> submit request -> finish accepted
   result -> apply cache update`, while platform adapters provide compiler
   submission, snapshot lookup, ordering, and readiness probes.
2. Move the desktop `sync_render_sections_with_budget` and browser
   `begin/finishChunkRenderCompileRequest` paths toward that shared method
   instead of each spelling the sequence locally.
3. Keep desktop native threads and browser Web Workers as separate compiler
   adapters behind the same submit/drain lifecycle.
4. Preserve the current Playwright worker/canvas smoke and native movement or
   timedemo smoke as the compatibility gates for each extraction.
