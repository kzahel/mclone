# Web Scene-Host Adoption

Topic: web-scene-host-adoption

Status: complete 2026-07-11. Tactical 170 Slices 0-6 landed the executable
baselines, portable scene prerequisites, neutral scene-session shell, browser
service adapters, direct-host proof, and atomic production cutover. Local
worker, IndexedDB local-world, and remote WebSocket modes now use one
`McloneSceneHost`; `WebChunkRenderSession` and the proof-only path are gone.
Tactical 169 Slices 5-6 adopted logical packs, transactional compiler epochs,
persistence, strict provenance, and reload diagnostics through this host.
The final audit added browser enforcement to the default purity gate, locked
the exact feature-gap ledger, removed obsolete aliases/dead compatibility
paths, stabilized actionable render-work accounting, and refreshed durable
operating/architecture docs. Structural adoption is closed. Tactical 171 —
Convergence And Parity Closeout Milestone D subsequently promoted Far LOD
through the same production host and worker/compiler seams without reopening
this adoption series.

Post-closeout feature follow-up (2026-07-14): Tactical
[`178-shared-web-lobby-scenario-parity.md`](../tactical/178-shared-web-lobby-scenario-parity.md)
owns the menu-lobby exception left by Tacticals 174-177. Structural scene-host
adoption remains closed; the follow-up generalizes native-shaped managed-world
and standby-start seams behind this same host, adds only unavoidable
IndexedDB/Worker adapters, and protects the ordinary browser single-world path
with explicit performance gates.

Post-closeout correction (2026-07-11): browser worker/socket construction had
been treated as gameplay-ready even though the authoritative underfoot chunk
could still be streaming. The shared scene host now keeps externally supplied
runtimes behind the same three-part startup-admission contract as native:
host/spawn authority, drawable coverage near the startup camera, and settled
presentation preparation. Web local authority requires the active interest
chunk snapshot (plus the richer playable-chunk diagnostics when the adapter
provides them); remote authority additionally requires drained initial
transport queues. Mono movement, gravity, and pose commits enforce admission
inside `McloneSceneHost`, so the browser cadence adapter cannot bypass it. The
CPU-throttled mobile smoke records the held and minimum pre-admission camera Y,
requires them to match, and waits for a grounded player after admission.

## Scope

Move the production Rust/WASM browser client from its app-local
`WebChunkRenderSession` orchestration onto
`mclone-scene::McloneSceneHost`, while retaining browser-owned cadence,
WebGPU canvas/surface ownership, DOM input, promises, Web Workers,
`SharedArrayBuffer` transport, IndexedDB execution, WebSocket construction,
asset fetching, and presentation.

This is the web completion of Tactical 168's native scene-host convergence.
It is not a browser renderer rewrite, a new worker topology, or a revival of
the retired TypeScript engine.

## Desired Outcome

All supported display clients use the same scene policy owner:

```text
browser events + requestAnimationFrame
  -> WebFrameDriver
       canvas/surface acquisition and presentation
       raw keyboard/mouse/touch translation
       promise, worker, IndexedDB, and WebSocket lifecycle
  -> McloneSceneHost
       session state and replacement policy
       camera/input/interaction semantics
       render admission, section sync/upload, and frame assembly
       actors, effects, UI/HUD, settings, diagnostics, and accounting
  -> injected browser services
       connection/runtime backend
       render compiler backend
       monotonic clock
       typed asynchronous operation executor
       deferred-drop and optional audio/teleport services
```

The browser may keep a small wasm-bindgen wrapper around the host, but that
wrapper must be a platform driver rather than a second game/runtime/render
orchestrator.

## Current State (Verified 2026-07-11)

The browser is now structurally converged on the same scene-policy owner as
the native display clients:

- `native/apps/mclone-web-client/src/web_scene_host.rs` is the production
  wasm-bindgen wrapper around one `McloneSceneHost`. It adapts browser runtime,
  compiler, catalog, clock, drop, input, and WebGPU target facts without owning
  a second session/render policy stack.
- `native/apps/mclone-web-client/www/mclone-web-app.ts` owns `WebFrameDriver`:
  rAF cadence, canvas/surface acquisition, DOM input collection, typed promise
  execution, visibility/resize events, public browser state, and presentation.
