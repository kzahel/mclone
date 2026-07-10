# 170: Web Scene-Host Adoption

Status: draft implementation plan 2026-07-10; Slice 0 is next. Production web
still uses `WebChunkRenderSession`; no adoption code has landed.

Topic: [`web-scene-host-adoption`](../topics/web-scene-host-adoption.md)

Workstream: shared native Rust and native web/WASM. The shared scene/runtime
contracts are the primary implementation surface; browser worker, promise,
WebGPU canvas, DOM input, and presentation glue remain in
`mclone-web-client`.

## Goal

Move every production browser host mode onto
`mclone_scene::McloneSceneHost` without weakening the existing browser worker,
shared-memory, rendering, catalog, or WebSocket architecture.

The finished browser path has one scene-policy owner:

```text
DOM events + requestAnimationFrame
  -> thin WebFrameDriver
       browser cadence, canvas/surface, raw input, promises, presentation
  -> McloneSceneHost
       session/camera/input/render/UI/effects/settings/accounting policy
  -> browser service adapters
       connection, compiler, clock, async operations, drops, assets, audio
```

Local worker, IndexedDB local-world, and remote WebSocket modes must all cross
the production cutover together. The old web orchestrator and its duplicated
policy matches are deleted in the cutover slice rather than retained behind a
long-lived compatibility toggle.

## Final Acceptance

This tactical is complete when:

- `mclone-scene` directly checks for `wasm32-unknown-unknown`;
- the production web rAF path owns a `McloneSceneHost` through a thin browser
  driver for local worker, IndexedDB local-world, and remote WebSocket modes;
- the shared host owns session replacement, camera/input semantics,
  interaction, render admission and section synchronization, frame assembly,
  effects, actors, settings, UI/HUD, diagnostics, and accounting on web;
- browser promises and workers use typed request/completion or existing
  platform-neutral service contracts, never async policy matches in the app;
- the resident shared-memory render compiler and existing server/worldgen/light
  worker topology remain in use with their bounded-pool and transport metrics;
- app-local settings, session-dispatch, camera-input assembly, render-sync,
  upload-admission, and sky-to-present orchestration are deleted;
- the current reason-bearing web feature profile is preserved unless a feature
  is separately proven and promoted;
- browser visual/performance smokes and representative native, Android, and XR
  gates show no shared-host regression; and
- the topic, architecture/platform, and web operating docs describe the landed
  boundary and remaining browser-specific exceptions.

## Current Baseline

Verified on 2026-07-10:

- `native/apps/mclone-web-client/src/web_canvas.rs` is 6,431 lines.
  `WebChunkRenderSession` combines WebGPU resources with runtime/session state,
  camera/input construction, settings effects, session dispatch, catalog work,
  render synchronization/uploads, actors, UI, and full-frame ordering.
- `native/apps/mclone-web-client/www/mclone-web-app.ts` has the correct
  browser-owned rAF/input/resource rim, but still relays app-local policy and
  compiler wake work.
- `WebIntegratedServerRunner` already implements `IntegratedServerRunner`.
- `WebRenderSectionCompiler` already implements `RenderSectionCompiler` over
  the resident `SharedArrayBuffer` compiler ring and delta protocol.
- local and remote web sessions already expose normal-frame updates through
  `ClientConnection`.
- web server, worldgen, light, and render compiler workers already have bounded
  shared pools, fallback paths, ABI checks, and transport metrics from
  Tacticals 062 and 067.
- shared client-experience/session/catalog reducers exist, but web consumes
  some of their outputs through its own matches.

The direct portability gate currently reports 95 errors:

```bash
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
```

These are mostly cascades from six real boundary families:

1. the scene runtime lives in the WASM-excluded `native_session_runtime`;
2. neutral mesh assets/defaults live in native-gated `render_assets`;
3. the host concretely owns `NativeWorldCatalog` and its synchronous executor;
4. camera reconcile is native-gated and coupled to native runtime methods;
5. the host concretely owns `NativeTeleportPreviewWorker`; and
6. reachable scene timing and deadlines use `std::time::Instant` directly.

