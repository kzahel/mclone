# 086: XR Scene Driver Convergence

Status: completed. Slices 1-4 landed: XR transform helpers now live in
`mclone-xr-scene`, actor presentation-to-render-instance building now lives in
`mclone-render-session`, the shared XR scene owns actor draw resources, and
desktop XR now consumes the shared scene driver. The remaining Android XR
remote-transport follow-up landed in
[`087`](087-android-xr-remote-host-adapter.md).

## Purpose

Delete the desktop-only `XrMcloneWorldState` fork by widening
`mclone-xr-scene` into the one XR scene driver shared by desktop OpenXR and
Android XR.

The end state is one actor-capable, host-pluggable XR scene driver. Desktop XR
should keep OpenXR runtime/session/swapchain glue and option parsing in the app
crate, while terrain runtime policy, render-view transforms, actor instance
construction, and host-mode integration live behind shared native contracts.

## Current State

- Android XR constructs `mclone-xr-scene::XrMcloneTerrainState` through the
  local path in this tactical; tactical 087 added the Android-owned TCP remote
  adapter.
- Desktop XR constructs `mclone-xr-scene::XrMcloneTerrainState` with the
  desktop TCP `RemoteServerSession` type.
- XR tracking-origin, stage-to-world, render-view conversion, yaw
  normalization, and startup view-pose helpers now live in `mclone-xr-scene`;
  desktop XR imports them.
- Desktop flat, headless/perf, web, and desktop XR now share
  `mclone-render-session::actor_instances_from_presentations(...)`.
- Shared `mclone-xr-scene` now owns `ActorDrawResources`, accepts an actor
  atlas, and renders actor instances. Android XR passes the actor atlas it
  already loaded.
- `mclone-xr-scene` composes `NativeSingleViewSceneRuntime<S>` instead of
  re-owning local integrated runner, polling, idle wait, render-section sync,
  and host command plumbing.
- Desktop app code still owns OpenXR runtime/session/swapchain setup and the
  concrete desktop TCP session type. Android XR still owns
  activity/loader/packaging and now owns its concrete TCP session adapter.

## Target Shape

- `mclone-xr-scene` owns XR scene state and render-frame orchestration shared
  by desktop OpenXR and Android XR.
- App crates own platform/session/swapchain construction, asset loading, and
  platform-specific launch options.
- Actor presentation to render-instance conversion is shared by desktop flat,
  web, and XR callers.
- XR scene state is generic over the remote dedicated session adapter, with a
  local-only constructor/type path so Android XR does not need to name desktop
  TCP session types.
- Client platform and host mode remain separate axes: every client lane keeps a
  path toward dedicated-server play without forking gameplay or render-session
  internals.

## Implementation Slices

### Slice 1 - Shared XR Transform Helpers

- [x] Make the duplicated XR transform helpers public in `mclone-xr-scene`.
- [x] Add a shared startup view-pose helper used by both `XrMcloneTerrainState`
  and desktop XR.
- [x] Delete the desktop-local helper copies from `xr_clear_smoke.rs`.
- [x] Move duplicate transform tests into `mclone-xr-scene`.
- [x] Validate `mclone-xr-scene`, desktop XR compile, Android XR compile, and
  formatting gates.

Recorded Slice 1 result:

- Promoted `XrTrackingOrigin`, `XrStageToWorld`, render-view conversion, yaw
  normalization, Vec3/Vec3d conversion, and startup view-pose helpers to the
  `mclone-xr-scene` public API.
- Rewired desktop XR to import those helpers from `mclone-xr-scene`.
- Removed the duplicate desktop helper block and moved the transform tests into
  `mclone-xr-scene`.

