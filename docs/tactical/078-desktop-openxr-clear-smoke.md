# 078: Desktop OpenXR Clear Smoke

Status: active; Slice 2D Windows/Linux Vulkan graphics session compiles; local Windows runtime reaches instance diagnostics but reports the HMD unavailable, swapchains/frame loop next.

## Purpose

Add the first desktop OpenXR runtime path for mclone without pulling in the full
client/server/render-session stack.

This slice should prove loader discovery, instance/session creation, graphics
binding, swapchain acquisition, frame wait/begin/end, projection layer
submission, and validation/logging. It should not yet render the mclone world.

## Why Desktop First

Desktop OpenXR is the lowest-risk way to prove stereo runtime ownership before
Android XR:

- It avoids Quest packaging, Android loader initialization, and headset storage
  policy.
- It lets the renderer/device/swapchain boundary be debugged with normal native
  logs and desktop tools.
- It creates the host shape that Android XR should reuse later instead of
  growing out of the flat Android `NativeActivity` path.

## Target Shape

Add desktop OpenXR behind an explicit opt-in feature and app mode, for example:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke
```

Expected ownership:

- `mclone-native-client` owns desktop OpenXR startup and validation mode.
- OpenXR loader, instance, session, spaces, frame loop, and swapchains stay in
  the app/platform layer.
- `mclone-render` may keep shared `wgpu` resources and helpers, but must not
  own OpenXR sessions or action sets.
- Shared engine crates remain OpenXR-free.

The first image can be a per-eye clear color or simple renderer-independent
test pattern. It should still acquire real XR swapchain images and submit a
projection layer through the runtime.

## Playbox References

Use Playbox as a pattern library only:

- `~/code/playbox/docs/architecture/platforms.md` for desktop OpenXR platform
  ownership.
- `~/code/playbox/src/xr/` for loader/session/swapchain/frame-loop shape.
- `~/code/playbox/Cargo.toml` for the opt-in `xr` feature shape:
  `default = []`, `xr = ["dep:ash", "dep:openxr", "dep:libloading"]`.
- `~/code/playbox/src/main.rs` for the CLI pattern where non-XR builds accept
  the flag shape but fail with a clear "rebuild with `--features xr`" message.
- `~/code/playbox/src/xr/mod.rs` for the registered-loader-first OpenXR entry
  path with `XR_RUNTIME_JSON` and `MONADO_OPENXR_RUNTIME_PATH` fallbacks.
- `~/code/playbox/src/xr/graphics_metal.rs` for the macOS
  `XR_KHR_metal_enable` graphics binding, runtime-required `MTLDevice`
  matching, and raw Metal command-queue session creation.
- `~/code/playbox/scripts/start-xr.sh` for the macOS WiVRn launcher/env
  defaults and optional Quest USB tunnel workflow.
- `~/code/playbox/scripts/run_playbox_wivrn_capture.sh` for the fully explicit
  WiVRn runtime manifest/library env used in capture validation.
- `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md` if a
  companion window is needed later. Do not add companion-window complexity in
  this first smoke unless required by the local runtime.

Do not copy Playbox's PhysX, scene, egui, controller, body, passthrough, or
spatial-room systems.

## Implementation Slices

### Slice 1 - Dependency And Feature Gate

- [x] Add the minimum OpenXR dependency behind an `xr` feature.
- [x] Keep default desktop, web, Android, and dedicated-server builds unchanged.
- [x] Add a CLI mode for the clear smoke.
- [x] Document that no runtime prerequisites or environment variables are used
  before the loader/session slice.

Landed in Slice 1:

- `mclone-native-client` now has an opt-in `xr` feature modeled after
  Playbox's feature gate: optional `ash`, `openxr`, and `libloading`
  dependencies with `default = []`.
- `--xr-clear-smoke [--frames N]` is parsed as a separate run mode and rejects
  combinations with headless or perf modes.
- Non-XR builds retain the CLI mode but return
  `rebuild with --features xr to use --xr-clear-smoke`.
- XR-enabled builds compile a small placeholder module that proves the feature
  dependency path and leaves loader/session/swapchain work to Slice 2.
- No OpenXR types were added to shared engine crates.

Runtime prerequisites:

- Slice 1 does not load the OpenXR runtime and does not require runtime
  environment variables.
- Slice 2 should document the exact desktop runtime path it uses. Based on
  Playbox, likely candidates are loader/runtime discovery variables such as
  `XR_RUNTIME_JSON` or Monado/WiVRn-specific runtime paths, but mclone should
  record only variables it actually consumes.

Validation:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:web:build
```

Slice 1 validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
# Expected failure with a feature hint:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --xr-clear-smoke --frames 2
# Slice 1 expected success, before runtime probing landed:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Expected Slice 1 CLI results:

- Without `--features xr`: exits with
  `rebuild with --features xr to use --xr-clear-smoke`.
- With `--features xr` at the Slice 1 commit: exited successfully after
  printing that loader/session/swapchain bring-up was next. Current Slice 2A
  behavior attempts real runtime probing instead.

### Slice 2 - Runtime And Graphics Binding

