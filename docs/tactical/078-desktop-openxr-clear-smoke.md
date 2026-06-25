# 078: Desktop OpenXR Clear Smoke

Status: active; Slice 3 desktop XR clear submission is validated on Windows with VirtualDesktopXR/Quest 3 over Vulkan; macOS Metal keeps the same backend shape and should be rerun on a Mac runtime.

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
- `~/code/playbox/android/validate-common.sh` for Quest wake/proximity
  save/restore helpers. Keep mclone's Windows Quest/Virtual Desktop startup
  script aligned with this source pattern instead of growing one-off ADB
  command sequences.
- `~/code/playbox/docs/quest-testing-tips.md` for Quest-side validation notes,
  including the current limitation that blind `KEYCODE_*` and tap injection do
  not reliably drive spatial VR UI.
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
- [x] Create color swapchains for both eyes and host-owned depth targets.

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

Landed in Slice 2E:

- Added `scripts/start-xr.ps1` for Windows desktop XR smoke bring-up.
- Added package scripts:
  - `pnpm native:xr:windows:check`
  - `pnpm native:xr:windows:smoke`
- The PowerShell launcher:
  - checks/builds/runs `mclone-native-client` with `--features xr`
  - defaults to Virtual Desktop's OpenXR manifest when installed unless
    `-UseActiveRuntime`, `-NoVirtualDesktop`, or `-RuntimeJson` overrides it
  - starts `VirtualDesktop.Service.exe` when present and stopped
  - launches `VirtualDesktop.Streamer.exe` when it is not already running
  - wakes a connected Quest over ADB and launches `VirtualDesktop.Android`
    unless `-NoQuestLaunch` is passed
  - runs `--xr-clear-smoke --frames N`

Additional Windows/Virtual Desktop probe on June 25, 2026:

```powershell
.\scripts\start-xr.ps1 -Frames 2
```

- Confirmed `VirtualDesktop.Service.exe` was already running.
- Launched `VirtualDesktop.Streamer.exe`.
- Confirmed Quest 3 over ADB at `192.168.1.103`, same subnet as the PC
  (`192.168.1.107`).
- Woke the headset with `adb shell svc power stayon true` and
  `KEYCODE_WAKEUP`.
- Launched the headset app package `VirtualDesktop.Android`; activity
  `md59102214312e19799944a61bf7bc2f23e.VrActivity` became focused and
  `isSleeping=false`.
- Windows firewall has an enabled inbound allow rule named
  `Virtual Desktop Streamer`.
- `C:\ProgramData\Virtual Desktop\StreamerSettings.json` still showed
  `LastConnectDate: 2026-05-28T00:00:00Z`; host logs did not record a new
  headset connection.
- The smoke still reached `VirtualDesktopXR v1.0.10` and failed at
  `xrGetSystem` with `XR_ERROR_FORM_FACTOR_UNAVAILABLE`.

The current blocker is not loader, extension, or process startup; it is that
the Virtual Desktop headset app has not completed an active connection to the
PC Streamer. A manual in-headset computer selection may be required before the
runtime exposes `HEAD_MOUNTED_DISPLAY`.

Landed in Slice 2F:

- Updated `scripts/start-xr.ps1` to use the Playbox Quest wake/restore pattern
  from `android/validate-common.sh` and `scripts/run_playbox_wivrn_capture.sh`.
- The launcher now saves and restores these Android settings:
  - `global stay_on_while_plugged_in`
  - `secure skip_launch_check_requires_controllers_enabled`
  - `global require_controllers_for_vr_apps`
- During the smoke it disables the proximity override with
  `debug.oculus.disableProximity=1`, applies the test wake settings, sends
  `KEYCODE_WAKEUP`, and broadcasts
  `com.oculus.vrpowermanager.prox_close`.
- On cleanup it force-stops `VirtualDesktop.Android`, restores the saved
  settings, re-enables proximity with `debug.oculus.disableProximity=0` and
  `com.oculus.vrpowermanager.prox_open`, then sends `KEYCODE_SLEEP`.
- Added `-NoQuestRestore` for deliberate manual headset debugging after the
  launcher has prepared the Quest. Do not use it in normal smoke validation.

Slice 2F validation:

