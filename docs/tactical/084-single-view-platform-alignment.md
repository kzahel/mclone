# 084: Single-View Platform Alignment

Status: active. Slice 1 is complete: flat Android now consumes
`mclone-app-runtime::local_single_view::LocalSingleViewSceneRuntime` for local
integrated-server setup, polling, idle wait, render-section sync, and
render-facing scene facts.

## Purpose

Keep desktop flat, flat Android, headless capture, and web/WASM aligned around
shared single-view runtime/render contracts so new features do not require
manual platform-by-platform implementation work.

The five validated client/platform lanes are now real. The next health problem
is avoiding platform drift while lighting, UI, interaction, chunk streaming,
and rendering continue to grow.

## Current State

- `mclone-app-runtime::SingleViewRuntime` owns shared client/runtime/render
  session state.
- `mclone-render-session` owns render-section dirty state, compile requests,
  cache updates, neighbor readiness, and camera-controller contracts.
- Desktop flat wraps these through `WindowSceneRuntime`, with extra desktop
  and remote-session behavior.
- Flat Android renders real terrain through shared crates, but still carries a
  private `AndroidSceneRuntime` that duplicates local integrated-server
  bring-up, chunk-view setup, polling, idle wait, render-section sync, and
  traversal-ready queries.
- Web/WASM already consumes `SingleViewRuntime`, but its worker and browser
  adapter shape stays separate.

## Target Shape

Single-view platform adapters should own only true platform concerns:

- desktop `winit` event loop, window/surface, keyboard/mouse, frame pacing,
  headless output paths, and remote-session CLI options
- flat Android `NativeActivity`, Android app data paths, Vulkan surface,
  resume/suspend/resize, touch translation, APK scripts, and AVD validation
- web canvas, browser workers, TypeScript glue, storage/fetch adapters, and
  browser validation

Shared crates should own:

- local integrated runtime setup and chunk-view dispatch
- render-distance and chunk-tracking-radius policy
- polling and idle wait helpers
- render-section compile/sync policy
- render-section cache and traversal-ready queries
- render-facing time/sky/block facts

## Implementation Slices

### Slice 1 - Flat Android Local Runtime Extraction

- [x] Add a reusable native-only local single-view scene helper in
  `mclone-app-runtime`.
- [x] Move flat Android's local integrated-server setup, polling, idle wait,
  render-section sync, and cached-section access onto that helper.
- [x] Keep Android lifecycle, surface, resize, touch orbit, asset-root setup,
  and logging in `mclone-android-client`.
- [x] Add focused unit coverage for the shared helper.
- [x] Validate desktop/app-runtime/web/Android compile gates.

Recorded Slice 1 result:

- Added `native/crates/mclone-app-runtime/src/local_single_view.rs` behind
  `cfg(not(target_arch = "wasm32"))`.
- Added `LocalSingleViewSceneOptions` and `LocalSingleViewSceneRuntime`.
- The helper owns local integrated-server construction, chunk-view dispatch,
  client update application, idle waiting, render-section sync, cached-section
  access, traversal-ready queries, mesh assets, and sky/time facts.
- Replaced flat Android's private `AndroidSceneRuntime` with
  `LocalSingleViewSceneRuntime`.
- Left Android-only behavior in `mclone-android-client`: activity lifecycle,
  logger, asset-root env setup, Vulkan surface/device/config, resize,
  redraw/present, touch orbit, and app validation markers/logging.
- Added `mclone-app-runtime` tests for Java-shaped tracking-radius policy and
  local center-chunk load through the shared helper.

Validation after Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

### Slice 2 - Desktop Single-View Convergence Audit

- [ ] Compare `WindowSceneRuntime` with the new shared helper.
- [ ] Move local-only desktop code that does not involve remote sessions,
  desktop CLI, actor resources, or app-specific diagnostics into shared
  helpers.
- [ ] Keep remote dedicated session recovery and desktop-specific UI/debug
  ownership app-local.

### Slice 3 - Contract Matrix

- [ ] Document each shared single-view boundary, consuming app crates, and the
  minimum test/smoke gate that covers it.
- [ ] Add or update script names so the matrix is executable instead of prose.

### Slice 4 - Web Adapter Check

- [ ] Compare the web runtime/render-section path against the shared helper's
  policy.
- [ ] Keep browser worker/ABI behavior web-local, but align naming and shared
  invariants where possible.

## Validation

Focused checks for Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Optional device validation when Android tooling is available:

```bash
pnpm native:android:apk
pnpm native:android:avd-smoke -- --skip-build
```

## Guardrails

- Do not move Android activity, JNI, package, or surface ownership into
  `mclone-app-runtime`.
- Do not add OpenXR behavior to the flat Android app.
- Do not change desktop remote-session behavior while extracting flat Android's
  local integrated runtime.
- Do not make web depend on native OS threads or native filesystem assumptions.
- Keep screenshots and logs under `/tmp`.