- Local worker, persistent IndexedDB local-world, and remote WebSocket session
  startup and replacement all flow through the same host and neutral external
  scene-session seam. Normal-frame updates still enter through
  `ClientConnection`.
- `WebIntegratedServerRunner`, worldgen/light workers, resident
  `WebRenderSectionCompiler`, shared-memory rings, bounded pools, ABI locks,
  transport metrics, and fallback probes retain their prior topology.
- The active prepared asset set stays in the long-lived host across session
  replacement. Production now fetches reference, authored, and fallback packs,
  projects their catalog into the shared host, and replaces the selected scene
  resources plus resident compiler transactionally at one asset epoch.
- Browser audio and teleport preview remain explicit absent capabilities. The
  reason-bearing web feature profile now has exactly four gaps after the
  separately proven Far LOD promotion.

The direct acceptance gate is green after browser service assembly:

```bash
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
```

The Slice 0 baseline was 98 compiler errors, Slice 1 reduced it to 87, and
Slice 2 reduced it to five. Slice 3 removed the remaining browser
connection/compiler/drop and prepared-asset roots. The final green log is
`/tmp/mclone-t170-slice3-scene-wasm.txt`; its only warnings are two pre-existing
`mclone-server` WASM warnings. Compile portability alone is not runtime
acceptance; the Slice 4 proof and Slice 5 production matrix supply that
evidence.

## Slice 0 Landed Evidence (2026-07-10)

- `mclone_app_runtime::platform_operation` locks request identity, unique IDs,
  epoch/stale rejection, duplicate/unknown observability, exact failure
  restoration, and teardown invalidation in four focused portable tests.
- `pnpm native:web:scene-host-adoption` inventories the current combined Rust
  and TypeScript owners in warning mode and has an enforcement mode reserved
  for the atomic deletion slice. The baseline is 6,454 Rust lines, 1,881
  TypeScript lines, and the exact policy-pattern counts recorded in Tactical
  170.
- Named executable pre-cutover gates now cover local worker
  (`native:web:app-smoke`), IndexedDB local world
  (`native:web:indexeddb-smoke`), and remote WebSocket
  (`native:web:remote-smoke`). All passed with the expected host/session
  identity, settled queues, resident compiler transport, rendered world, and
  shared UI flow.
- The IndexedDB gate persisted block state `5` across reload with 81 chunk and
  one entity-chunk record. Local worker transports remained shared-memory;
  remote runner transport remained WebSocket; compiler generated-view fallback
  and shared-result overflow were both zero.
- The three-chunk movement baseline advanced 68 frames/renders. Compile samples
  measured `6.7..19.0 ms` total, `3.9..11.2 ms` worker, `1.0..3.6 ms` apply,
  and `6.7..10.0 ms` compile-local max frame gaps. These values are evidence,
  not hard-coded budgets.
- Deferred-drop backlog is now observable. Settled endpoints measured 70 items
  for local worker, 295 for remote WebSocket, and zero after IndexedDB reload;
  an extended 16-chunk/7-unload run also ended at zero. Because current web has
  no explicit drain, Slice 2 must select a bounded observable backend from this
  path-dependent evidence rather than assume zero work.
- Current local/remote world and title UI, the IndexedDB persisted edit, and the
  extended movement/mobile layout were visually inspected in `/tmp`; no image
  was added to the repository.

No production owner, behavior, or feature-profile state moved in Slice 0.

## Slice 1 Landed Evidence (2026-07-10)

- `mclone_app_runtime::monotonic` now owns ordered `MonotonicInstant`, injected
  `MonotonicClockHandle`, and self-evaluating `MonotonicDeadline` contracts.
  Native assembly supplies the system-`Instant` adapter; shared scene startup,
  admission deadlines, locomotion deltas, and render attribution use only the
  neutral contract. Ordering, saturation, and deadline-expiry tests use a
  manual clock.
- `mclone_app_runtime::render_asset_data` is always compiled and owns
  `SceneTexturedSections`, `TextureAtlasImage`, `TexturedMeshAssets`, and
  source-backed CPU preparation. Filesystem discovery, native compile workers,
  and device upload remain outside it. Compile defaults are consumed from the
  target-neutral runtime root.
- Camera reconciliation is always compiled over `EngineCameraRuntime`, the
  narrow command/update/interest facts it needs. The native session runtime is
  one adapter, and a target-neutral fake-runtime test proves the policy has no
  native runtime dependency.