Families 1-5 produce the compile errors; family 6 compiles on
`wasm32-unknown-unknown` and panics at runtime instead, so a green direct
WASM gate proves compile portability only — browser runnability is
established by the Slice 4 proof, never by `cargo check`. The implementation
should eliminate these root causes, not patch the ensuing type-inference
errors one at a time.

## Scope And Non-Goals

In scope:

- make scene/runtime policy and its data dependencies target-neutral;
- introduce the minimum clock, service, and asynchronous operation seams needed
  by native and browser hosts;
- adapt the existing browser connection, catalog, compiler, and worker owners;
- prove the shared host directly in a browser before production replacement;
- switch all production browser host modes and delete duplicated policy;
- add source-shape and behavior gates that prevent a second browser
  orchestrator from regrowing.

Not in scope:

- a renderer rewrite, a new WebGPU abstraction, or a new worker topology;
- a generic promise/future framework inside `mclone-scene`;
- moving `requestAnimationFrame`, DOM events, canvas/surface ownership,
  JavaScript object ownership, or presentation into a shared crate;
- reviving the retired TypeScript engine;
- replacing shared-memory compiler/runner traffic with inline synchronous WASM;
- automatically enabling far LOD, travel assist, frame-pipeline overlay,
  debug diagnostics, server cadence, teleport preview, or audio merely because
  the shared host can compile for web;
- arbitrary asset-pack browser redesign; Tactical 169 owns asset selection and
  transactional active-set replacement; or
- broad protocol/server-push work already owned by the networking tacticals.

## Locked Architecture

### One synchronous policy host

`McloneSceneHost` stays synchronous and frame-stepped. A host call may drain
already-available completions and emit new operations; it may not await a JS
promise, block on a worker, or own the browser event loop.

### Small services, not public generic proliferation

Split the current native-only runtime into a platform-neutral scene-session
shell plus narrow services. Do not make every downstream caller name a growing
`McloneSceneHost<S, R, C, D, A, ...>` type. Trait objects, private generics, or
small enums are acceptable internal representations after measurement; the
public host remains a coherent owner.

Native constructors may assemble native implementations. The web constructor
may assemble browser implementations. Session, camera, gameplay, render, UI,
and lifecycle policy compile once.

### Typed asynchronous operations

Browser-only asynchrony crosses a request/completion boundary:

```text
McloneSceneHost::step(...)
  -> PlatformOperation { session_epoch, request_id, kind }
  -> WebOperationExecutor starts promise/worker action
  -> PlatformOperationCompletion { session_epoch, request_id, result }
  -> next host step drains and validates completion
```

Required invariants:

- request IDs are unique within a host lifetime;
- session/resource epochs reject completions from superseded work;
- duplicate and unknown completions are harmless and observable;
- teardown invalidates outstanding operations without retaining JS objects in
  the host;
- failures restore an explicit retryable or terminal shared state;
- no Rust borrow is held across an `await` or promise callback; and
- native executors can complete the same operation immediately without a
  distinct policy path.

Prefer domain operations over a generic boxed callback API. Candidate kinds
are session start/replacement, remote connection/reconnection, world-catalog
work, prepared asset-set completion, and graceful shutdown. Existing genuinely
synchronous contracts should stay synchronous.

### Browser mechanisms remain adapters

The final `WebFrameDriver` may own:

- `requestAnimationFrame`, focus/visibility, canvas resize, and frame time;
- DOM keyboard, pointer, touch, pointer-lock, and gamepad collection;
- WebGPU instance/device/surface/canvas configuration and presentation targets;
- concrete Web Workers, `MessagePort`, `SharedArrayBuffer`, `JsValue`, promises,
  IndexedDB handles, WebSocket construction, and fetch objects;
- completion/wake queues whose neutral outputs are drained into services; and
- browser recovery UI around device/surface loss.

It may not own settings policy, session replacement policy, `EngineCameraInput`
semantics, client interaction policy, render admission, section dirty/revision
acceptance, terrain upload policy, frame layer ordering, UI/HUD assembly, or
frame accounting.

