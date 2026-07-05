# 141: Flat Client Platform Policy Convergence

Status: proposed architecture and guardrail workstream. Opened on 2026-07-04
after tactical
[`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md) exposed
another desktop-first policy split: desktop flat can create, open, delete, and
smoke persistent catalog worlds through `FlatClientDriver`, while native web
has an IndexedDB catalog store smoke but still renders a default catalog state
and leaves the shared catalog UI actions inert. Slice 1 landed on 2026-07-04:
`mclone-app-runtime` now has a platform-neutral flat-client catalog controller
with shared UI-state conversion, request effects, async-style completion
handling, active-delete guarding, and focused conformance tests. Slice 2 landed
on 2026-07-04: desktop `FlatClientDriver` now delegates catalog UI state and
create/open/delete action policy to the shared controller while retaining the
native catalog backend, world-root selection, `SceneOptions`, and startup
payloads as desktop adapter responsibilities. Slice 3 landed on 2026-07-05:
native web now renders controller-owned catalog state and routes menu
Create/Open/Delete through controller request/completion effects while keeping
IndexedDB promises and worker startup in the browser adapter. Slice 3 follow-up
landed on 2026-07-05: the web adapter now refreshes committed catalog render
state after catalog actions/responses, and `pnpm native:web:catalog-smoke`
covers menu-driven create/open/delete plus IndexedDB delete cleanup.

Workstream: documentation cleanup plus native Rust shared architecture. The
target remains shared implementation, desktop validation first. App crates own
platform adapters; shared crates own client, UI, session, catalog, and runtime
policy.

## Purpose

Stop flat-client features from repeatedly landing as a mature desktop path plus
parallel web, Android, and XR catch-up wiring.

The current question is concrete: native web should not need a private rewrite
of desktop world-create/open/delete policy just because its persistent backend
is IndexedDB and async. IndexedDB, browser workers, `wasm-bindgen`, and DOM
events are platform adapter concerns. Catalog semantics, UI state, status text,
delete-active rejection, create/open request shape, and teardown-before-start
policy are shared flat-client concerns.

This tactical defines the missing layer between `mclone-ui` actions and
platform-specific runtime/storage execution: a shared flat-client controller
with small platform adapters and completion/effect hooks.

## Problem Background

We have solved versions of this problem several times, but the pressure keeps
coming back at the app-host layer.

The current live split is representative:

- `mclone-native-client::flat_client_driver::FlatClientDriver` owns the
  desktop catalog cache, `LocalWorldSummary -> WorldCatalogUiState` conversion,
  stable UI row ids, active-row marking, create/open/delete action policy,
  catalog status strings, session start queueing, and native world-root
  selection.
- `mclone-web-client::web_canvas::WebChunkRenderSession` owns its own
  `GameUiHost`, `apply_web_ui_action`, session coordinator, runtime host, and
  browser status plumbing. The catalog actions are listed in the match arms,
  but they do nothing, and `ui_render_state` still sets
  `world_catalog: Default::default()`.
- Tactical 136 Slice 6 added a browser IndexedDB catalog store and smoke helper,
  but the menu-driven web path still needs another wiring layer to feed
  summaries into Rust UI state and route catalog actions.

The web shape is different in important ways: IndexedDB is asynchronous,
browser workers are message-driven, and JavaScript owns some lifecycle glue.
Those differences do not justify copying UI/session/catalog policy. They call
for a completion-based adapter, the same broad solution shape used by earlier
convergence work.

The deeper cause is that `FlatClientDriver` started as a native staging owner in
[`105-offscreen-flat-client-host.md`](105-offscreen-flat-client-host.md). That
was appropriate while it still depended on native render resources,
`WindowSceneRuntime`, `SceneOptions`, mouse lock, frame pacing, and desktop
startup factories. It should not become the permanent home for policy that every
flat client and XR menu panel needs.

## Past Attempts And Lessons

The relevant history is useful because there is a clear success pattern.

- [`003-native-ts-parity-roadmap.md`](003-native-ts-parity-roadmap.md) set the
  early expectation that native-first does not mean desktop-only. It is
  historical now, but the pressure it named still exists.
- [`061-shared-engine-web-adapter-refactor.md`](061-shared-engine-web-adapter-refactor.md)
  identified the same failure mode for runtime/render-section policy:
  desktop app modules and web canvas code had started growing parallel cache,
  scheduling, and render-prep rules. Its boundary rule still applies here:
  platform code owns surfaces, event loops, fetch/storage, and packaging; shared
  code owns engine policy.
- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  is the best success model. Desktop and web converged on one logical
  `RenderSectionCompiler` and streaming loop without forcing desktop through the
  web `SharedArrayBuffer` ABI. The symmetry was in the trait and policy, not in
  identical transport bytes.
- [`084-single-view-platform-alignment.md`](084-single-view-platform-alignment.md)
  moved native single-view runtime/render contracts toward shared adapters and
  put the durable platform matrix in
  [`../topics/platform-parity.md`](../topics/platform-parity.md). It also called
  out that web/WASM still carries a separate adapter shape.
- [`085-web-host-mode-convergence.md`](085-web-host-mode-convergence.md) proved
  the right async lesson: web can await promises and callbacks while native
  remains blocking where appropriate, as long as both consume the same exchange
  accounting and reconnect/resync policy.
- [`095-shared-session-coordinator.md`](095-shared-session-coordinator.md)
  moved create/open/join request state, active descriptors, status, and
  failure transitions into `mclone-app-runtime`. That was necessary but not
  sufficient: app hosts still apply many UI actions locally, and runtime
  factories still carry platform payloads.
- [`098-flat-input-capability-convergence.md`](098-flat-input-capability-convergence.md)
  reframed input differences as capabilities, not platform identities. The same
  principle applies to catalog and session behavior: persistent storage may be a
  filesystem, IndexedDB, or Android app-private root, but the user-facing
  operation is still create/open/delete local world.
- [`101-create-world-chunk-progress-screen.md`](101-create-world-chunk-progress-screen.md)
  moved startup progress into shared server/app-runtime/UI contracts while
  letting each platform host the pump or async task in its own lifecycle.
- [`105-offscreen-flat-client-host.md`](105-offscreen-flat-client-host.md)
  intentionally moved flat-client lifetime into a native staging driver so
  desktop and offscreen could share it. That helped, but the doc also records
  why the driver is not directly reusable by web: it is still native-client
  staging, not a platform-neutral controller.
- [`123-ui-v2-menu-rebuild.md`](123-ui-v2-menu-rebuild.md) gave us retained
  shared screens, layout, hit testing, and `GameUiAction` outputs. It did not
  make action application shared.
- [`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md)
  made storage host-owned through shared request/completion contracts. Browser
  IndexedDB async behavior is already accepted as an adapter behind shared
  persistence semantics.