- `McloneSceneHost` owns `TeleportPreviewCapability` rather than
  `NativeTeleportPreviewWorker`. Native desktop, Android, desktop XR, and
  Android XR attach the lazy native service factory; unavailable capability is
  explicit and tested for the future browser assembly.
- Scene audio is an `AudioOutputCapability`. Platform assembly constructs and
  attaches `AudioEngine`; shared scene code emits neutral sound commands and
  asks the capability for transactional asset replacement. CPAL/device/stream
  construction is native-gated inside `mclone-audio`, whose prepared data and
  absent-output contract compile for WASM.
- Native desktop/offscreen, synthetic stereo, flat Android, desktop XR, and
  Android XR build/smoke gates passed. The inspected `/tmp` captures showed
  textured terrain and actors in desktop offscreen, distinct stereo forest
  views with both menu composites, and a live flat-Android terrain frame. No
  capture was committed.

Production web still uses `WebChunkRenderSession`; Slice 1 added no alternate
browser host or cutover toggle.

## Slice 2 Landed Evidence (2026-07-10)

- `McloneSceneHost` is no longer generic over a concrete remote session and
  stores `SceneSessionRuntime`, whose neutral shell owns session identity and
  host-facing runtime policy. Native assembly carries only an active descriptor
  plus concrete service ownership; the old `native_session_runtime` module and
  public `Native*Runtime` entry points were deleted.
- `native_service_assembly` now owns native runner, render-compiler,
  filesystem, and bounded drop-thread construction. Desktop, offscreen, flat
  Android, desktop XR, and Quest assembly all install those services through
  the same non-generic host API. The scene purity gate rejects concrete native
  service fields and policy matches in native assembly.
- The host's compact service container owns the injected monotonic clock,
  typed catalog operations, optional teleport preview, and optional audio.
  Connection/compiler/drop ownership is hidden behind the single
  `SceneRuntimeService` boundary rather than widening the public host type.
- Catalog UI policy remains shared, while native filesystem work completes
  through the typed `PlatformOperationService`. Immediate and deferred scripted
  executors produce identical session/catalog/reconnect host state; executor
  replacement cancels the old epoch and rejects late work.
- Deferred snapshot destruction uses `DeferredDropService`, with a portable
  frame-drain queue and native bounded-thread implementation. The hard capacity
  is 4,096 items versus Slice 0's measured 295-item peak; overflow destroys on
  the caller and increments visible inline-fallback accounting. Queue
  replacement and bound/fallback behavior are tested.
- The direct scene WASM audit fell from 87 errors to five. Those remaining
  roots named browser service/prepared-asset implementations reserved for
  Slice 3, not concrete native catalog or host policy ownership.
- Desktop offscreen and synthetic-stereo captures were byte-identical to the
  pre-slice canaries (`cdffd673...` and `18bbdadd...`) and were visually
  inspected. The rebuilt flat-Android AVD produced a visible frozen-daytime
  terrain/HUD frame. Desktop XR and Quest APK checks passed. The later attached
  Quest 3 validation closed the platform hold before Slice 3: the Oculus
  runtime reached `FOCUSED`, submitted a frame, reached playable 9/9, drew 67
  of 118 sections and both actors, emitted `MCLONE_ANDROID_XR_READY`, and had
  no app-fatal marker.

Production web still uses `WebChunkRenderSession`; Slice 2 added no alternate
browser host or cutover toggle.

## Slice 3 Landed Evidence (2026-07-10)

- `McloneSceneHost::with_scene_runtime` accepts an already-started neutral
  `SceneSessionRuntime`, `PreparedSceneAssets`, projected clock, typed catalog
  service, and explicit optional capabilities. Native filesystem/thread
  constructors remain native-only; browser-prepared assets cross the portable
  seam without making scene code fetch or parse JavaScript objects.
- `WebSceneRuntimeService` adapts the existing local worker or remote
  WebSocket connection, resident `WebRenderSectionCompiler`, and bounded
  deferred-drop queue to `SceneRuntimeService`. The compiler wake function and
  doorbell stay private to the browser adapter. Budget-one sync,
  dirty/revision acceptance, shared-result transport, normal-frame connection
  ingress, and the 4,096-item drop bound are preserved.