### Preserve worker and transport investments

The existing implementations are inputs to the adoption, not temporary code to
replace:

- `WebIntegratedServerRunner` and the local server worker;
- worldgen and light workers;
- `WebRenderSectionCompiler`, its resident compiler worker, shared-memory ring,
  snapshot deltas, and bounded pools;
- normal-frame `ClientConnection` ingress for local and remote sessions; and
- current queue, fallback, pool, and transport instrumentation.

The web compiler adapter must own its wake mechanism internally, directly or
through a browser-supplied wake sink. No `Worker`, `JsValue`, or
`SharedArrayBuffer` type crosses into `mclone-scene`.

### Preserve feature truth

The first production cutover consumes the current web
`ClientExperienceProfile` unchanged. A feature moves from `Unsupported` only in
a separately evidenced change that includes its input/UI/backend/diagnostic
path. Structural host reuse is not feature support evidence.

## Service Extraction Map

Names below are directional; implementing agents should fit existing module
vocabulary rather than creating parallel abstractions.

| Boundary | Shared owner/contract | Native assembly | Web assembly |
|---|---|---|---|
| monotonic time | compact scene instant/deadline plus clock source | `std::time::Instant` adapter | rAF/`performance.now()` projection |
| scene runtime | neutral session shell and update/command policy | existing native runner/connection owners | local worker or remote WebSocket connection |
| render compile | existing `RenderSectionCompiler` | native worker compiler | existing resident-SAB compiler |
| compiler wake | private backend wake sink | channel/thread wake | browser worker post/wake mechanism |
| world catalog | typed operation/result surface around shared catalog policy | filesystem/SQLite executor | IndexedDB promise executor |
| deferred drops | bounded submit/drain/backlog service | existing native drop worker | measured frame-budgeted queue or worker backend |
| teleport preview | capability-shaped submit/poll service | native preview worker | explicitly unavailable initially or later worker |
| active assets | neutral prepared render/actor/effect data and epoch | native loaders | fetch/parse plus Tactical 169 web active-set adapter |
| audio | optional already-attached engine service | current native device engine | absent until browser gesture/playback gate proves it |

The clock is a correctness boundary, not merely a compile shim. Deadline and
elapsed comparisons must have one monotonic meaning on native and web; wall
clock and JavaScript `Date` are not substitutes.

## Implementation Strategy

Land and validate portable prerequisites while the old production web host is
still active. Then prove a real `McloneSceneHost` browser frame in an isolated
smoke entry point. Only after that proof is green, replace all production modes
in one bounded cutover and delete the old orchestration.

This ordering allows small reviewable commits without maintaining two
production policy hosts. Each slice should normally land as its own buildable
commit and use:

```text
Topic: web-scene-host-adoption
```

The production cutover may use several local commits while being developed,
but the landed slice must not leave one host mode or a runtime flag on the old
orchestrator.

## Slice 0: Baseline, Tripwires, And Contract Locks

Purpose: make the current behavior and forbidden end state executable before
moving ownership.

Work:

1. Record a code-owner inventory for every policy block currently in
   `WebChunkRenderSession` and `mclone-web-app.ts`, mapping it to
   `McloneSceneHost`, a neutral service, or the final `WebFrameDriver`.
2. Add or tighten browser smoke assertions for all production host modes:
   local worker, IndexedDB-backed local world, and remote WebSocket.
3. Record current movement/perf, frame-gap, compiler transport, server runner,
   worldgen/light worker, bounded-pool, queue-depth, and fallback observations.
   Reuse existing reports and add fields only where ownership cannot otherwise
   be verified.
4. Add focused shared tests for asynchronous request identity, epoch/stale
   rejection, failure restoration, and teardown semantics before browser
   promises consume them.
5. Extend source-shape tooling in warning/baseline form to identify the app
   policy patterns that must disappear at cutover. Do not reject the known old
   host before its deletion slice.
6. Capture and visually inspect the current web world/UI in `/tmp`; record the
   smoke command and salient evidence in this tactical rather than committing
   images.

Exit criteria:

