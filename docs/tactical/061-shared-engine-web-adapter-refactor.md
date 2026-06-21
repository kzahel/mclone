# 061: Shared Engine / Web Adapter Refactor

Status: active; shared update dirtying, browser app loop, first-person web
camera/input, resize, browser no-clip pose sync, and browser sky/actor drawing
landed; browser walking/collision mode, block interaction, and hotbar selection
landed; browser target preview landed; desktop no-clip/mouse-look helpers
converged.

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
- Extracted the next orchestration layer into `mclone-render-session`:
  - `RenderSectionSyncPlan`
  - `prepare_render_section_sync_plan`
  - `finish_render_section_compile_result`
  - ready-plan request build/submission helpers on `RenderSectionDirtyState`
- Updated desktop and web to use the shared sync-plan preparation and compile
  finish paths. Platform adapters still provide snapshot lookup, ordering,
  readiness probes, compiler execution, and GPU upload/presentation.
- Added shared tests for sync-plan stale cleanup/ready selection and
  compile-finish accepted/stale reporting.
- Added `RenderSectionSession` to `mclone-render-session` as the first small
  shared owner for CPU render-section cache plus dirty/inflight/revision state.
- Updated desktop `WindowSceneRuntime` and web `WebChunkRenderSession` to route
  dirty marking, dirty-work classification, sync-plan preparation, compile
  request build/submission, compile-result finish, cache merge, stale requeue,
  removal cleanup, and cached-section reads through `RenderSectionSession`.
- Kept desktop native thread execution and browser worker/packed payload
  execution in the platform adapters; this chunk only moved shared state
  transitions behind one Rust owner.
- Added a shared session test covering dirty mark, compile request submission,
  accepted compile finish, cache update, removal cleanup, and dirty-state
  clearing.
- Added session-level helper APIs for the repeated adapter sequences:
  - `mark_chunk_dirty_with_loaded_sections`
  - `known_section_keys_for_chunk`
  - `submit_ready_plan_compile_request`
  - cache-aware `requeue_stale_sections`
- Updated desktop `WindowSceneRuntime` and web `WebChunkRenderSession` to use
  those helpers instead of carrying local known-section-key collection,
  stale-section requeue loops, or manual build/accept compile-submission
  sequences.
- Kept compiler execution platform-owned: desktop still submits to the native
  `std::thread` worker, and web still routes browser worker packed payloads
  through the Rust session API.
- Added shared tests proving known-key collection includes loaded/cached/dirty
  and inflight sections, stale requeue only keeps loaded/cached sections, and a
  failed platform submit leaves ready work dirty rather than marking it
  inflight.
- Added `RenderSectionRemovalMode` and `RenderSectionSyncUpdate` so the shared
  session can run the common dirty-work classification, adapter ordering,
  sync-plan preparation, removal handling, and ready-plan reporting sequence.
- Updated desktop `WindowSceneRuntime::sync_render_sections_with_budget` to use
  `RenderSectionSession::prepare_sync_update` while keeping native
  distance-based ordering, camera-neighbor readiness, pending native worker
  gating, and native thread submission in the desktop adapter.
- Updated web `WebChunkRenderSession::prepare_chunk_render_plan` to use the
  same helper while keeping coordinate ordering, browser worker request
  submission, and deferred removal upload in the web adapter.
- Added shared tests for native-style immediate removal application and
  web-style deferred removals for combined rebuild/remove GPU uploads.
- Added `RenderSectionFinishedCompileUpdate`,
  `RenderSectionSession::finish_compile_update`, and
  `RenderSectionSession::drain_completed_compile_updates` so accepted/stale
  compile-result handling, stale-section requeue, CPU cache mutation, deferred
  removal application, and pending native-worker job reporting live behind the
  shared session surface.
- Updated desktop `WindowSceneRuntime` to drain the native compile worker and
  hand the completed batch to the shared session instead of locally finishing,
  requeueing, and applying each result.
- Updated web `WebChunkRenderSession` to finish browser worker packed payloads
  through the same shared helper before GPU upload/presentation, while keeping
  JS/Rust request IDs and packed report decoding in the browser adapter.
- Added shared session tests covering accepted-plus-stale compile results and
  removal-only updates with no accepted sections.
- Added shared dirty-mark helpers on `RenderSectionSession`:
  - `mark_chunks_dirty_with_loaded_sections`
  - `mark_chunk_neighborhood_dirty_with_loaded_sections`
  - `mark_view_sync_dirty_with_loaded_sections`
  - `apply_loaded_view_sync`
