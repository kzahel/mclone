# 075: Shared Single-View Runtime Prereq

Status: completed first pass; prerequisite for `074-flat-android-build-smoke.md`.

## Purpose

Extract the platform-neutral single-view runtime shell before adding the flat
Android host. Desktop and native web already share the hard render/server seams,
but each app crate still owns its own client/runtime orchestration around those
seams. Android should consume a shared Rust runtime instead of becoming a third
copy.

This slice is a prerequisite to Android runtime integration, not Android
packaging. It must land independently and keep desktop plus native-web behavior
green.

## Current Shape

Shared seams already present:

- `mclone_render_session::RenderSectionCompiler`: desktop uses an OS-thread
  worker, native web uses a `SharedArrayBuffer` resident worker ring.
- `mclone_server::IntegratedServerRunner`: native and web-worker integrated
  server hosts both implement this trait.
- `mclone_render_session::EngineRenderSession`: already owns the client replica,
  render dirty state, cache updates, and budgeted render-section sync policy.

Duplicated app-shell work:

- chunk-view state: interest center, render distance, tracking radius
- host exchange application: command/update counters, dirty marking, client
  update application, diagnostics
- render-section streaming orchestration around `EngineRenderSession`
- common world queries: block lookup, water camera test, highest non-air column
- plain runtime stats used by debug overlays and smokes

## Target Crate

Add `native/crates/mclone-app-runtime`.

The crate owns only platform-neutral runtime state and reports:

- `SingleViewRuntime`
  - wraps `EngineRenderSession`
  - stores interest center, render distance, tracking radius
  - applies host exchanges and server updates
  - drives render-section streaming through caller-supplied compiler/snapshot
    hooks
  - exposes render/cache pending-work queries
  - exposes block/world/time queries
- plain reports:
  - `RuntimeExchange`
  - `RuntimeStepReport`
  - `RuntimeUpdateApplyReport`
  - `RuntimePollDiagnostics`
  - `SingleViewRuntimeStats`

The crate must not depend on `winit`, `web-sys`, `js-sys`, `wasm-bindgen`,
DOM/canvas types, Gradle/Android packaging, or filesystem asset probing.

## Adapter Ownership

Desktop keeps:

- `winit` event loop and input routing
- native surface creation and frame pacing
- filesystem/environment asset selection
- `RenderSectionCompileWorker`
- TCP remote reconnect policy
- full-frame rendering helper and headless capture glue

Native web keeps:

- canvas/WebGPU surface ownership
- wasm-bindgen exports and `JsValue` report serialization
- `WebIntegratedServerRunner` worker lifecycle
- `WebRenderSectionCompiler` `SharedArrayBuffer` arena and doorbells
- async websocket/worker command exchange
- TypeScript/browser glue

Android later owns:

- `NativeActivity` lifecycle and surface adapter
- Android asset staging/lookup
- touch/input adapter
- APK/ADB validation scripts

## Design Rules

- Keep host transport outside `SingleViewRuntime`. The shared runtime consumes
  plain `RuntimeExchange` values so desktop can block or reconnect, web can
  defer/budget, and future Android can use the native runner without adopting
  browser async semantics.
- Keep render compiler transport outside `SingleViewRuntime`. The runtime takes
  `&mut impl RenderSectionCompiler` plus a snapshot-submit hook so desktop can
  clone/move snapshots and web can stage delta-only worker mirror inputs.
- Return plain Rust structs from the shared crate. App crates translate those
  into logs, debug panes, or `JsValue`.
- Do not move frame composition in this slice. Desktop `render_full_frame` and
  web `render_chunk_report_with_cache_update` still own target acquisition,
  uploads, overlays, and platform reports.
- Do not move asset selection. The shared runtime may consume already-built
  client/render-session state, but platform-specific source lookup remains in
  the app crates.

## Implementation Slices

### Slice 1 - Crate And Plain Runtime

- [x] Add `mclone-app-runtime` to the workspace.
- [x] Move shared chunk-view helpers and runtime report structs into the crate.
- [x] Add `SingleViewRuntime` over `EngineRenderSession`.
- [x] Add exchange/update application APIs that preserve dirty marking and
  update counters.
- [x] Move common block/time/world query helpers.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
```

### Slice 2 - Desktop Adapter

- [x] Replace desktop-owned duplicated runtime fields with `SingleViewRuntime`.
- [x] Keep `NativeIntegratedServerRunner`, `RemoteServerSession`, assets,
  render compiler worker, and frame rendering in `mclone-native-client`.
- [x] Preserve desktop drain-to-idle helpers by looping one runtime poll step
  while checking runner diagnostics.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-native-client
pnpm native:movement:smoke
pnpm native:timedemo:smoke
```

Rendered-output gate:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-runtime-refactor-desktop.png \
  --width 1280 --height 720 --screenshot-debug-pane true
```

Inspect `/tmp/mclone-runtime-refactor-desktop.png`.

### Slice 3 - Native Web Adapter

- [x] Replace `WebRuntime`'s duplicated engine/exchange state with
  `SingleViewRuntime`.
- [x] Keep worker/websocket async exchange in the web crate.
- [x] Keep `WebRenderSectionCompiler` delta staging and doorbell logic in the
  web crate while driving the shared render-section sync method.
- [x] Keep all `JsValue`/canvas/report serialization in the web crate.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-web-client
pnpm native:web:build
pnpm native:web:smoke
```

### Slice 4 - Android Handoff

- [x] Update `074-flat-android-build-smoke.md` to treat this slice as complete.
- [ ] Start Android shell work with Android consuming `mclone-app-runtime`
  instead of copying desktop/web shell code.

## Review Rejection Criteria

- Any `winit`, `web_sys`, `js_sys`, `wasm_bindgen`, or Android/Gradle dependency
  in `mclone-app-runtime`.
- Any attempt to unify desktop and web `RenderSectionCompiler` implementations.
- Any whole-world web snapshot clone introduced by hiding the web delta submit
  hook.
- Any movement of DOM/canvas/`JsValue` serialization into the shared crate.
- Any opportunistic renderer surface refactor unrelated to the runtime shell.

## Completion Criteria

- Desktop and native web both use `SingleViewRuntime` for shared runtime state,
  update application, common queries, and render-section sync orchestration.
- App crates still own their platform transports, surfaces, input, assets, and
  report serialization.
- The validation gates above pass, including a manually inspected desktop
  screenshot saved under `/tmp`.
- `074-flat-android-build-smoke.md` no longer asks Android to extract runtime
  helpers during Android packaging; it consumes the shared crate.