- `ProjectedMonotonicClock` clamps negative or regressing rAF observations.
  `WebSceneSessionLifecycle` gives local start, remote connect/reconnect,
  shutdown, supersession, retry, and terminal failure explicit epoch-safe
  states. Deferred typed catalog operations accept out-of-order completions
  while rejecting superseded and post-teardown work.
- Browser teleport preview and audio remain explicitly unavailable. Far LOD,
  cadence control, and worker-owned persistence also report their current
  capabilities honestly; Slice 3 did not promote any web feature exception.
- `pnpm native:web:scene-adapters` passed its five-bit contract report
  (`31`): monotonic clamp, superseded-completion rejection, explicit reconnect,
  typed catalog completion, and post-teardown rejection. Its resident compiler
  used `shared-result-buffer` with no generated-view fallback or overflow, and
  both rendered centers settled.
- The unchanged production local, catalog, and remote smokes passed. Local
  remained `localWorld`, remote remained `remote-websocket`, and compiler
  transport remained resident shared-result-buffer. Production still uses
  `WebChunkRenderSession`; no alternate production host or cutover toggle was
  added.
- Shared-runtime/web tests passed (213 and 9 tests respectively), as did the
  workspace, direct scene/web WASM, browser build/typecheck, asset-lock, scene
  purity, thin-adapter purity, desktop offscreen, synthetic stereo, desktop XR,
  flat-Android AVD, Android XR APK, and attached Quest validations.
- `/tmp/mclone-t170-slice3-adapters-page.png` and
  `/tmp/mclone-t170-slice3-adapters-canvas.png` showed textured forest/ore
  terrain, sky, crosshair, and hotbar. The useful local, catalog, and remote
  page captures, native offscreen/stereo captures, and flat-Android capture
  were also inspected. Black/partial browser canvas-only artifacts were
  rejected rather than treated as visual evidence. No capture was committed.

## Slice 4 Landed Evidence (2026-07-10)

- A query-only `sceneHostProof=1` entry point constructs a real
  `McloneSceneHost` over the landed browser runtime services and current packed
  assets. It is smoke-only: production still constructs
  `WebChunkRenderSession`, and no old/new production toggle was added.
- The JavaScript rim retains rAF, DOM translation, compiler wake promises,
  canvas/surface acquisition, resize, visibility events, presentation, and a
  non-overlap busy guard. The host applies `FlatInputFrame`, owns scene sync
  and render admission, and assembles the shared sky, terrain, actors/effects,
  and UI/HUD frame.
- `pnpm native:web:scene-host-proof` reached `scene-host-proven` with 473 host
  frames/renders and a 10.365 ms maximum frame gap. It loaded 49 chunks and 218
  sections, drew 17 sections/97,278 indices and both actors, emitted 94 GUI
  commands, applied movement from six DOM/script input frames, changed a block,
  applied a shared setting effect, rendered pause UI, and resized to 960x540.
- The runner, worldgen, and light transports remained `shared-memory`. The
  resident compiler woke 72 times through `shared-result-buffer`, loaded the
  6,985-file asset pack once, and used neither generated-view fallback nor
  shared-result overflow.
- Hidden pages perform no host step or presentation. Visibility entry calls
  `on_background`; current worker-owned continuous IndexedDB persistence makes
  that lifecycle save a synchronous no-op. On resume the first delta is zero,
  later deltas clamp to 50 ms, and each rAF performs at most one default-budget
  host step rather than a catch-up loop. The eight-frame resume probe observed
  at most four updates per frame and zero deferred-drop backlog.
- Surface acquisition failure enters explicit `restart-required` state and
  requires recreation of the proof owner. A simulated loss remained terminal
  on the following frame, released the JavaScript busy guard, and still allowed
  controlled shutdown. Browser audio and teleport preview remain explicitly
  absent; no user-gesture audible probe exists.
- The full browser matrix remained green. The 16 movement samples measured
  5.9-17.9 ms total (9.4 ms average), 3.9-11.7 ms worker round trip (5.4 ms
  average), 1.1-3.7 ms decode/finish/apply (1.5 ms average), and 7.3-8.8 ms
  maximum frame gaps (8.3 ms average), consistent with Slice 0. IndexedDB again
  retained block state `5` across reload with 81 chunk and one entity record.