- [x] Load the active OpenXR runtime.
- [x] Create an OpenXR instance with the required graphics extension for the
  local desktop backend.
- [x] Create the runtime-selected Metal graphics device on Apple targets.
- [x] Create a Metal session and `STAGE` reference space on Apple targets.
- [x] Create the runtime-selected Vulkan graphics device on Windows/Linux.
- [ ] Create color swapchains for both eyes and host-owned depth targets.

Landed in Slice 2A:

- `--xr-clear-smoke` now loads the OpenXR entry instead of using the placeholder
  feature probe.
- Loader discovery follows the Playbox pattern: use the registered OpenXR
  loader first, then try `MONADO_OPENXR_RUNTIME_PATH`, `XR_RUNTIME_JSON`
  manifest resolution, and known Windows runtime manifest/loader paths.
- The `xr` feature also includes optional `serde_json` for runtime manifest
  parsing; it is not part of default native, web, or Android builds.
- The desktop graphics extension is selected by target backend:
  `XR_KHR_metal_enable` on Apple targets and `XR_KHR_vulkan_enable2` elsewhere.
- The smoke creates an OpenXR instance when a runtime is available, queries
  runtime properties, finds the head-mounted display system, logs blend modes,
  and validates that `PRIMARY_STEREO` reports at least two views.
- Android XR remains out of scope. If this desktop smoke is built for Android,
  it exits with a desktop-only error instead of adding Android loader or
  package behavior.

Runtime prerequisites:

- A system OpenXR loader/runtime registration, or one of:
  - `XR_RUNTIME_JSON=/path/to/active_runtime.json`
  - `MONADO_OPENXR_RUNTIME_PATH=/path/to/runtime_library`
- The current local machine has no registered loader and no fallback path set,
  so the command fails before instance creation with:
  `failed to load OpenXR loader (dlopen failed); no fallback paths were found`.

Slice 2A validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
# Expected local failure until an OpenXR runtime is installed/configured:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Landed in Slice 2B:

- Added an app-local `graphics_metal` module modeled after Playbox's Metal
  OpenXR graphics binding.
- Implemented a Metal `openxr::Graphics` type that calls
  `xrGetMetalGraphicsRequirementsKHR`, creates `XrGraphicsBindingMetalKHR`,
  and lets `openxr` create the session.
- Matched `wgpu`'s Metal adapter/device against the runtime-required
  `MTLDevice`, then passed the raw Metal command queue to OpenXR session
  creation.
- Created a `STAGE` reference space after session creation.
- Kept the Mac-specific `metal` and `pollster` dependencies behind the `xr`
  feature and the Apple target. Shared engine crates remain OpenXR-free.

Slice 2B validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
# Expected local failure until an OpenXR runtime is installed/configured:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Landed in Slice 2C:

- Added `scripts/start-xr.sh`, modeled after Playbox's `scripts/start-xr.sh`.
- The launcher defaults to the working local WiVRn macOS build shape:
  `HOST_BUILD_DIR=$HOME/code/wivrn-macos/build/wivrn`,
  `MONADO_OPENXR_RUNTIME_PATH=$HOST_BUILD_DIR/_deps/monado-build/src/xrt/targets/openxr/libopenxr_wivrn.dylib`,
  and `XR_RUNTIME_JSON=$HOST_BUILD_DIR/openxr_wivrn-dev.json`.
- Added `--wivrn-usb` to start/reuse `wivrn-server-headless --no-encrypt`,
  install `adb reverse tcp:9757 tcp:9757`, and launch the Quest WiVRn client
  with `wivrn+tcp://localhost:9757`.
- Added package scripts:
  - `pnpm native:xr:check`
  - `pnpm native:xr:smoke`
  - `pnpm native:xr:wivrn-usb`

Slice 2C validation:

```bash
bash -n scripts/start-xr.sh
pnpm native:xr:check
scripts/start-xr.sh --frames 2
```

`scripts/start-xr.sh --frames 2` should now use the WiVRn OpenXR runtime env
automatically on macOS when the local `~/code/wivrn-macos/build/wivrn` files
are present. Local validation reached the WiVRn runtime dylib and reported
`XR_KHR_metal_enable=true`; it then failed at `xrCreateInstance` because the
WiVRn/Monado service socket was not active:
`Failed to connect to socket .../Library/Caches/monado/wivrn/comp_ipc`.
Use `scripts/start-xr.sh --wivrn-usb --frames 2` to start the host, install the
ADB USB tunnel, launch the Quest WiVRn client, and then run the smoke. The
local `--wivrn-usb` validation stopped before host startup because ADB reported
`no devices/emulators found`.

Landed in Slice 2D:

- Added an app-local `graphics_vulkan` module modeled after Playbox's Vulkan
  OpenXR graphics binding.
- Implemented the non-Apple desktop path with `openxr::Vulkan`,
  `XR_KHR_vulkan_enable2`, runtime-created Vulkan instance/device, and the
  runtime-selected physical device.
- Wrapped the runtime-selected Vulkan instance/device into `wgpu` so the next
  swapchain slice can reuse the same graphics ownership boundary.