- every large web owner has an explicit destination;
- all three production host modes have an executable pre-cutover smoke;
- async state-machine edge cases are test-locked independently of JS promises;
- performance thresholds use current evidence rather than invented values; and
- no production behavior or user-facing feature profile changes.

## Slice 1: Portable Scene Prerequisites

Purpose: remove neutral data/utility code from native cfg islands before
changing the runtime owner.

Work:

1. Introduce the shared monotonic clock/instant/deadline contract and migrate
   scene startup, frame admission, locomotion attribution, and render timing
   away from bare reachable `std::time::Instant` assumptions.
2. Move `TexturedMeshAssets`, compile defaults, and other target-neutral
   render-asset data out of native-gated modules. Keep filesystem/device loading
   in platform adapters.
3. Make camera reconciliation operate on the neutral runtime/player facts it
   actually needs. Do not create a web-specific reconciliation copy.
4. Replace concrete `NativeTeleportPreviewWorker` ownership with an optional
   capability/service. Preserve native behavior and make unavailable behavior
   explicit for web.
5. Make audio attachment optional at the host boundary; do not initialize an
   output device from shared scene code.
6. Add native and target-neutral unit coverage for time ordering, deadline
   expiry, camera reconciliation, absent teleport, and absent audio.

Exit criteria:

- neutral scene types have no filesystem, winit, OpenXR, CPAL device-creation,
  OS-thread, or browser dependency;
- native desktop/offscreen/XR behavior remains unchanged;
- the remaining direct WASM errors are runtime/catalog/service ownership rather
  than neutral data and timing; and
- public names are platform-neutral rather than aliases with `Native` in the
  semantic contract.

## Slice 2: Neutral Runtime Shell And Service Container

Purpose: make `McloneSceneHost` itself own target-neutral runtime/session state
while preserving native consumers first.

Work:

1. Split `native_session_runtime` into a target-neutral scene-session shell and
   native constructor/service assembly. Move session state, command/update pump,
   readiness, replacement, camera access, diagnostics, and lifecycle policy to
   the shell.
2. Remove concrete `NativeWorldCatalog` and synchronous catalog executor
   ownership from the host. Route catalog intent/results through the shared
   typed platform-operation surface while keeping catalog UI policy shared.
3. Define a compact service container/constructor boundary for connection,
   render compiler, clock, deferred drops, teleport, audio, and platform
   operations without exposing a large public generic stack.
4. Adapt all existing native host constructors to assemble the new services.
   Desktop, offscreen, Android, desktop XR, and Quest remain consumers of the
   same host API. Delete the superseded `native_session_runtime` policy entry
   points in the same slice; no compatibility wrapper may keep the old call
   graph alive beside the shell.
5. Decide deferred-drop representation using Slice 0 measurements: reuse a
   target-neutral bounded queue contract, with native thread and web drain/worker
   implementations outside the host.
6. Add ownership and teardown tests: service replacement cannot leak an old
   compiler, connection, catalog operation, or drop queue into a new epoch.
7. Land the split move-first: mechanical relocation commits with function
   bodies unchanged (reviewable with `git diff --color-moved`), followed by
   separate small seam-introduction commits. Move commits intend zero native
   behavior change.
8. Add executor-equivalence tests: drive one scripted session scenario
   through an immediate-completion executor and a deferred-completion
   executor and assert identical host state and policy decisions, pinning the
   invariant that native has no distinct policy path for typed operations.
9. Extend the scene-host/thin-adapter purity gates to the new native
   service-assembly modules: assembly code may construct services but may not
   match on session, settings, catalog, or camera policy. This is the native
   mirror of the Slice 5 web tripwires.

Exit criteria:

- `McloneSceneHost` has no concrete native catalog, native teleport worker, or
  native session-runtime field;
- every native driver remains thin and passes the existing purity gates;
- service construction is platform-specific while policy is not;
- deferred-drop behavior and backlog accounting have a selected web-capable
  contract;
- move commits show relocation rather than rewrite, and the superseded native
  runtime entry points are deleted rather than wrapped;
- immediate and deferred operation executors are proven equivalent by shared
  tests;