- Updated desktop `WindowSceneRuntime` to use the shared neighborhood dirty
  helper for chunk snapshots, unloads, remote resyncs, and initial render-cache
  seeding while preserving the native changed-chunk-plus-neighbors policy.
- Updated web `WebChunkRenderSession::prepare_chunk_view` to compute
  previous/current loaded chunk sync and mark dirty/removal chunks through the
  shared session helper, deleting the web-local `mark_render_sync_dirty` loop.
- Added shared session tests for native-style neighborhood dirtying and
  web-style loaded-view sync dirtying with cached removal sections.
- Added `RenderSectionReadyWorkSubmission` and
  `RenderSectionSession::submit_prepared_sync_plan` so prepared sync plans now
  share empty-ready-plan application, ready-update counter reporting, compile
  request construction, submit-success dirty/inflight transitions, and
  submitted-section reporting.
- Updated desktop `WindowSceneRuntime::sync_render_sections_with_budget` to use
  the shared prepared-plan submission helper while preserving the native
  pending-worker gate, distance ordering, and threaded compiler submission.
- Updated web `WebChunkRenderSession` to carry the full
  `RenderSectionSyncPlan` through its browser request context and use the
  shared helper for both persistent worker requests and inline packed-report
  fallback requests.
- Added shared tests proving empty/deferred prepared sync plans do not invoke a
  platform submit and ready prepared sync plans become inflight only after the
  platform submit succeeds.

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

First `EngineRenderSession` shell result:

- Added `EngineRenderSession` in `mclone-render-session` as the shared owner for
  `ClientRuntime` plus `RenderSectionSession`.
- The shell now owns common client-backed render-session operations:
  - dirtying changed chunk neighborhoods from loaded client snapshots
  - clearing the client replica for resync while preserving render invalidation
  - initial render-cache dirty seeding from loaded chunks
  - loaded-view sync dirty/removal marking
  - completed compile result draining/filtering
  - sync-update preparation
  - prepared sync-plan submission
  - compile-result finish/cache merge/removal handling
  - cached CPU section reads
- Updated `WebRuntime` to own the shared shell, and removed the separate
  `RenderSectionSession` from `WebChunkRenderSession`.
- Updated desktop `WindowSceneRuntime` to own the shared shell and route render
  dirtying, initial cache seeding, sync preparation, compile drain, compile
  submission, compile finish, and cached-section reads through it.
- Kept platform-owned boundaries intact:
  - desktop still owns local/remote transport, integrated-server polling, native
    threaded compile worker, `winit`, and GPU upload/presentation
  - web still owns browser fetch/DOM/canvas, JS worker request transport, packed
    worker payload decoding, and WebGPU upload/presentation
- Added a shared `EngineRenderSession` test that hydrates a real
  `ClientRuntime` snapshot, prepares ready render work through the shell, and
  verifies the resulting compile request/inflight transition.

Shared update dirtying result:

- Added `EngineServerUpdateReport` and `EngineServerUpdateDirtyPolicy` to
  `mclone-render-session`.
- Moved section-block dirty key calculation out of desktop
  `scene_runtime.rs` into the shared engine session layer.
- Desktop now uses the shared server-update classifier and dirty-marker while
  preserving native timing fields and the native chunk-neighborhood dirtying
  policy.
- Web loopback now applies server updates through the shared helper with
  `SECTION_BLOCK_UPDATES_ONLY`, so loaded-view sync still owns chunk add/remove
  invalidation and block deltas share the same dirty-section policy as desktop.
- Added shared tests for block-delta boundary dirtying, full update
  application, and web-style snapshot dirtying that is intentionally delegated
  to loaded-view sync.

### 4. Add Browser App Loop Over Shared Session

Only after the shared session exists, replace the two-shot WASM smoke shape with
a minimal browser runtime loop:

- `requestAnimationFrame`
- persistent session
- camera position/input controls
- chunk-view updates from camera movement
- shared render-section sync/update
- WebGPU present each frame

Acceptance:

- Playwright can still run deterministic two-step validation
- a manual browser page can move the camera and see streamed terrain update
- counters come from the shared session, not web-only bookkeeping

First browser app-loop result:

- Added `native/apps/mclone-web-client/www/app.html` and
  `mclone-web-app.js` as a minimal manual browser app over the Rust/WASM
  session.
