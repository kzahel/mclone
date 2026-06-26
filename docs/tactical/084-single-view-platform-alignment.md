# 084: Single-View Platform Alignment

Status: active. Slice 1 is complete. Slice 2 has moved desktop's
static-client and live `WindowSceneRuntime` paths onto shared app-runtime
helpers. Slice 3 has moved remote dedicated dispatch/resync policy, flat
Android remote-host selection, and the native local/remote single-view scene
shell into shared app-runtime contracts while keeping concrete transports and
platform config in app crates. Slice 4's durable matrix now lives in
`docs/topics/platform-parity.md`; the remaining alignment work is making the
sentinel gates more executable and auditing web/XR forks against the shared
contracts.

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
- Desktop flat wraps the shared native local/remote scene shell through
  `WindowSceneRuntime`. Desktop still owns `winit`, headless/perf/XR-smoke
  entrypoints, CLI options, actor textures, UI/debug state, and concrete TCP
  session construction.
- Flat Android renders real terrain through shared crates, selects local
  integrated or remote dedicated through the shared host-mode contract, and now
  uses a shared native local/remote scene shell. Android still owns
  `NativeActivity`, property lookup, TCP session construction, surface/device
  ownership, touch input, and APK validation scripts.
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
- [x] Compare live `WindowSceneRuntime` with the new shared helper.
- [x] Move local-only desktop code that does not involve remote sessions,
  desktop CLI, actor resources, or app-specific diagnostics into shared
  helpers.
- [x] Keep concrete remote transport and desktop-specific UI/debug ownership
  app-local while shared host-mode code owns recovery/resync semantics.

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

Recorded Slice 2 third-chunk result:

- Extended `NativeSingleViewSceneRuntime<S>` with the live desktop-facing
  surface: core accessors for tests/diagnostics, chunk-view changes, player
  position update drains, forced day-time, pending render-work queries,
  block/sky/time facts, and shared runtime stats.
- Replaced desktop `WindowSceneRuntime`'s owned `SingleViewRuntime`, local
  integrated runner, remote-session slot, and render compile worker with
  `NativeSingleViewSceneRuntime<RemoteServerSession>`.
- Desktop now keeps platform/app concerns only: actor textures, `winit`,
  headless/perf/XR-smoke entrypoints, CLI scene options, UI/debug state, and
  concrete TCP session construction.
- Shared app-runtime now owns desktop and flat Android terrain mesh assets,
  render compile queue, polling/idle waits, render-section sync, cached
  sections, traversal-ready queries, local command dispatch, and remote
  dedicated dispatch/reconnect/resync behavior for the native single-view
  shell.
- Updated desktop window, headless, perf, and XR smoke code to read terrain
  mesh assets through `WindowSceneRuntime::mesh_assets()` instead of a
  desktop-owned field.