- native service-assembly modules pass the extended purity gates;
- representative desktop, offscreen pixel-canary (compared against a
  pre-slice capture), flat Android, desktop XR, and Quest evidence is
  recorded in this tactical before Slice 3 begins; and
- any remaining WASM failures name missing browser implementations, not
  native-only scene policy.

## Slice 3: Browser Service Adapters And Direct WASM Gate

Purpose: assemble browser implementations behind the neutral host contracts
without yet replacing the production frame loop.

Work:

1. Project rAF/`performance.now()` through the shared monotonic clock contract,
   including resume behavior that cannot produce negative or wall-clock jumps.
2. Adapt `WebIntegratedServerRunner` and local `ClientConnection` ownership to
   the neutral runtime service without changing the worker protocol.
3. Adapt `WebSocketServerSession` to nonblocking shared reconnect/start/shutdown
   operations. Define supersession and stale completion behavior explicitly.
4. Adapt IndexedDB world-catalog requests to typed operations and completions;
   remove policy decisions from promise callbacks.
5. Make `WebRenderSectionCompiler` a directly injectable compiler service whose
   wake sink is private to the browser adapter. Preserve budget-one admission,
   dirty/revision acceptance, resident deltas, shared rings, metrics, and
   fallback probes.
6. Implement the selected browser deferred-drop backend and its bounded backlog
   reporting.
7. Provide explicit absent implementations for unsupported teleport/audio
   capabilities. Do not silently fake successful activation.
8. Supply browser asset data through the neutral prepared-set seam. Coordinate
   with Tactical 169's epoch contract and do not create a second asset owner.

Exit criteria:

```bash
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
```

Both pass; compiler/server/catalog adapters have focused WASM/browser tests;
out-of-order and post-teardown completions are rejected; remote reconnect has
an explicit state path; compiler wake requires no JavaScript object in shared
code; and the old production `WebChunkRenderSession` still behaves unchanged.

## Slice 4: Direct Browser Scene-Host Proof

Purpose: render and interact with a real shared host in a browser before the
irreversible production cutover.

Work:

1. Add a smoke-only/direct-proof entry point that constructs
   `McloneSceneHost` with the browser services and a single-view WebGPU target.
   It must not become a second production mode or public long-lived toggle.
2. Feed it the shared `FlatInputFrame`/camera/view contracts from real DOM input
   translation and fixed scripted smoke inputs.
3. Exercise local worker startup through playable/idle readiness, normal-frame
   update draining, interaction/block edit, UI/HUD, settings effects, terrain
   compile/upload/draw, actors/effects, resize, and controlled shutdown.
4. Verify the host, not the app, calls render-section synchronization/admission
   and assembles sky, terrain, actors/effects, and UI in the shared order.
5. Lock hidden-tab behavior: whether scene stepping pauses, what continues in
   the server/compiler workers, and how resume drains queued work within a
   bounded budget.
6. Lock WebGPU device/surface-loss behavior at least to a controlled retry,
   restart-required state, or user-visible terminal error; it must not wedge
   the rAF busy guard silently.
7. Resolve browser audio scope for cutover. Keep the capability absent unless a
   real user-gesture activation and audible playback probe exists.
8. Compare visual captures, frame gaps, queue depths, compiler/runner shared
   transport, fallback counts, and movement/perf results to Slice 0.

Exit criteria:

- a real local browser world is visually inspected from the shared host;
- input, interaction, UI, chunk streaming, actors/effects, and shutdown are
  smoke-proven;
- worker/shared-memory/fallback metrics show the existing topology is active;
- no material frame-gap or movement/perf regression is unexplained;
- hidden-tab and device/surface-loss policies are recorded in the topic; and
- the proof path is ready to be folded into production and then removed.

## Slice 5: Atomic Production Cutover And Deletion

Purpose: replace the production web orchestrator once, for every supported host
mode, and remove the duplicated policy in the same landed slice.

Precondition: Slice 4 is green. Tactical 169's active asset-set/epoch seam is
either landed and consumed or stable enough to integrate directly; this slice
must not introduce a competing resource epoch.

