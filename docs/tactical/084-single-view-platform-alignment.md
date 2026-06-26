# 084: Single-View Platform Alignment

Status: active. Slice 1 is complete. Slice 2 has two chunks landed:
desktop's local static client-runtime construction and shared render-section
queue draining now use `mclone-app-runtime` helpers, while deeper live
`WindowSceneRuntime` convergence remains next.

## Purpose

Keep desktop flat, flat Android, headless capture, and web/WASM aligned around
shared single-view runtime/render contracts so new features do not require
manual platform-by-platform implementation work.

An important invariant for this tactical: client platform and server host mode
are separate axes. Local integrated is a default/validation mode, not a
platform identity. Every client lane should retain a path to dedicated-server
play, and future P2P/shared-session work should fit behind the same
command/update contracts.

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
  private local-only scene shell; the first extraction removed its private
  `AndroidSceneRuntime`, but it still has no dedicated-server host-mode path.
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

- host-mode-neutral local-integrated versus remote-dedicated runtime selection
- local integrated runtime setup and chunk-view dispatch
- dedicated-server command/update exchange semantics
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

- [x] Move desktop's local static client-runtime construction onto shared
  local single-view helpers without changing remote-session behavior.
- [x] Move native blocking render-section "sync all until idle" policy onto
  `SingleViewRuntime`.
- [ ] Compare live `WindowSceneRuntime` with the new shared helper.
- [ ] Move local-only desktop code that does not involve remote sessions,
  desktop CLI, actor resources, or app-specific diagnostics into shared
  helpers.
- [ ] Keep remote dedicated session recovery and desktop-specific UI/debug
  ownership app-local.

Recorded Slice 2 first-chunk result:

- Added `build_local_single_view_client_runtime(...)` to
  `mclone-app-runtime::local_single_view`.
- Added shared `drain_integrated_server_runner_until_idle(...)` for native
  integrated-server runner jobs.
- Rewired desktop `build_scene_client_runtime(...)` so the local integrated
  branch uses the shared helper.
- Left the remote branch in `mclone-native-client` so remote session transport
  and recovery stay app-local.
- Added an asset-independent app-runtime unit test proving the shared helper
  loads the center chunk through the local integrated server.

Validation after Slice 2 first chunk:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client scene_client_runtime_loads_center_chunk
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Recorded Slice 2 second-chunk result:

- Added native-only `SingleViewRuntime::sync_all_render_sections(...)` in
  `mclone-app-runtime`.
- Rewired `LocalSingleViewSceneRuntime::sync_all_render_sections(...)` to use
  the shared helper.
- Rewired desktop `WindowSceneRuntime::sync_all_render_sections(...)` to use
  the same shared helper.
- Removed the now-dead desktop passthrough for render ready-work checks.
- Kept the helper native-only because web/WASM render-worker draining is
  nonblocking and should not inherit a blocking sleep-until-idle API.

Validation after Slice 2 second chunk:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client render_compile
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

### Slice 3 - Dedicated Host Mode Contract

- [ ] Introduce a shared host-mode/options contract in `mclone-app-runtime`
  that represents local integrated and remote dedicated play without desktop
  app terminology.
- [ ] Move command/update exchange and resync semantics behind a shared
  session/transport boundary. Desktop TCP, browser WebSocket, Android network
  config, and future P2P should be adapters around that boundary.
- [ ] Keep desktop's existing `--remote-addr` behavior working while making it
  one consumer of the shared host-mode contract.
- [ ] Add a flat Android remote dedicated configuration path once the shared
  contract exists. Android-specific property/intent/UI details stay in the app
  crate.
- [ ] Add host-mode conformance coverage so local integrated and remote
  dedicated construction can be validated without running every device lane.

### Slice 4 - Contract Matrix

- [ ] Document each shared single-view boundary, consuming app crates, and the
  minimum test/smoke gate that covers it.
- [ ] Add or update script names so the matrix is executable instead of prose.

### Slice 5 - Web Adapter Check

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
- Do not regress desktop remote-session behavior while extracting shared
  runtime contracts; remote transport generalization should be explicit and
  covered by tests.
- Do not make web depend on native OS threads or native filesystem assumptions.
- Keep screenshots and logs under `/tmp`.