- `/tmp/mclone-native-web-scene-host-proof.png` and its canvas capture showed
  clean textured spruce terrain, water, sky, HUD, a centered cow, and a chicken.
  Local/app, mobile UI/options, catalog, block-edit, and remote page captures
  were also inspected. Partial IndexedDB and sky-only movement endpoint images
  were rejected as visual evidence. No capture was committed.
- Workspace, focused tests (213 app-runtime, 92 scene, and 11 web-client), both
  direct WASM checks, browser build/typecheck, asset lock, purity gates,
  desktop offscreen, synthetic stereo, and desktop XR checks passed. The native
  captures retained their Slice 3 hashes (`cdffd673...` and `18bbdadd...`) and
  were visually inspected. No shared public host contract changed, so the
  Slice 3 flat-Android and attached-Quest canaries remain the device evidence.

## Slice 5 Landed Evidence (2026-07-11)

- The production `WebSceneHost` wrapper owns one `McloneSceneHost`, while
  `WebFrameDriver` owns only browser cadence, DOM/input translation, typed
  promise execution, canvas/surface resources, and presentation. Local worker,
  persistent IndexedDB world, and remote WebSocket replacement all use that
  host.
- `WebChunkRenderSession`, its duplicated policy/support surface, the direct
  proof module, query flag, and proof-only package command are deleted. There
  is no old/new runtime toggle.
- The default adoption gate enforces zero competing Rust/TypeScript owner,
  settings/session/transition/runtime-install policy, camera semantics, render
  admission/frame assembly, async dispatch/restart, or compiler-wake relay
  matches. Its current inventory is 5,299 Rust and 1,890 TypeScript lines.
- The final full browser matrix passed. Local runner/job transports remained
  shared-memory, remote remained WebSocket, and the compiler remained resident
  on `shared-result-buffer` with one 6,985-file pack load and no generated-view
  fallback or shared-result overflow. Deferred-drop backlog ended at zero.
- IndexedDB retained placed block state `5` across reload with 121 chunk and
  two entity-chunk records. Sixteen movement samples measured 6.6-17.7 ms
  total (13.3 ms average), 6.5-17.7 ms worker round trip (13.3 ms average), and
  6.9-9.3 ms maximum frame gaps (8.2 ms average).
- Focused app-runtime and scene suites passed 213 and 92 tests plus the startup
  contract. Direct scene/web WASM, browser typecheck, asset lock, source-shape,
  scene-host purity, and thin-adapter purity gates passed.
- Final desktop world/HUD, portrait touch, nested options, and catalog captures
  were visually inspected. Native offscreen, synthetic stereo, and fresh flat
  Android captures were also inspected. The Quest APK rebuilt; no headset was
  visible after restarting `adb`, so the current slice does not claim a fresh
  attached-device result. No capture was committed.
- At the production cutover, the long-lived host retained prepared asset epoch
  0 across session replacements. Browser pack discovery/selection and
  transactional compiler reinitialization were intentionally deferred to
  Tactical 169 Slice 5 and have since landed through that host.

## Locked Decisions

### One policy host

The production web app adopts `McloneSceneHost`; it does not gain a second
"web scene host" with copied policy. The existing worker and WebGPU mechanics
become services and driver code around the shared host.

### Nonblocking host

`McloneSceneHost` remains synchronous and step-based. It never awaits a JS
promise, blocks for startup, or owns a browser event loop. Browser operations
cross the boundary as typed request IDs and later completions.

### Neutral runtime shell, not generic proliferation

Split the current native session runtime into a platform-neutral scene-session
shell plus small injected service contracts. Do not permanently widen the
public host into a stack such as `McloneSceneHost<S, R, C, D, ...>` merely to
encode every backend in its type. Native and browser constructors may remain
platform-specific, but runtime/session/render policy must compile once.

The exact internal representation may use trait objects, private generics, or
an enum where measurement justifies it. The public ownership shape and the
small service boundaries are the contract.

### Typed async request/completion protocol

Worker construction, WebSocket connection/reconnection, IndexedDB catalog
work, and graceful shutdown are asynchronous browser facts. The host emits a
typed operation with an epoch/request ID; the web adapter starts the promise
and later submits a typed success/failure completion. Stale or superseded
completions are rejected. Native services may complete the same operations
immediately.

No browser adapter may regain a `SessionStartRequest` policy match merely
because the operation is asynchronous.

### Existing worker/compiler topology survives