Work:

1. Replace `WebChunkRenderSession` with the shared host plus a thin
   `WebFrameDriver` in the production rAF path.
2. Route local worker, IndexedDB local-world, and remote WebSocket startup,
   normal frames, reconnect/replacement, and shutdown through the same host.
3. Keep rAF, canvas/surface acquisition, DOM input collection, promise startup,
   WebGPU recovery, and presentation in browser code.
4. Delete app-local settings-effect matches, session-start/dispatch matches,
   camera-input semantic assembly, render-section sync/admission, terrain upload
   decisions, frame layer ordering, UI/HUD assembly, and duplicate accounting.
5. Fold the Slice 4 proof onto the production path and delete its special entry
   point or flag.
6. Preserve the existing web feature profile and public browser API unless an
   API is only an obsolete orchestrator control. Update glue/types deliberately.
7. Run all web host modes and visually inspect desktop and mobile browser
   layouts after the final deletion, not only before it.

Required end-state tripwires should reject web app code that regrows:

- a `ClientExperienceSettingEffect` policy match;
- a `SessionStartRequest` dispatch policy match;
- app-local `EngineCameraInput` semantic construction;
- direct app calls to `sync_render_sections_with_budget` or upload-admission
  policy;
- app-local sky-to-terrain-to-actors-to-UI frame orchestration; or
- another session/runtime struct that owns both platform resources and shared
  scene policy.

Exit criteria:

- every production host mode owns a `McloneSceneHost`;
- no runtime flag or mode retains the old orchestrator;
- the direct proof shim and `WebChunkRenderSession` are gone;
- web source-shape gates pass in enforcement mode;
- all browser smoke and performance gates pass with reviewed metrics; and
- native thin-adapter and scene-host purity gates remain green.

## Slice 6: Parity Audit, Enforcement, And Closeout

Purpose: turn the adoption into a durable boundary and distinguish structural
completion from remaining feature work.

Work:

1. Extend the thin-adapter/purity tooling to cover the browser driver and keep
   it in the default validation path.
2. Audit every web `ClientExperienceProfile` exception against the landed host.
   Remove only exceptions whose full browser implementation is genuinely
   present; retain all others with current reasons and named follow-ups.
3. Audit public names and cfgs for obsolete native/web aliases and unreachable
   compatibility paths; delete them rather than documenting dead seams.
4. Update `docs/native-engine-architecture.md`, `docs/platforms.md`,
   `docs/native-web.md`, Tactical 168, and the topic with final ownership,
   validation evidence, and remaining gaps.
5. Run the full browser matrix plus representative native desktop, offscreen,
   Android, XR, and Quest validation required by the shared contract changes.
6. Mark this tactical complete only after the topic can answer current owner,
   feature truth, performance evidence, and next web work without consulting
   the old implementation.

Exit criteria:

- one enforced policy-host architecture covers browser and native clients;
- browser-only code is cadence/resource/I/O glue with documented exceptions;
- retained feature gaps are explicit follow-ups, not structural ambiguity;
- operating and architecture docs agree with code; and
- the topic status records the cutover commit/evidence and recommended next
  slice outside this tactical.

## Validation Matrix

Run focused tests while editing, then these shared gates after each service or
host-contract slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml \
  -p mclone-app-runtime --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-scene --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:scene-host:purity