- The page owns a persistent `WebChunkRenderSession`, fetches the packed
  vanilla asset zip, submits browser worker compile jobs, uploads through
  WebGPU, and keeps presenting through `requestAnimationFrame`.
- The first landing used keyboard/buttons to move the chunk center one chunk at
  a time; it was useful as a persistent-session proof but was not first-person
  input.
- Added `native:web:app-smoke` for the persistent page, with Playwright
  screenshots saved under `/tmp`.

First-person web camera/input result:

- Added shared `EngineCameraController`, `EngineCameraInput`,
  `EngineCameraSnapshot`, and `EngineRenderCamera` primitives to
  `mclone-render-session`.
- The shared controller owns no-clip key state, mouse-look deltas, camera pose,
  speed clamping, render-camera generation, and chunk-center derivation.
- `WebChunkRenderSession` now exposes camera state, camera advancement,
  camera-derived worker compile requests, camera compile finish, and camera
  frame rendering as WASM methods.
- The browser app now runs a real RAF input/render loop: DOM events feed
  keyboard and mouse deltas to Rust, Rust advances the camera, the app keeps
  rendering from the current camera, and worker compiles are submitted only when
  the camera crosses into a new chunk view.
- The page requests pointer lock on canvas click and falls back to drag-based
  mouse deltas when lock is unavailable.
- `native:web:app-smoke` now clicks the canvas, exercises pointer-lock/fallback
  state, holds forward movement until the Rust camera crosses from the origin
  chunk, waits for the streamed loaded center to match the camera center with
  zero pending compile jobs, asserts the last streamed compile came from the
  browser worker, and captures page/canvas screenshots.
- The current web app still has no walking collision mode, pause/options menus,
  inventory UI, or multiplayer/server selection UI.

Resize and browser pose-sync result:

- Desktop `SpectatorCamera` now reuses shared render-session camera constants
  and `render_camera_from_snapshot(...)` for render-camera construction while
  preserving the existing native walking/collision path.
- `EngineCameraController` now exposes the same move-command and teleport
  correction hooks used by desktop `LocalPlayerController`, without moving
  transport ownership into the shared crate.
- `WebRuntime` now has a general `send_gameplay_command(...)` path that uses
  the same protocol encode/decode loopback and shared server-update application
  as chunk-view requests.
- `WebChunkRenderSession` applies pending player-position corrections to the
  browser camera, sends accept-teleport acknowledgements, and syncs no-clip
  camera position/rotation through gameplay commands.
- The web session exposes `resizeCanvas(width, height)`, and the browser app
  passes explicit display pixel dimensions into Rust so the canvas backing
  store, WebGPU surface, and depth target resize together.
- `native:web:app-smoke` now also asserts explicit resized canvas dimensions
  and gameplay command counts greater than the two chunk-view commands.

Sky and actor browser result:

- Actor texture atlas loading moved into `mclone-render::actor_assets`, so
  desktop and browser build the same cow/white actor atlas from whichever
  `AssetSource` their platform adapter owns.
- `WebChunkRenderSession` now owns a `SkyRenderer`, `ActorDrawResources`, and
  `ActorInterpolationState` alongside the textured section resources.
- Browser frames now render in the same pass order as desktop for the world:
  sky/time-of-day clear and dome, textured chunks with loaded color, then actors
  using the shared client actor presentations and packed-light probes.
- Local integrated desktop and web clients now seed their replica with the
  integrated server's initial day-time before the first frame, so both start
  from the same authoritative `dayTime=1000` morning clock.
- Web render reports expose day-time, celestial phase, sky-rendered state,
  actor counts, drawn actor index counts, and actor atlas dimensions.
- The browser HUD displays time and actor counters, and `native:web:app-smoke`
  now asserts visible sky-colored pixels plus `actors 1/1` from the starter cow
  path after browser pose sync.
- The two-shot chunk smoke also verifies the packed actor atlas and sky render
  state while keeping actor count diagnostic-only, since that path does not
  drive the full camera/spawn correction loop.

Walking/no-clip browser movement result:

- `EngineCameraController` now owns an explicit `WALK`/`NOCLIP` movement mode.
  The old no-clip `apply_input(...)` path remains for existing smokes, while
  the browser app uses the new mode-aware path.
- `WALK` mode calls the same `LocalPlayerController::tick_walking_movement`
  and `WalkingMovementStep` path desktop uses, against the web client replica
  for collision queries.
- Server player-position corrections now perform the same small ground probe in
  browser walking mode before acknowledging and resyncing the corrected pose.