- [`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md) added
  shared catalog identity, session vocabulary, native backend, UI v2 screens,
  desktop lifecycle wiring, and the first browser IndexedDB catalog store. It
  now exposes the exact missing layer: shared UI and shared backend contracts
  exist, but action policy is still split between app hosts.

The lesson is consistent: converge on shared logical policy and narrow adapters.
Do not force every platform through the same transport shape. Also do not let
transport differences become a reason to duplicate user-facing rules.

## Target Shape

Introduce a shared flat-client controller, initially in `mclone-app-runtime`
unless dependency pressure proves it deserves a dedicated crate such as
`mclone-flat-client`.

```text
platform events / storage completions / runtime completions
  -> platform adapter
  -> shared FlatClientController
  -> mclone-ui GameUiHost / GameUiRenderState
  -> shared session/catalog/input/runtime contracts
  -> platform effects
```

The shared controller should own:

- `GameUiAction` policy that is not truly platform-specific;
- `GameUiHost` screen transitions or a single shared wrapper around them;
- catalog UI state, including summary-to-row conversion, stable UI ids,
  selected row, active row, status messages, create draft facts, and capability
  display;
- create/open/delete validation flow above platform storage;
- session coordinator transitions, including local create/open, remote join,
  failure status, and teardown-before-start policy;
- loading/startup status projection from shared app-runtime diagnostics;
- common flat UI render-state composition;
- tests that run without `winit`, DOM, OpenXR, or Android activity types.

Platform adapters should own:

- desktop `winit`, windows, mouse lock, frame pacing, native surface acquire and
  present;
- native desktop world roots, direct `--world-dir` developer bypasses, and the
  native `NativeWorldCatalog` / `SqliteWorldStore` construction details;
- browser canvas, pointer lock, DOM event translation, `wasm-bindgen` exports,
  worker URLs, WebSocket construction, IndexedDB promises, and JavaScript smoke
  harnesses;
- Android `NativeActivity`, app-private roots, lifecycle hooks, touch/raw input
  event collection, and APK validation;
- OpenXR session, swapchain, controller action, panel projection, and XR
  presentation details;
- execution of controller effects that require platform resources.

The controller should not be an async trait object. Prefer an effect and
completion model:

```text
controller.apply_ui_action(action)
  -> FlatClientEffects {
       catalog_requests,
       session_start_requests,
       host_actions,
       status_dirty
     }

platform submits effects
platform later calls controller.apply_catalog_response(...)
platform later calls controller.apply_session_start_result(...)
```

Desktop can complete catalog requests immediately. Web can submit an IndexedDB
promise and feed the completion back on a later browser turn. Android can do
filesystem work through an app-private adapter. The shared action semantics do
not care which completion timing was used.

## Non-Goals

- Do not move `winit`, DOM, `web_sys`, OpenXR, Android activity types, or GPU
  surface ownership into shared crates.
- Do not force desktop through browser-style async promises or serialized ABIs.
- Do not force web to pretend IndexedDB is synchronous.
- Do not rewrite every flat host in one slice.
- Do not make `FlatClientDriver` directly compile for wasm if the cleaner move
  is extracting its shared policy into a smaller controller.
- Do not use the controller to hide real platform constraints. If a platform
  cannot support an action yet, it should surface a disabled capability or a
  visible unsupported status through shared state.

## Guardrails

### One Action, One Shared Owner

Every new `GameUiAction` must have exactly one owner:

- shared controller, for normal user-facing UI/session/catalog/input/runtime
  policy;
- platform adapter, only for true host actions such as quit process, mouse lock,
  frame pacing, browser pointer lock, surface rebuild, native capture, or XR
  session operations;
- documented unsupported capability, when a visible shared UI action is not
  available on a platform yet.

No app crate should add an inert match arm for a visible shared action without a
linked tactical note and a planned shared owner.

### No Parallel Status Strings

UI-facing status text for shared operations should be produced in shared code.
Platform adapters may include platform error details, but they should pass those
details through shared status/error types. If desktop says "Persistent worlds
unavailable" and web says something else for the same semantic failure, the
status policy has already forked.

### Shared State Conversion Only

Conversions such as `LocalWorldSummary -> WorldCatalogUiEntry`, UI row id
allocation, active-world row mapping, delete-enabled state, and capability
projection must live in one shared place. Platform adapters should feed facts,
not rebuild the UI model.

### Async Is A Completion Timing, Not A Policy Fork

If web needs a promise, represent the shared operation as pending and complete
it later. Do not duplicate the operation's validation, status, session request,
or UI transition rules in TypeScript or `web_canvas.rs`.

### Diff Footprint Tripwire

If a change touches both `mclone-native-client::flat_client_driver` and
`mclone-web-client::web_canvas` with matching `GameUiAction` logic, stop and
extract the policy first unless the change is purely platform plumbing.

Initial manual checks:

```bash
rg -n "apply_ui_action|apply_web_ui_action|GameUiAction::" native/apps/mclone-native-client/src native/apps/mclone-web-client/src native/apps/mclone-android-client/src native/crates/mclone-xr-scene/src
rg -n "OpenWorldList|OpenWorldCreate|CreateCatalogWorld|DeleteWorld|world_catalog: Default::default" native/apps native/crates
```

These can later become a small architecture lint or conformance test once the
controller is in place.

### Adapter Conformance Tests

The shared controller needs fake-adapter tests for at least:

- list, select, create, open, delete, and delete-confirm flow;
- duplicate display name / duplicate id errors;
- delete-active rejection;
- storage unavailable / read-only catalog;
- async pending catalog operation followed by success;
- async pending catalog operation followed by failure;
- create/open queuing a shared session request;
- existing active session requiring teardown before replacement;
- quit-to-title clearing active state before CRUD becomes enabled.

Desktop, web, Android, and XR adoption should be a thin smoke layer on top of
those shared conformance tests, not four separate sources of policy truth.

### Platform Matrix Updates

When a slice changes user-visible support, update
[`../topics/platform-parity.md`](../topics/platform-parity.md) and link this
tactical or the tactical that landed the adoption. The matrix is where we catch
"desktop done, web pending, Android unknown" before it becomes forgotten drift.

### No Permanent Desktop Staging Gravity

`FlatClientDriver` can remain a useful desktop/offscreen host wrapper, but when
it gains behavior that web, flat Android, or XR menu panels also need, the next
slice should either:

1. extract that behavior to the shared controller immediately, or
2. add an explicit temporary exception in this tactical with the planned
   extraction slice.

## Implementation Slices

### Slice 0: Tactical And Audit Baseline

Status: this document.

Deliverables:

- record the current desktop/web catalog action split;
- link prior convergence attempts and carry forward their successful boundary
  patterns;
- define the shared-controller target shape;
- add guardrails for future UI/action/session/catalog work;
- add the tactical index link.

### Slice 1: Shared Catalog Action Controller

Status: landed 2026-07-04.

Add the narrowest shared controller piece in `mclone-app-runtime`:

- catalog UI cache/state type owned outside app crates;
- `LocalWorldSummary -> WorldCatalogUiState` conversion;
- stable `WorldCatalogUiWorldId` allocation and active-row mapping;
- handling for `OpenWorldList`, `OpenWorldCreate`, `SelectWorld`,
  `ConfirmDeleteWorld`, `CancelDeleteWorld`;
- effect generation for `CreateCatalogWorld`, `OpenWorld`, and `DeleteWorld`;
- completion entrypoints for catalog list/create/open/delete results;
- tests with a fake catalog adapter covering success, failures, active-delete
  rejection, and pending async completion.

This slice should not move render resources, runtime factories, or browser
worker lifecycle. It is deliberately only the world-catalog action/state policy
that currently sits in desktop `FlatClientDriver`.

Recorded Slice 1 result:

- Added `mclone_app_runtime::flat_client_catalog` with
  `FlatClientCatalogController`, `FlatClientCatalogActionContext`,
  `FlatClientCatalogEffects`, catalog request effects, and session-start
  effects.
- Centralized `LocalWorldSummary -> WorldCatalogUiState` conversion, stable
  `WorldCatalogUiWorldId` allocation, active-world row mapping, create-display
  draft state, persistent/read-only/transient capabilities, loading state, and
  catalog status messages.
- Routed shared catalog actions through effect generation and completion
  methods instead of direct storage execution:
  `OpenWorldList`, `OpenWorldCreate`, `SelectWorld`, `ConfirmDeleteWorld`,
  `CancelDeleteWorld`, `CreateCatalogWorld`, `OpenWorld`, and `DeleteWorld`.
- Kept storage and runtime platform-local: callers execute emitted
  `WorldCatalogRequest` values and later feed `WorldCatalogResponse` or
  `WorldCatalogError` back to the controller. Desktop can complete those
  requests synchronously; web can complete them after IndexedDB promises.
- Added focused tests for world-list UI conversion and stable row IDs,
  create/open/delete effect flow, async-style completion, active-world delete
  rejection, storage errors, unsupported catalog state, and duplicate-id error
  surfacing.

Validation after Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime flat_client_catalog
cargo test --manifest-path native/Cargo.toml -p mclone-ui
git diff --check
```

### Slice 2: Desktop Delegates Catalog Policy

Status: landed 2026-07-04.

Refactor desktop `FlatClientDriver` so it becomes a native adapter around the
shared catalog controller:

- keep `NativeWorldCatalog`, world-root selection, `SceneOptions`, mouse lock,
  and startup factory payloads in `mclone-native-client`;
- remove desktop-local summary-to-UI conversion and active/delete status policy;
- execute controller effects by calling the native catalog backend and queuing
  existing session starts;
- keep the three desktop catalog smokes from tactical 136 green.

Recorded Slice 2 result:

- Replaced desktop-local catalog row cache and `WorldCatalogUiState` ownership
  with `FlatClientCatalogController`.
- Removed duplicate desktop summary-to-UI conversion, stable row-id allocation,
  active-row mapping, unsupported/persistent status messages, create/open/delete
  validation, and delete-active status policy.
- Added a desktop adapter layer that executes emitted `WorldCatalogRequest`
  values through `NativeWorldCatalog`, feeds `WorldCatalogResponse` /
  `WorldCatalogError` back into the controller, and queues existing
  catalog-backed local session starts from controller session-start effects.
- Kept native-only responsibilities in `mclone-native-client`: `world_root`,
  `NativeWorldCatalog`, `SceneOptions`, world-dir paths, mouse lock, and local
  startup payload construction.
- Updated desktop driver tests to observe catalog state through
  `GameUiRenderState` and backend summaries instead of private desktop cache
  fields.

Validation after Slice 2:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime flat_client_catalog
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
```

### Slice 3: Web Adopts The Same Controller

Status: landed 2026-07-05.

Wire native web through the shared controller rather than adding a second copy
of desktop policy:

- feed IndexedDB catalog summaries into the controller;
- return controller `WorldCatalogUiState` from web `ui_render_state`;
- route `OpenWorldList`, `OpenWorldCreate`, `SelectWorld`,
  `ConfirmDeleteWorld`, `CancelDeleteWorld`, `CreateCatalogWorld`,
  `OpenWorld`, and `DeleteWorld` through controller effects/completions;
- keep IndexedDB schema, promise plumbing, worker startup, and JavaScript smoke
  helpers web-local;
- remove inert catalog action arms from `apply_web_ui_action`.

Recorded Slice 3 result:

- Added `FlatClientCatalogController` ownership to
  `WebChunkRenderSession` and returned its active-world-aware
  `WorldCatalogUiState` from web `ui_render_state`.
- Replaced inert web catalog action arms with shared controller calls for
  world-list/create/select/open/delete/confirm/cancel actions. Rust now emits
  catalog request effects in the existing UI report object.
- Added wasm completion entrypoints for browser IndexedDB results:
  `applyWorldCatalogResponse` and `applyWorldCatalogError`.
- Added `startIndexedDbLocalWorld` so controller create/open completions start
  the integrated web worker with `worldStorage=indexeddb` and the resolved
  `worldId`, while using the shared active session descriptor.
- Kept browser-only concerns in TypeScript:
  `openWorldDb`, IndexedDB CRUD promises, active-world delete adapter argument,
  worker URLs, and app-smoke/runtime restart plumbing.
- Updated the platform parity tracker to mark web world-select/persistence as
  partial and to add the shared flat catalog controller as an explicit contract
  adoption row.
- Added a focused browser catalog UI smoke, `pnpm native:web:catalog-smoke`,
  that drives the shared Rust menu through Create/Open/Delete, verifies active
  `sessionWorldId` transitions, seeds deterministic IndexedDB chunk/entity
  records for the deleted world, and asserts delete removes both stores.
- Fixed the web adapter's committed UI render-state refresh after catalog
  actions/responses so pointer hit-testing sees the latest controller-owned
  catalog state without waiting for another render frame.
- Aligned the smoke's menu hit points with the Rust `GuiScale` and clamped
  `centered_panel` layout rules.

Validation after Slice 3:

```bash
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:catalog-smoke
pnpm native:web:app-smoke
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime flat_client_catalog
git diff --check
```

Results on 2026-07-05:

- Passed: `pnpm native:web:typecheck`
- Passed: `pnpm native:web:build`
- Passed: `pnpm native:web:smoke`
- Passed: `pnpm native:web:catalog-smoke`
- Passed: `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime flat_client_catalog`
- Passed: `git diff --check`
- Repeated validation issue: `pnpm native:web:app-smoke` failed twice in the
  existing block-place probe with `hitType: miss`, `commandSent: false`, and
  otherwise healthy app/session/compiler state. No catalog UI or IndexedDB
  request failed in that run; track this as an app-smoke interaction probe
  issue before using it as a blocker for catalog controller adoption.

### Slice 4: Session Replacement And Startup Policy

Move the remaining shared session action policy out of app-local match arms:

- local create/open and remote join action handling;
- failure-status UI restoration;
- teardown-before-start rule;
- loading/startup overlay projection;
- quit-to-title state transition policy, while hosts still perform actual
  runtime shutdown.

This should build on tactical
[`095-shared-session-coordinator.md`](095-shared-session-coordinator.md) rather
than replacing it.

### Slice 5: Broader Flat UI Policy Pass

Audit the remaining `GameUiAction` variants and classify them:

- shared controller;
- host action effect;
- unsupported capability with visible shared status;
- truly platform-specific action.

Prioritize actions that already appear in both desktop and web match arms:
options toggles, touch controls, movement/render settings, player model,
debug-hotbar assignment, and pause/title navigation.

### Slice 6: Android And XR Adoption

Adopt the controller in flat Android and the shared XR scene/menu panel:

- Android app-private world catalog adapter;
- Android XR app-private catalog adapter;
- desktop XR panel routing through the same shared action/state policy;
- headset/AVD smokes layered on top of shared conformance tests.

### Slice 7: Regression Gates

Make the guardrails harder to forget:

- add controller conformance tests for every shared UI action family;
- add a small test or script that fails when visible catalog actions are inert
  in app crates;
- update `docs/topics/platform-parity.md` rows for world-select/persistence and
  shared-contract adoption;
- add a review checklist to the next relevant tactical when a desktop-first
  slice intentionally defers platform adoption.

## First Good Implementation Chunk

The first code chunk should be Slice 1 only: extract catalog UI/action policy
from desktop into a shared controller with fake-adapter tests.

Do not start by wiring web menu actions directly to the new IndexedDB helpers.
That would make the feature work once, but it would repeat the failure pattern:
desktop policy in `FlatClientDriver`, web policy in `WebChunkRenderSession` or
TypeScript, and Android/XR waiting for a third copy.

The exit criterion for the first chunk is modest and useful: desktop can still
own native storage and runtime startup, web can still be unwired, but the shared
crate has the one place where catalog state, status, selection, active-world
guarding, and create/open/delete effects are defined and tested.

## Open Questions

- Does the controller stay in `mclone-app-runtime`, or should it become a new
  `mclone-flat-client` crate once session, input, UI, and catalog behavior all
  sit there?
- Should `GameUiHost` move wholly inside the controller, or should the first
  slice only centralize catalog sub-state and effects while app hosts still own
  their existing UI host instance?
- Should catalog effects use `WorldCatalogRequestId` from the existing catalog
  module, or a controller-local effect id that can cover future non-catalog
  async operations?
- How much of desktop `FlatClientDriver` remains after this workstream: a thin
  native/offscreen host around the controller, or a longer-lived native adapter
  for render-resource ownership?