Validation after Slice 2 third chunk:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo fmt --manifest-path native/Cargo.toml --all --check
pnpm native:web:build
git diff --check
```

### Slice 3 - Dedicated Host Mode Contract

- [x] Introduce a shared host-mode/options contract in `mclone-app-runtime`
  that represents local integrated and remote dedicated play without desktop
  app terminology.
- [x] Move command/update exchange and resync semantics behind a shared
  session/transport boundary. Desktop TCP, browser WebSocket, Android network
  config, and future P2P should be adapters around that boundary.
- [x] Keep desktop's existing `--remote-addr` behavior working while making it
  one consumer of the shared host-mode contract.
- [x] Add a flat Android remote dedicated configuration path once the shared
  contract exists. Android-specific property/intent/UI details stay in the app
  crate.
- [x] Move flat Android's local/remote scene wrapper into `mclone-app-runtime`
  so Android does not own render-section, mesh asset, sky/time, or
  traversal-ready dispatch for host-mode variants.
- [x] Add host-mode conformance coverage so local integrated and remote
  dedicated construction can be validated without running every device lane.

Recorded Slice 3 first-chunk result:

- Added `mclone-app-runtime::host_mode` with `SingleViewHostMode`,
  `SingleViewHostOptions`, `RemoteDedicatedServerSession`, and shared helpers
  for initial remote dedicated client construction, command/update application,
  reconnect preparation, replica clearing, and chunk-view resync.
- Kept concrete TCP in `mclone-native-client::remote_session::RemoteServerSession`.
  The desktop session now implements the shared `RemoteDedicatedServerSession`
  trait.
- Rewired desktop remote client construction and live remote command dispatch
  through `mclone-app-runtime::host_mode`.
- Removed desktop-private copies of remote command success handling,
  reconnect/resync, and client-replica clearing policy from
  `WindowSceneRuntime`.
- Added app-runtime host-mode unit coverage for initial remote chunk-view load,
  normal remote update application, and reconnect/resync after a failed
  command.

Validation after Slice 3 first chunk:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime host_mode
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client remote_session
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Recorded Slice 3 second-chunk result:

- Added flat Android remote dedicated selection through Android-owned
  `debug.mclone.remote_addr`. When unset or empty, flat Android remains local
  integrated.
- Added `INTERNET` permission to the flat Android manifest and `mclone-net` /
  `mclone-protocol` dependencies to `mclone-android-client`.
- Added an Android TCP session adapter implementing
  `RemoteDedicatedServerSession`. The app owns address/property lookup and TCP;
  remote command application and reconnect/resync policy still come from
  `mclone-app-runtime::host_mode`.
- Added an Android single-view scene wrapper that presents the same render,
  polling, cached-section, sky/time, and traversal-ready surface to the frame
  renderer for local integrated and remote dedicated host modes.
- Updated AVD and Quest-flat validators with `--remote-addr`; validators clear
  `debug.mclone.remote_addr` when the option is omitted so local smokes remain
  deterministic.

Validation after Slice 3 second chunk:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime host_mode
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash -n android/validate-avd.sh android/validate-quest-flat.sh android/validate-common.sh
pnpm native:web:build
cd native
cargo ndk -t arm64-v8a -o ../android/jniLibs build --release --package mclone-android-client --lib
cd ../android
.\gradlew.bat assembleDebug
git diff --check
```

Recorded Slice 3 third-chunk result:

- Added `NativeSingleViewSceneRuntime<S>` to
  `mclone-app-runtime::local_single_view`, pairing
  `LocalSingleViewSceneRuntime` with a generic remote dedicated scene runtime
  over `RemoteDedicatedServerSession`.
- Added `RemoteDedicatedSingleViewSceneRuntime<S>` for native remote hosts.
  It owns shared remote client initialization, render-section compilation,
  cache/traversal-ready queries, mesh assets, and sky/time facts.
- Added shared local/remote methods for host labels, mesh assets, render
  distance, loaded chunk count, polling/idle wait, render-section sync, cached
  sections, traversal-ready section keys, sky color, time of day, sun angle,
  and gameplay command dispatch.
- Rewired flat Android to call the shared scene shell. Android now retains only
  `debug.mclone.remote_addr` lookup and the concrete
  `AndroidRemoteServerSession` TCP adapter.

Validation after Slice 3 third chunk:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm native:web:build
cd native
cargo ndk -t arm64-v8a -o ../android/jniLibs build --release --package mclone-android-client --lib
git diff --check
```

### Slice 4 - Contract Matrix

- [x] Document each shared single-view boundary, consuming app crates, and the
  minimum test/smoke gate that covers it.
- [ ] Add or update script names so the matrix is executable instead of prose.

Recorded Slice 4 first-chunk result:

- `docs/topics/platform-parity.md` now owns the durable feature/platform matrix
  and shared-contract/consumer matrix.
- Refreshed the matrix after the native scene-shell convergence: desktop
  `WindowSceneRuntime` and flat Android both compose
  `NativeSingleViewSceneRuntime<S>`, and flat Android has a TCP remote-dedicated
  path through `debug.mclone.remote_addr`.
- Left the remaining contract gaps explicit: web still carries an async host
  fork and inline render path, while XR still carries a terrain-state fork and
  lacks a remote transport adapter.

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