pnpm native:thin-adapters:purity
pnpm native:desktop-offscreen:smoke
```

The direct `mclone-scene` WASM gate is expected to fail only before Slice 3;
the tactical records the root error count after each prerequisite slice.

Browser build/static gates:

```bash
pnpm native:web:build
pnpm native:web:typecheck
```

Browser behavior gates after browser adapter changes, and all of them for
Slices 4-6:

```bash
pnpm native:web:smoke
pnpm native:web:thread-smoke
pnpm native:web:canvas-smoke
pnpm native:web:chunk-smoke
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
pnpm native:web:mobile-smoke
pnpm native:web:block-edit-probe
pnpm native:web:movement-perf
pnpm native:web:remote-smoke
```

At the first drawable milestone and after the production cutover, capture
browser screenshots under `/tmp` and inspect the world, HUD/menu, actor/effect,
and mobile layouts. Do not commit captures.

After a neutral runtime shell or public host/service contract changes, use the
current platform matrix in [`../platforms.md`](../platforms.md#validation-policy)
for representative desktop, flat Android, desktop XR, and Quest gates. Build
Android lanes through the documented scripts, never with hand-rolled cargo/NDK
commands.

## Performance And Correctness Invariants

Adoption is a refactor only if all of these remain true:

- the render compiler remains resident and delta-fed; no whole-view compile
  transaction or per-job Wasm module startup is introduced;
- server, worldgen, light, and render compiler worker transports retain shared
  memory where currently supported and report explicit fallback otherwise;
- bounded pools and queues remain bounded, with backlog/drop/fallback facts
  exposed rather than hidden behind promises;
- the browser frame thread never blocks on worker, catalog, fetch, or WebSocket
  work;
- normal-frame updates still enter through `ClientConnection` and are drained
  under the shared host's budgets;
- dirty/revision acceptance prevents stale compile results and resource epochs
  prevent stale asset/completion adoption;
- each rAF callback performs at most one scene step/presentation attempt and
  cannot overlap another callback through a dangling promise;
- hidden-tab resume cannot perform an unbounded one-frame catch-up;
- local and remote shutdown/reconnect cannot resurrect a superseded session;
- the native offscreen pixel canary remains visually stable; and
- no multiview/XR path regresses when shared render/session contracts move.

## Decision Register And Deadlines

The topic owns current truth; record each resolution there as it is made.

| Decision | Must be settled by | Acceptance basis |
|---|---|---|
| compiler wake sink representation | Slice 3 | no JS types in scene; existing compiler metrics and prompt wake preserved |
| deferred-drop browser backend | Slice 2 | measured cost/backlog; bounded and observable |
| remote reconnect state machine | Slice 3 | nonblocking, epoch-safe, retry/terminal states tested |
| hidden-tab pause/resume policy | Slice 4 | bounded resume work and no session ambiguity |
| WebGPU device/surface-loss response | Slice 4 | controlled recovery or explicit user-visible terminal state |
| browser audio in first cutover | Slice 4 | real gesture activation and audible probe, otherwise absent capability |
| Tactical 169 asset epoch integration | before Slice 5 | one prepared-set/resource epoch owner |
| web feature-exception promotions | Slice 6 | full behavior/UI/diagnostic smoke per feature |

Unknowns are not permission to add platform policy to the app. If a boundary
cannot satisfy the locked architecture, stop that slice, update the topic with
the evidence and options, and revise this tactical before production cutover.

## Cross-Tactical Coordination

- [`168-unified-native-scene-host.md`](168-unified-native-scene-host.md) owns the
  shared native host shape and thin-adapter guardrails this work extends.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) and
  [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  own the browser worker/shared-memory architectures that must survive.
- [`085-web-host-mode-convergence.md`](085-web-host-mode-convergence.md) owns the
  existing local/remote browser host-mode seams.
- [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md)
  and [`154-client-ingress-adapter-cleanup.md`](154-client-ingress-adapter-cleanup.md)
  own normal-frame `ClientConnection` ingress.
- [`165-native-feature-parity-baseline.md`](165-native-feature-parity-baseline.md)
  owns the feature-profile/exception framework; this tactical preserves web
  reasons before any later promotion.
- [`169-runtime-asset-pack-selection.md`](169-runtime-asset-pack-selection.md)
  owns active asset-set preparation/replacement. Slices 0-4 here may proceed
  while its implementation advances, but Slice 5 consumes its epoch seam and
  must not race it with another web resource lifecycle.

## Recommended Start

Begin with Slice 0. The first implementation commit should add executable
baseline/tripwire coverage and the explicit old-owner-to-new-owner inventory;
it should not yet move production web behavior. Slices 1-3 can then remove the
WASM blockers in reviewable shared-first commits while Tactical 169 stabilizes
the asset epoch needed by the eventual atomic cutover.
