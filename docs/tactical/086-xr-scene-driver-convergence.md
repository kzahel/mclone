# 086: XR Scene Driver Convergence

Status: active. Slices 1 and 2 landed: XR transform helpers now live in
`mclone-xr-scene`, and actor presentation-to-render-instance building now lives
in `mclone-render-session`. Slice 3, rendering actors through the shared XR
scene, is the next implementation chunk.

## Purpose

Delete the desktop-only `XrMcloneWorldState` fork by widening
`mclone-xr-scene` into the one XR scene driver shared by desktop OpenXR and
Android XR.

The end state is one actor-capable, host-pluggable XR scene driver. Desktop XR
should keep OpenXR runtime/session/swapchain glue and option parsing in the app
crate, while terrain runtime policy, render-view transforms, actor instance
construction, and host-mode integration live behind shared native contracts.

## Current State

- Android XR constructs `mclone-xr-scene::XrMcloneTerrainState`.
- Desktop XR still constructs app-local `XrMcloneWorldState` in
  `mclone-native-client`.
- XR tracking-origin, stage-to-world, render-view conversion, yaw
  normalization, and startup view-pose helpers now live in `mclone-xr-scene`;
  desktop XR imports them.
- Desktop flat, headless/perf, web, and desktop XR now share
  `mclone-render-session::actor_instances_from_presentations(...)`.
- Desktop XR renders actors through `ActorDrawResources`; shared
  `mclone-xr-scene` currently renders terrain only.
- `mclone-xr-scene` owns a concrete local integrated `SingleViewRuntime` plus
  `NativeIntegratedServerRunner` plumbing instead of composing
  `NativeSingleViewSceneRuntime<S>`.

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

- [ ] Add `ActorDrawResources` ownership to `XrMcloneTerrainState`.
- [ ] Pass actor atlas assets into `mclone-xr-scene::new(...)`.
- [ ] Render shared XR actor instances through `render_eye_target(...)`.
- [ ] Repoint Android XR to pass the actor atlas it currently loads but
  discards.
- [ ] Update platform parity docs based on actual validation status.

### Slice 4 - Pluggable XR Host Runtime

- [ ] Replace `XrMcloneTerrainState`'s hand-rolled local runtime/runner plumbing
  with `NativeSingleViewSceneRuntime<S>`.
- [ ] Make the XR scene state generic over `S: RemoteDedicatedServerSession`.
- [ ] Provide a local-only constructor/type path for Android XR.
- [ ] Repoint desktop XR to construct the shared scene driver with
  `RemoteServerSession`.
- [ ] Delete app-local `XrMcloneWorldState`.
- [ ] Update platform parity docs to reflect shared XR scene-driver ownership.

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