Validation after Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
```

### Slice 2 - Shared Actor Instance Builder

- [x] Move `actor_instances_from_presentations(...)` and light-probe helpers
  out of app-local desktop/web code into a shared native crate.
- [x] Keep actor presentations as an input parameter so flat desktop can pass
  interpolated presentations and XR can pass raw client presentations.
- [x] Repoint desktop flat/headless/perf, web, and desktop XR callers.
- [x] Validate native client, web client, render-session, app-runtime, Android
  XR, and wasm build gates.

Recorded Slice 2 result:

- Added `mclone-render` as a dependency of `mclone-render-session` so
  renderer-facing actor instances can be built at the render-session boundary.
- Moved actor instance construction and actor light-probe helpers into
  `mclone-render-session`.
- Rewired desktop flat, headless, perf, desktop XR, and web canvas callers to
  use the shared helper.
- Removed duplicate native app and web canvas helper implementations while
  preserving caller-owned interpolation policy.

Validation after Slice 2:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm native:web:build
```

### Slice 3 - Actors In Shared XR Scene

- [x] Add `ActorDrawResources` ownership to `XrMcloneTerrainState`.
- [x] Pass actor atlas assets into `mclone-xr-scene::new(...)`.
- [x] Render shared XR actor instances through `render_eye_target(...)`.
- [x] Repoint Android XR to pass the actor atlas it currently loads but
  discards.
- [x] Update platform parity docs based on actual validation status.

Recorded Slice 3 result:

- Added `ActorDrawResources` to `XrMcloneTerrainState`.
- Added an actor-atlas argument to `XrMcloneTerrainState::new(...)`.
- Built shared XR actor instances through
  `mclone-render-session::actor_instances_from_presentations(...)` and passed
  them to both eye renders.
- Rewired Android XR to pass `actor_assets.atlas` into the shared scene instead
  of discarding loaded actor assets.
- Extended `XrTerrainFrameSummary` and Android XR readiness/first-frame logs
  with actor counts.

Validation after Slice 3:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
```

Device visual validation is still pending: this slice compiles and wires the
shared Android XR actor path, but Quest smoke/logcat validation must confirm
nonzero actor rendering when the runtime has remote players or passive entities
to present.

### Slice 4 - Pluggable XR Host Runtime

- [x] Replace `XrMcloneTerrainState`'s hand-rolled local runtime/runner plumbing
  with `NativeSingleViewSceneRuntime<S>`.
- [x] Make the XR scene state generic over `S: RemoteDedicatedServerSession`.
- [x] Provide a local-only constructor/type path for Android XR.
- [x] Repoint desktop XR to construct the shared scene driver with
  `RemoteServerSession`.
- [x] Delete app-local `XrMcloneWorldState`.
- [x] Update platform parity docs to reflect shared XR scene-driver ownership.

Recorded Slice 4 result:

- Added preloaded-mesh constructors to
  `NativeSingleViewSceneRuntime<S>` so platform apps can keep asset staging
  while sharing the scene runtime shell.
- Made `XrMcloneTerrainState<S>` generic over
  `S: RemoteDedicatedServerSession` and added an uninhabited
  `XrLocalOnlyRemoteSession` default for Android/local-only callers.
- Added `XrMcloneTerrainState::with_runtime(...)` so app crates can pass a
  local or remote `NativeSingleViewSceneRuntime<S>`.
- Rewired desktop XR to construct the shared scene with
  `RemoteServerSession` and deleted app-local `XrMcloneWorldState`.
- Kept OpenXR session/swapchain/controller glue in the desktop and Android XR
  app crates.

Validation after Slice 4:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
```

## Validation

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
git diff --check
```

Device/runtime smokes still matter for Slices 3 and 4:

```bash
pnpm native:xr:*
pnpm native:android-xr:validate
```

## Guardrails

- Do not move OpenXR session, swapchain, Android activity, or runtime-loader
  ownership into shared game/runtime crates.
- Do not make Android XR depend on the desktop TCP session type.
- Do not mark Android XR actor support complete until actor rendering is wired
  and validated, not merely because the atlas is passed through.
- Do not force native blocking runtime paths into async to match browser/Web
  mechanics.
- Keep future dedicated-server and P2P topology behind shared command/update
  contracts rather than platform-specific gameplay forks.