- The browser app exposes `N` as the movement-mode toggle, maps Shift into the
  shared player input, and displays mode plus ground/collision state in the HUD.
- `native:web:app-smoke` now verifies a deterministic walking probe in `WALK`
  mode before toggling to `NOCLIP` for the existing chunk-streaming movement
  check. The captured app report showed a walking displacement of about `0.22`
  blocks, then `NOCLIP` streaming to chunk `(-1, -1)` with zero pending compile
  jobs.

Block interaction browser result:

- `WebChunkRenderSession` now owns a `ClientInteractionController` next to the
  shared camera controller.
- The browser `interactBlock("break" | "place")` export follows the desktop
  mouse-handler sequence: sync player pose, sync carried item if needed, raycast
  from the player eye/view through the client replica, build the
  `DebugInstantBreak` or `UseItemOn` command, and send it through the same
  gameplay command path.
- Browser mouse handling maps short left-clicks to break and short right-clicks
  to place after the initial pointer-lock/focus click. Dragging still drives
  camera look and does not accidentally interact.
- Interaction reports expose the action, block hit/miss, hit block position,
  hit/result block state IDs, direct command/update counts, and pending compile
  job count. The HUD displays the most recent target and action result.
- Browser number keys now select hotbar slots through
  `ClientInteractionController::select_hotbar_slot`, matching desktop's
  zero-based `Digit1` through `Digit9` slot mapping. Camera and interaction
  reports expose `selectedHotbarSlot`, and the HUD displays the selected slot as
  a one-based player-facing value.
- `EngineCameraController::pick_block` now owns the camera-facing pick primitive
  for shared consumers. Browser target preview and browser interaction both use
  that helper so the eye/view ray stays tied to the same player pose.
- The browser app calls `previewBlockTarget` every frame and displays the
  current non-mutating raycast target separately from the last action result.
  The preview report includes hit/miss, block position, hit block state,
  selected slot, command/update counters, and pending compile jobs.
- `native:web:app-smoke` now proves the browser can break and place through
  this path before toggling to no-clip, then selects slot `2` before placing.
  The captured report first previewed `(-1, 93, 1)` twice without changing
  command/update counters, then broke that block to air, selected zero-based
  slot `1`, synced the carried item, placed after hitting `(-1, 93, 3)` with
  result block state `5` (dirt), accepted two recompiled render sections for
  each interaction, and ended with zero pending compile jobs.

Desktop camera convergence result:

- `EngineCameraController` now exposes shared primitives for no-clip stepping,
  mouse-delta turning, and scroll-wheel speed adjustment.
- The web controller continues to use those primitives internally, while the
  native desktop app now calls them from its no-clip movement path, cursor
  fallback look path, raw mouse-motion look path, and spectator speed
  adjustment.
- The desktop walking path still owns its existing `LocalPlayerController`
  tick, collision, correction, and interest-update behavior. This slice does
  not replace the native app's player owner; it removes duplicated behavior
  around the desktop no-clip path first.
- Validation covered native unit tests, movement smoke, timedemo smoke, a native
  rendered screenshot, WASM target check, and the browser app smoke. The browser
  app smoke still proved target preview plus selected-slot break/place with zero
  pending compile jobs after the shared helper extraction.

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
pnpm --silent native:web:app-smoke
```

For rendered-output changes, inspect screenshots saved under `/tmp`, especially:

```text
/tmp/mclone-native-web-canvas.png
/tmp/mclone-native-web-smoke.png
/tmp/mclone-native-web-app.png
/tmp/mclone-native-web-app-canvas.png
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

The next slice should reduce desktop/web ownership drift now that desktop
no-clip and browser movement/targeting/interaction use shared controller
primitives:

1. Move duplicated desktop/web movement-mode reporting and camera-state
   serialization behind shared render-session/controller helpers where the
   ownership boundary is already clear.
2. Evaluate whether desktop can hold an `EngineCameraController` facade around
   the existing local player without disrupting walking/collision corrections,
   or whether the current helper-level convergence is the right intermediate
   shape for one more slice.
3. Keep `native:web:app-smoke` proving walk + no-clip movement, target preview,
   selected-slot break/place, section dirty/update publication, worker compiles,
   sky/actors, resize, screenshots, and zero pending compile jobs.
4. Keep browser worker compile submission, shared dirty/session policy, sky,
   actors, resize, pose sync, movement modes, and WebGPU presentation unchanged;
   leave pause/options/inventory and multiplayer/server selection as later UI
   slices.