- Created a Vulkan OpenXR session and `STAGE` reference space on non-Apple
  desktop targets after runtime/system diagnostics succeed.
- Kept all OpenXR/Vulkan ownership inside `mclone-native-client`; no shared
  engine crates gained OpenXR types.

Windows runtime state on June 25, 2026:

- `XR_RUNTIME_JSON` and `MONADO_OPENXR_RUNTIME_PATH` were unset.
- `HKLM\SOFTWARE\Khronos\OpenXR\1\ActiveRuntime` was
  `C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json`.
- Installed runtime manifests found locally:
  - Virtual Desktop:
    `C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json`
    (`VirtualDesktopXR (Bundled)`, library `.\virtualdesktop-openxr.dll`)
  - Meta:
    `C:\Program Files\Meta Horizon\Support\oculus-runtime\oculus_openxr_64.json`
    (`Oculus OpenXR`, library `.\LibOVRRTImpl64_1.dll`)
  - SteamVR:
    `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\steamxr_win64.json`
    (`SteamVR`, library `bin\vrclient_x64.dll`)
- The generic OpenXR loader was not present at
  `C:\Windows\System32\openxr_loader.dll`; the smoke loaded SteamVR's
  `openxr_loader.dll` fallback, which then selected the active/explicit
  runtime.
- A Quest 3 was visible over ADB on Android 14 and `OVRService` was running,
  but the desktop OpenXR runtimes did not report an available HMD form factor.

Slice 2D validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
pnpm native:web:build
```

Local setup notes:

- `cargo test -p mclone-native-client` initially failed two asset-dependent
  tests because this Windows checkout had no hydrated
  `reference/minecraft-1.17.1/extracted` tree or `extracted.zip`. Hydrating the
  local 1.17.1 client assets and running
  `python .\scripts\build-reference-asset-pack.py` made the package tests pass
  (`70 passed`).
- `pnpm native:web:build` initially failed because the Rust
  `wasm32-unknown-unknown` target was not installed on this machine. After
  `rustup target add wasm32-unknown-unknown`, the build passed.

Windows runtime probes:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
```

Observed with the active Virtual Desktop runtime:

- Entry: fallback loader
  `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll`
- Runtime: `VirtualDesktopXR v1.0.10`
- Extensions: `XR_KHR_vulkan_enable2=true`, `ext_debug_utils=true`,
  `ext_hand_tracking=true`, `fb_display_refresh_rate=true`
- Result: failed before system/session creation at
  `locate OpenXR head-mounted display system` with
  `FORM_FACTOR_UNAVAILABLE` ("device is currently not available").

Observed with the Meta runtime forced explicitly:

```powershell
$env:XR_RUNTIME_JSON='C:\Program Files\Meta Horizon\Support\oculus-runtime\oculus_openxr_64.json'
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
```

- Entry: same SteamVR `openxr_loader.dll` fallback.
- Runtime: `Oculus v1.117.0`
- Extensions: `XR_KHR_vulkan_enable2=true`, `ext_debug_utils=true`,
  `ext_hand_tracking=true`, `fb_display_refresh_rate=true`
- Result: same `FORM_FACTOR_UNAVAILABLE` before system/session creation.

The Vulkan session path is compile-validated on Windows in this slice, but the
local runtime did not reach session creation until the headset is active in a
PC OpenXR runtime (for example, Virtual Desktop connected in-headset or Quest
Link active for the Meta runtime).

Implementation should follow the measured platform path. On macOS this likely
means Metal-specific runtime/device matching. On Windows/Linux this likely
means Vulkan instance/device negotiation. Keep backend-specific code isolated
inside the app/platform XR module.

### Slice 3 - Clear Frame Submission

- [ ] Wait/begin/end OpenXR frames.
- [ ] Acquire and release per-eye swapchain images.
- [ ] Clear each eye to a visible diagnostic color or pattern.
- [ ] Submit a projection layer using runtime-provided views/projections.
- [ ] Run for a bounded number of frames in smoke mode and exit cleanly.

Suggested validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 120
```

Record the exact runtime, platform backend, and observed result in this doc
when the smoke lands.

## Out Of Scope

- Android XR / Quest packaging.
- Controller action bindings.
- Hand tracking.
- Passthrough.
- Spatial room APIs.
- In-world UI.
- Mclone world/client/server rendering.
- Desktop companion window, unless needed only to keep the runtime alive.

## Review Rejection Criteria

- OpenXR types leaking into shared client, server, protocol, mesh, asset,
  worldgen, light, or app-runtime crates.
- Android XR manifest/package changes in this desktop smoke.
- A fake stereo path that does not acquire real OpenXR swapchain images.
- A permanent dependency on one local runtime path without documented fallback
  or clear error reporting.

## Completion Criteria

- The native client has an opt-in desktop OpenXR clear-smoke mode.
- The smoke creates a real OpenXR session, renders/submits bounded frames, and
  exits cleanly.
- Non-XR desktop, native web, and flat Android builds remain unaffected.
- The follow-up mclone-frame tactical can reuse the XR frame loop and swapchain
  ownership without reopening loader/session work.