```powershell
pnpm native:xr:windows:check
git diff --check
pnpm native:xr:windows:smoke
```

Observed pre-smoke headset baseline:

- `global stay_on_while_plugged_in=15`
- `secure skip_launch_check_requires_controllers_enabled=false`
- `global require_controllers_for_vr_apps=0`
- `debug.oculus.disableProximity=0`
- `mWakefulness=Asleep`

Observed during `pnpm native:xr:windows:smoke` on June 25, 2026:

- Quest serial: `2G0YC1ZF93041Z`
- Launcher selected
  `C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json`.
- `VirtualDesktop.Service.exe` and `VirtualDesktop.Streamer.exe` were already
  running.
- The launcher woke the Quest and launched `VirtualDesktop.Android`.
- The smoke reached `VirtualDesktopXR v1.0.10` and still failed at
  `xrGetSystem` with `XR_ERROR_FORM_FACTOR_UNAVAILABLE`.
- The launcher restore block ran after the cargo failure.

Observed post-smoke headset state:

- `global stay_on_while_plugged_in=15`
- `secure skip_launch_check_requires_controllers_enabled=false`
- `global require_controllers_for_vr_apps=0`
- `debug.oculus.disableProximity=0`
- `mWakefulness=Asleep`

Manual debug with `-NoQuestRestore` confirmed that disabling proximity and
sending `prox_close` makes the Virtual Desktop headset UI visible to ADB screen
capture. The headset UI showed the PC entry `rex`, but an ADB tap did not
complete a Streamer connection and
`C:\ProgramData\Virtual Desktop\StreamerSettings.json` still had
`LastConnectDate: 2026-05-28T00:00:00Z`. The remaining blocker is an actual
Virtual Desktop/PC connection, not mclone's OpenXR loader or Vulkan binding.

Successful connected probe on June 25, 2026:

```powershell
$env:XR_RUNTIME_JSON='C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json'
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
```

After the headset was manually connected to the PC in Virtual Desktop, the
smoke succeeded:

- Entry: fallback loader
  `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll`
- Runtime: `VirtualDesktopXR v1.0.10`
- System: `Meta Quest 3`, orientation and position tracking available.
- Blend modes: `OPAQUE`
- Stereo views:
  - eye 0 recommended `1728x1824`, max `16384x16384`, samples `1/4`
  - eye 1 recommended `1728x1824`, max `16384x16384`, samples `1/4`
- Vulkan session:
  `physical_device='NVIDIA GeForce RTX 4090' api=1.4.341 queue_family=0`
- Reference space: `STAGE`

The test was rerun after restoring the forced Quest wake/proximity settings
without force-stopping the headset app or Windows Streamer, and the direct smoke
still succeeded. The next implementation slice can assume loader, runtime,
system, Vulkan graphics binding, and `STAGE` creation are validated on this
Windows/Virtual Desktop setup.

Landed in Slice 2G:

- Added `scripts/xr-quest-virtual-desktop.psm1` as the shared Windows
  Quest/Virtual Desktop helper.
- Added `scripts/start-quest-virtual-desktop.ps1` as the simple reproducible
  startup entry point. It starts/reuses the Windows Virtual Desktop host,
  saves Quest wake settings to a temp state file, disables the proximity
  override, applies test wake settings, sends `KEYCODE_WAKEUP`, broadcasts
  `com.oculus.vrpowermanager.prox_close`, and launches `VirtualDesktop.Android`.
- Refactored `scripts/start-xr.ps1` to reuse the same helper instead of
  carrying a separate ADB wake/restore implementation.
- Added package scripts:
  - `pnpm native:xr:windows:prepare`
  - `pnpm native:xr:windows:restore`
  - `pnpm native:xr:windows:sleep`
  - `pnpm native:xr:windows:smoke:connected`

Manual Virtual Desktop flow:

```powershell
pnpm native:xr:windows:prepare
# Select/connect the PC inside the Virtual Desktop headset UI.
pnpm native:xr:windows:smoke:connected
pnpm native:xr:windows:restore
```