The web server runner, worldgen/light workers, render compiler, shared-memory
rings, bounded pools, ABI locks, and transport metrics are investments to
reuse. Adoption must not replace them with inline WASM, whole-view compile
transactions, or a promise-owned scheduling loop.

The web render-compiler backend owns its worker wake mechanism internally
(directly or through a supplied wake sink). `mclone-scene` sees only the
platform-neutral compiler contract and never handles `JsValue`, `Worker`, or
`SharedArrayBuffer` objects.

### Browser cadence remains browser-owned

`requestAnimationFrame`, visibility/focus events, canvas resize, WebGPU
surface acquisition, and final presentation stay in the web driver. rAF time
is projected into a shared monotonic clock/frame-time contract; scene code does
not call browser APIs.

### Prepare, prove, then cut over atomically

Portable prerequisites and browser service adapters may land while the current
production web path remains active. Once the direct host browser proof is
green, local worker, IndexedDB local-world, and remote WebSocket production
modes switch together and the old policy paths are deleted in the same slice.
Do not leave a long-lived old/new orchestrator toggle.

### Preserve the current web feature profile first

Host adoption does not automatically declare every currently reason-bearing
web feature supported. The first cutover preserves the web profile. Remove a
web exception only when its full browser plumbing, UI, diagnostics, and smoke
evidence land. Structural convergence and feature burn-down are related but
separately reviewable.

## Service Boundaries To Extract

| Service | Shared requirement | Browser implementation |
|---|---|---|
| monotonic clock/deadline | frame time, elapsed measurements, admission deadline without `Instant` | rAF/`performance.now()`-based clock |
| scene connection/runtime control | command/update pump, diagnostics, host identity, cadence capability, shutdown/reconnect state | `WebIntegratedServerRunner` or `WebSocketServerSession` adapter |
| render compiler | `RenderSectionCompiler` lifecycle and queue health | existing resident-SAB `WebRenderSectionCompiler` |
| deferred chunk drops | bounded handoff/backlog reporting without an OS-thread assumption | measured frame-budgeted queue or Worker backend |
| platform operation executor | start/reconnect/catalog/shutdown request IDs and completions | `spawn_local`/promise completion queue owned by web adapter |
| teleport preview | submit/poll/disable capability without concrete native worker | explicit unavailable mode initially or later Web Worker executor |
| active asset set | neutral mesh/actor/effect data; loaders stay platform-local | packed-byte fetch/parse and existing web compiler catalog setup |
| audio output | optional engine attachment with no host-side device creation | browser-compatible CPAL/WebAudio path after user-gesture validation |

## Known Gaps And Decision Deadlines

- **Compiler wake ownership:** resolved in Slice 3. The browser runtime-service
  adapter owns the JavaScript wake function and doorbell privately; shared
  scene code sees only the neutral compiler contract.
- **Remote reconnect:** the Slice 3 lifecycle protocol has nonblocking
  reconnect, retry/terminal, supersession, and stale-completion states. Slice 5
  preserved the remote smoke while routing it through the shared host.
- **Hidden-tab behavior:** resolved for cutover in Slice 4. Scene step and
  presentation pause while workers may continue. Visibility entry calls the
  host background hook; continuous worker-owned persistence means no immediate
  browser write. Resume starts at zero delta, then clamps to 50 ms and performs
  one budgeted host step per rAF.
- **Deferred drops:** resolved for production resume. The bounded 4,096-item
  browser backend reported zero final backlog and retained its diagnostics
  through the cutover.
- **WebGPU device/surface loss:** explicit `restart-required` owner recreation
  is retained in production. Loss is terminal for that owner and the rAF busy
  guard is always released.
- **Audio activation:** resolved for first cutover as an absent capability.
  Promote it only after a real user-gesture activation and audible probe.
- **Web feature exceptions:** travel assist, frame-pipeline overlay, debug
  diagnostics, and server cadence are the exact tested
  `WEB_FEATURE_PARITY_EXCEPTIONS` ledger. Far LOD left the ledger only after
  production local-worker, IndexedDB, remote-WebSocket, UI, diagnostics,
  shared-result worker, performance, and pixel gates passed. Each remaining
  row retains a named implementation and proof requirement until individually
  validated.
