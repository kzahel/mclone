# 087: Android XR Remote Host Adapter

Status: completed first pass. Android XR can now construct the shared XR scene
against a TCP remote-dedicated runtime through `debug.mclone.remote_addr`.
Compile validation passed; a Quest remote smoke against a reachable dedicated
server is still pending.

## Purpose

Close the Android XR host-mode gap left after tactical 086. The invariant is
that client platform and host mode stay separate axes: Android XR should be able
to run local integrated or remote dedicated without forking gameplay, client
runtime, render-session, or XR scene internals.

## Current State

- `mclone-xr-scene::XrMcloneTerrainState<S>` is host-pluggable and composes
  `NativeSingleViewSceneRuntime<S>`.
- Desktop XR passes the desktop TCP `RemoteServerSession`.
- Android XR now owns its own concrete `AndroidXrRemoteServerSession` wrapper
  around `mclone_net::NativeClientSession`.
- Android XR startup reads `debug.mclone.remote_addr`. When set, it constructs
  `NativeSingleViewSceneRuntime::remote_dedicated_with_mesh_assets(...)`; when
  unset or empty, it constructs the local integrated runtime with the same
  preloaded assets and still enters the shared XR scene through
  `XrMcloneTerrainState::with_runtime(...)`.
- Quest install/validate scripts expose `--remote-addr ADDR` and clear the
  property when omitted so local smokes are deterministic.

## Implementation Slice

- [x] Add direct Android XR app dependencies on `mclone-net` and
  `mclone-protocol`.
- [x] Add an Android XR TCP remote-session adapter implementing
  `RemoteDedicatedServerSession`.
- [x] Route local and remote Android XR startup through
  `XrMcloneTerrainState::with_runtime(...)`.
- [x] Keep Android activity, OpenXR loader/session/swapchain, asset staging, and
  debug-property parsing in the Android XR app crate.
- [x] Add `--remote-addr` support to `android-xr/install-quest-openxr.sh` and
  `android-xr/validate-quest-openxr.sh`.
- [x] Update platform parity and hosting docs.

Recorded result:

- Android XR local and remote host selection now differs only by the
  app-owned session adapter and `NativeSingleViewSceneRuntime` constructor.
- The remote path uses the same shared command/update, reconnect, resync,
  render-section, actor, and locomotion paths as desktop XR.
- No desktop TCP session type moved into Android XR; platform transport remains
  app-local.

## Validation

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime host_mode
cargo fmt --manifest-path native/Cargo.toml --all --check
bash -n android-xr/startup-properties.sh android-xr/install-quest-openxr.sh android-xr/validate-quest-openxr.sh
git diff --check
```

Still pending:

```bash
pnpm native:android-xr:validate -- --debug --skip-build --remote-addr HOST:25565 --view-pose 0,120,-96,180
```

That device smoke needs a dedicated server reachable from the headset.

## Guardrails

- Do not move Android debug-property parsing, JNI/activity lifecycle, or OpenXR
  swapchain ownership into shared runtime crates.
- Do not make Android XR depend on the desktop app's `RemoteServerSession`.
- Keep future WebSocket, P2P, or in-app connect UI work behind the same
  `RemoteDedicatedServerSession` / command-update contract rather than adding
  an XR-specific gameplay path.