`native:xr:windows:restore` restores Quest wake/proximity settings from the
saved state file and sleeps the headset. It intentionally does not stop Windows
Virtual Desktop Streamer processes. `native:xr:windows:sleep` is available for
the deliberate manual/debug path where the headset was left awake with
`-NoQuestRestore` and only needs a scripted proximity reset plus
`KEYCODE_SLEEP`.

Connected smoke package scripts now sleep the headset by default after the run.
Pass `-NoQuestRestore` only for intentional short-lived debugging or screenshot
capture; the launcher prints an explicit warning because the headset may remain
awake.

Implementation should follow the measured platform path. On macOS this likely
means Metal-specific runtime/device matching. On Windows/Linux this likely
means Vulkan instance/device negotiation. Keep backend-specific code isolated
inside the app/platform XR module.

### Slice 3 - Clear Frame Submission

- [x] Wait/begin/end OpenXR frames.
- [x] Acquire and release per-eye swapchain images.
- [x] Clear each eye to a visible diagnostic color or pattern.
- [x] Submit a projection layer using runtime-provided views/projections.
- [x] Run for a bounded number of frames in smoke mode and exit cleanly.

Landed in Slice 3:

- Added Playbox-shaped per-eye XR state for both desktop backends:
  - Windows/Linux Vulkan wraps runtime-owned `VkImage` swapchain images into
    `wgpu::Texture` values via `wgpu-hal`.
  - macOS Metal wraps runtime-owned `MTLTexture` swapchain images into
    `wgpu::Texture` values via `wgpu-hal`.
- The smoke creates left/right color swapchains and host-owned depth targets
  from the runtime-recommended stereo view size.
- The frame loop waits and begins OpenXR frames, handles non-renderable frames
  with an empty `xrEndFrame`, locates runtime stereo views in `STAGE`, clears
  the two eye targets to diagnostic colors, releases both images, and submits a
  projection layer with the runtime pose/FOV data.
- The bounded smoke requests session exit and drains session state changes.
- The process-bounded smoke intentionally keeps the XR graphics object graph
  alive after a clean app-requested `EXITING` transition. This mirrors
  Playbox's desktop runtime workaround for OpenXR runtimes that can fault while
  destroying session-owned graphics handles after shutdown. On this Windows
  VirtualDesktopXR setup, the smoke submitted frames successfully but exited
  with `STATUS_ACCESS_VIOLATION` until this Playbox teardown shape was applied.

Suggested validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 120
```

Windows validation on June 25, 2026:

```powershell
pnpm native:xr:windows:smoke:connected
```

Observed result:

- Launcher selected
  `C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json`.
- Entry: fallback loader
  `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll`.
- Runtime: `VirtualDesktopXR v1.0.10`.
- System: `Meta Quest 3`, orientation and position tracking available.
- Blend mode: `OPAQUE`.
- Stereo views:
  - eye 0 recommended `1728x1824`, max `16384x16384`, samples `1/4`
  - eye 1 recommended `1728x1824`, max `16384x16384`, samples `1/4`
- Vulkan session:
  `physical_device='NVIDIA GeForce RTX 4090' api=1.4.341 queue_family=0`.
- Reference space: `STAGE`.
- Swapchains: `Rgba8UnormSrgb`, eye size `1728x1824`, images `3/3`.
- Frame result: `submitted=2 runtime_frames=2 skipped=0`.
- Session state sequence included `IDLE`, `READY`, `SYNCHRONIZED`,
  `VISIBLE`, `FOCUSED`, then app-requested shutdown through `STOPPING`,
  `IDLE`, and `EXITING`.
- Process exit: success, with
  `desktop OpenXR clear smoke complete: frames=2`.

Visual validation:

```powershell
.\scripts\start-xr.ps1 -Frames 600 -NoQuestLaunch -NoQuestRestore
adb shell screencap -p /sdcard/mclone-xr-clear.png
adb pull /sdcard/mclone-xr-clear.png C:\tmp\mclone-xr-clear.png
adb shell rm /sdcard/mclone-xr-clear.png
```

The 600-frame run reported
`submitted=600 runtime_frames=600 skipped=0` and exited successfully. The
captured headset screenshot at `C:\tmp\mclone-xr-clear.png` showed the expected
stereo diagnostic clear: blue left eye, green right eye, with the Virtual
Desktop overlay composited above it.

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