- **Physics engines sit outside the parity framework:** `physics-rapier` and
  `physics-box3d` are opt-in `mclone-server` cargo features exposed only
  through the desktop client, not `ClientExperienceProfile` entries, so the
  Slice 6 parity audit will not surface the native/web physics disparity. If
  a physics engine graduates from opt-in, give it an explicit profile or
  ledger entry before claiming web parity.
- **Slice 2 native regression risk:** the platform-matrix hold is closed.
  Desktop/native captures stayed byte-identical, flat Android rendered, and an
  attached Quest 3 reached a playable local world with no app-fatal marker.
- **Asset-pack work:** resolved for platform adoption in Tactical 169 Slice 5.
  Browser fetch/worker lifecycle stays in `WebFrameDriver`, while catalog,
  selection, preparation, replacement, and commit policy stay in shared Rust.
  The current synchronous main-Wasm CPU preparation fallback should move to a
  dedicated worker after timing evidence; it does not add a competing policy or
  active-resource owner.

## Validation Contract

Every implementation slice keeps these compile gates green:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml \
  -p mclone-app-runtime --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
```

The direct scene gate is mandatory and green after Slice 3:

```bash
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
```

Production adoption must preserve:

- local worker, thread/shared-memory, canvas/chunk, app-loop, catalog,
  mobile-input, block-edit, movement/perf, and remote-WebSocket browser smokes;
- resident render-compiler and runner/job shared-memory metrics, bounded pool
  behavior, and explicit fallback probes;
- visually inspected browser screenshots under `/tmp`;
- the native offscreen pixel canary and native thin-adapter/scene-host purity
  gates; and
- representative desktop, flat Android, desktop XR, and Quest checks after the
  neutral runtime shell or public host contract changes.

## Code And Documentation Map

- [`../tactical/170-web-scene-host-adoption.md`](../tactical/170-web-scene-host-adoption.md): implementation slices and acceptance.
- [`../tactical/168-unified-native-scene-host.md`](../tactical/168-unified-native-scene-host.md): parent host convergence and
  original web blocker audit; the neutral mesh-asset boundary (family 2 above)
  is newly identified in this topic and absent from that audit.
- [`../tactical/062-shared-threading-topology.md`](../tactical/062-shared-threading-topology.md): browser runner/worldgen/light worker topology.
- [`../tactical/067-shared-render-worker-architecture.md`](../tactical/067-shared-render-worker-architecture.md): resident shared-memory render compiler.
- [`../tactical/085-web-host-mode-convergence.md`](../tactical/085-web-host-mode-convergence.md): web local/remote host-mode seams.
- [`../tactical/151-remote-inbound-update-pipeline.md`](../tactical/151-remote-inbound-update-pipeline.md) and [`../tactical/154-client-ingress-adapter-cleanup.md`](../tactical/154-client-ingress-adapter-cleanup.md): normal-frame `ClientConnection` ingress.
- [`../tactical/165-native-feature-parity-baseline.md`](../tactical/165-native-feature-parity-baseline.md): reason-bearing web feature divergences.
- [`../tactical/169-runtime-asset-pack-selection.md`](../tactical/169-runtime-asset-pack-selection.md): active asset-set and compiler-epoch coordination.
- [`../native-web.md`](../native-web.md): browser build, smoke, and deploy commands.

Primary code:

- `native/crates/mclone-scene/`
- `native/crates/mclone-app-runtime/src/scene_session_runtime.rs`
- `native/crates/mclone-app-runtime/src/native_service_assembly.rs`
- `native/crates/mclone-app-runtime/src/client_connection.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/src/web_remote_session.rs`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`

## Slice 6 Closeout Evidence (2026-07-11)

The production cutover landed in `ffc33de6`; this closeout keeps all 13
browser old-owner/policy inventory counts at zero through the default
thin-adapter gate. The focused Rust suites passed 216 app-runtime, 107
render-session, 92 scene, and 11 web-client tests plus three ABI locks. Direct
scene/web WASM checks, workspace check, browser build/typecheck, desktop
offscreen, synthetic stereo, desktop-XR check, flat-Android arm64+x86_64 build
and AVD smoke, and the Quest APK build passed.

