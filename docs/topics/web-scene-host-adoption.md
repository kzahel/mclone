# Web Scene-Host Adoption

Topic: web-scene-host-adoption

Status: active 2026-07-10. Tactical 170 Slices 0-4 landed the executable
browser baselines, portable scene prerequisites, neutral scene-session shell,
browser service adapters, and an isolated real-host browser proof. The direct
scene/web WASM gates and direct browser proof are green; Slice 5's atomic
production cutover is next. The implementation sequence is recorded in
[`170-web-scene-host-adoption.md`](../tactical/170-web-scene-host-adoption.md);
production adoption has not started, the proof remains smoke-only, and the old
web owner remains active.

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

## Current State (Verified 2026-07-10)

The browser client is functional and already shares important lower-level
contracts, but the top-level orchestration is still forked:

- `native/apps/mclone-web-client/src/web_canvas.rs` is 7,017 lines after the
  Slice 4 proof exposed the runtime diagnostics needed by the shared host.
  `WebChunkRenderSession` owns runtime/session state, camera/input assembly,
  settings-effect dispatch, catalog/session dispatch, render synchronization,
  GPU uploads, actors, UI, and the sky-to-present frame sequence.
- `native/apps/mclone-web-client/www/mclone-web-app.ts` owns the rAF loop,
  canvas/input event collection, promise serialization, render-compiler wake
  relay, and public browser state. The rAF and browser-resource portions are
  the correct future driver rim; the policy relay portions should shrink.
- `WebIntegratedServerRunner` already implements the shared
  `IntegratedServerRunner` contract and uses a Web Worker.
- `WebRenderSectionCompiler` already implements the shared
  `RenderSectionCompiler` contract over the resident shared-memory compiler
  ring. It uses the same budget-one dirty/revision/acceptance loop as native.
- Worldgen and light job workers, bounded shared-buffer pools, transport
  metrics, and stress/fallback coverage are already landed through Tactical
  062. Render-compiler streaming, resident snapshot deltas, and ABI locks are
  landed through Tactical 067.
- Local worker and remote WebSocket modes already drain normal-frame updates
  through the shared `ClientConnection` contract.
- Client-experience, session, and catalog policy are shared, but web still
  applies their effects through app-local matches and asynchronous wrappers.

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
acceptance; Slice 4's direct browser proof now supplies that evidence.

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
  reconnect, retry/terminal, supersession, and stale-completion states. The
  Slice 4 proof used the local worker; Slice 5 must preserve the already-green
  production remote smoke while routing it through the shared host.
- **Hidden-tab behavior:** resolved for cutover in Slice 4. Scene step and
  presentation pause while workers may continue. Visibility entry calls the
  host background hook; continuous worker-owned persistence means no immediate
  browser write. Resume starts at zero delta, then clamps to 50 ms and performs
  one budgeted host step per rAF.
- **Deferred drops:** resolved for direct-host resume. The Slice 3 bounded
  4,096-item browser backend reported zero resume backlog and no fallback in
  the Slice 4 proof; production cutover must preserve those diagnostics.
- **WebGPU device/surface loss:** resolved for first cutover as explicit
  `restart-required` owner recreation. Loss is terminal for that owner and the
  rAF busy guard is always released.
- **Audio activation:** resolved for first cutover as an absent capability.
  Promote it only after a real user-gesture activation and audible probe.
- **Web feature exceptions:** far LOD, travel assist, frame-pipeline overlay,
  debug diagnostics, and server cadence retain their current reason-bearing
  state until individually validated.
- **Physics engines sit outside the parity framework:** `physics-rapier` and
  `physics-box3d` are opt-in `mclone-server` cargo features exposed only
  through the desktop client, not `ClientExperienceProfile` entries, so the
  Slice 6 parity audit will not surface the native/web physics disparity. If
  a physics engine graduates from opt-in, give it an explicit profile or
  ledger entry before claiming web parity.
- **Slice 2 native regression risk:** the platform-matrix hold is closed.
  Desktop/native captures stayed byte-identical, flat Android rendered, and an
  attached Quest 3 reached a playable local world with no app-fatal marker.
- **Asset-pack work:** Tactical 169 also touches web asset payload and compiler
  lifecycle. Portable prerequisite work may proceed independently, but the
  atomic cutover must consume Tactical 169's active asset-set/epoch contract
  rather than invent a competing web resource owner.

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

## Recommended Next Work

1. Implement Tactical 170 Slice 5 only: atomically move local worker,
IndexedDB local-world, and remote WebSocket production modes onto
`McloneSceneHost`; consume Tactical 169's active asset-set/epoch contract;
preserve the Slice 4 visibility, loss, and capability decisions; delete
`WebChunkRenderSession`, duplicated policy, and the proof-only entry point;
enable the end-state enforcement tripwires; then stop before Slice 6's parity
audit.