All ten required browser behavior lanes passed. Movement crossed three chunks
in 167 frames/renders; compile samples measured 6.7-18.7 ms total and 7.2-8.5
ms local maximum frame gaps. Shared-memory runner/worldgen/light transports,
resident shared-result compiler transport, zero generated-view fallback,
zero result overflow, zero final deferred-drop backlog, and remote WebSocket
transport were preserved. Reviewed `/tmp` captures covered desktop and mobile
world/HUD, actor/selection/debug effects, catalog UI, native offscreen,
synthetic stereo, and flat Android. No capture was committed. USB Quest was not
initially visible after an `adb` restart. Once it reappeared, the canary exposed
and fixed shell-owned external asset-directory writes: embedded first-party
packs now stage internally while the ADB-installed reference archive remains an
external discovery source. The rebuilt release APK passed on Quest 3 with
Oculus OpenXR/Adreno 740, discovered all three packs, reached a playable 9/9
target, and rendered 134 sections (37 drawn) plus two actors without an
app-fatal marker.

## Recommended Next Work

The adoption series is closed. Continue through Tactical 171 — Convergence And
Parity Closeout. Its Milestone D production browser proof passed and removed
only the `FarLod` ledger entry. Tactical 166 — Shared Resident-Tile Substrate
Slice 4 subsequently landed multi-level rings while preserving the proven
native and browser worker/admission/upload paths. The macro next step is
Tactical 162 — Real-Chunk LOD Reduction Draft Slice 4; keep the remaining four
web exceptions unchanged until their own complete proofs land.

## Mobile Touch Follow-Up (2026-07-12)

The scene-host cutover initially copied the browser `TouchOverlay` into
`MonoUiContext` but left `resolved_input` at its keyboard/mouse default. The
shared HUD correctly filtered the overlay out, so touch movement remained live
while the menu, action buttons, and active joystick were invisible. The web
adapter now projects touch visibility and prompt capability into the shared
input context. Browser glue also makes a best-effort fullscreen request from
the first touch gesture, applies `viewport-fit=cover`, and suppresses the
focusable canvas's default outline, which was the thin light border seen along
the screen edges.

Restoring the touch HUD exposed Chrome WebGPU's mapped-at-creation size limit
for a large textured section upload. Shared textured terrain uploads now use
unmapped `COPY_DST` buffers plus `Queue::write_buffer`, preserving the shared
renderer path for native and web. Focused UI/web Rust tests (79 + 11 plus the
three web ABI locks), web typecheck, and native render/web checks pass. The
mobile smoke proves touch movement/look/buttons, the pause shortcut, one-shot
fullscreen invocation, and exact horizontal viewport coverage. Its automated
pixel capture remains blocked by the already documented intermittent Chrome
WebGPU all-black readback artifact in this environment; runtime reports reached
playable rendering with 396 GUI commands and no page or WebGPU error after the
upload fix. The inspected `/tmp` captures confirmed the focus border was gone
but were otherwise affected by that black readback artifact.

Follow-up device feedback found that the fullscreen hook above was still
installed too late: `TouchControls` was constructed only after WASM, assets,
WebGPU, and the worker runtime were ready. It now installs at module bootstrap
and fires on the first touch/pen `pointerup`, which is the transient-activation
edge browsers grant for non-mouse pointers. The mobile smoke sends that tap
before scene-host initialization and locks one fullscreen invocation.

The same follow-up restored honest startup presentation. A platform bootstrap
status covers the pre-WebGPU phases, then retires after the first shared frame.
While the external web runtime is awaiting startup admission, `mclone-scene`
now feeds its worker-owned view-readiness snapshot into the same shared loading
grid used by native-owned startup. Web reports retain observed progress counts
after the worker diagnostics bridge carries compact loading/view cell grids
instead of dropping those detailed snapshots at the worker boundary. The
mobile smoke captures `/tmp/mclone-native-web-mobile-startup.png` at
that milestone. Chrome's canvas readback is still black in this environment,
but the live report proves a non-empty chunk target and GUI command list before
admission; the DOM bootstrap has already retired at that point.

Device feedback then exposed a high-DPI coordinate-space regression isolated
to the moving joystick visual. Browser touch input and overlay positions are
measured in canvas pixels, while the shared HUD draws in scaled GUI units. The
web adapter had passed the pixel positions through unchanged, placing the
active stick outside the visible GUI on typical mobile device-pixel ratios.
It now converts the base and thumb through the active `GuiScale`; the mobile
smoke locks that the active joystick reaches the shared HUD inside its bounds
and captures `/tmp/mclone-native-web-mobile-joystick.png` while held.
