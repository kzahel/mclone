# 083: Android XR / Quest Standalone

Status: active. Slice 1 package/build/install/launch plumbing is complete.
Slice 2's loader/session/swapchain first chunk is complete; first submitted
stereo frame is next.

## Purpose

Bring up a standalone Quest OpenXR target for mclone now that desktop XR has
validated real mclone terrain, stereo rendering, controller actions, and basic
locomotion.

This is not a flat Android extension. Android XR owns a separate package,
manifest, OpenXR Android loader initialization, Quest launch scripts, and
validation lane while reusing the same shared runtime, asset, renderer,
render-section, controller/action, and XR rig shape proven by desktop XR.

## Current State

Ready inputs:

- Flat Android has a `NativeActivity` APK, staged packed assets, Vulkan-backed
  `wgpu`, AVD screenshot/logcat smokes, and touch-orbit validation.
- Desktop OpenXR can render real mclone terrain through shared runtime/render
  code.
- Desktop OpenXR startup view pose, controller action polling, and player
  locomotion are implemented.
- Manual desktop XR headset validation confirmed movement feels good.
- A Quest headset is attached for standalone validation.
- The standalone Quest package now initializes the Android OpenXR loader,
  creates a Vulkan-backed OpenXR session, allocates per-eye color swapchains
  and depth targets, and validates `MCLONE_ANDROID_XR_SESSION_READY` on device.
- Playbox has the mature reference implementation for Android XR packaging,
  loader/session ownership, launch-scoped startup arguments, Android property
  toggles, and Quest validation scripts.

## Playbox References

Use `C:\Users\sox\Documents\code\playbox` as a pattern library only:

- `android-xr/README.md`: build/install/validate command surface, Quest launch
  workflow, `playbox.startup.argv` intent extra, and property-based wrapper
  settings.
- `android-xr/build-apk.sh`: release/debug `cargo ndk` build, shared `android`
  Gradle wrapper reuse, `libc++_shared.so` bundling.
- `android-xr/install-quest-openxr.sh`: build/install/optional-launch flow,
  launch-scoped argv JSON, startup property setup, and printed `adb shell am
  start` command.
- `android-xr/validate-quest-openxr.sh`: Quest wake/restore, logcat streaming,
  ready/failure marker checks, and cleanup on every exit path.
- `android-xr/startup-properties.sh`: centralized Android property names,
  reset defaults, JSON argv serialization, and shell quoting helpers.
- `android-xr/app/src/main/AndroidManifest.xml`: VR activity category,
  supported devices/input metadata, OpenXR runtime broker queries, and
  `android.app.lib_name`.
- `android-xr/app/src/main/java/com/playbox/android/xr/PlayboxXrActivity.java`:
  Java `NativeActivity` subclass that keeps the Rust `android_main` entry while
  exposing launch intent extras through JNI.
- `src/android_xr.rs`: Android logger, app data path asset root setup, Android
  system property reads, launch intent JSON parsing, and startup option logging.
- `src/xr/mod.rs` and `src/xr/graphics_vulkan.rs`: Android-compatible OpenXR
  Vulkan session/swapchain shape after the package/launch slice is proven.

Do not copy Playbox's PhysX, DAZ, VaM, egui panel, passthrough, spatial-room,
render-model, hand-tracking, or overlay-keyboard product shape unless a later
mclone tactical explicitly asks for that capability.

## Target Shape

New package shape:

```text
native/apps/mclone-android-xr-client/
  Cargo.toml
  src/lib.rs
android-xr/
  build-apk.sh
  install-quest-openxr.sh
  validate-quest-openxr.sh
  startup-properties.sh
  README.md
  app/build.gradle.kts
  app/src/main/AndroidManifest.xml
  app/src/main/java/com/kzahel/mclone/xr/McloneXrActivity.java
```

Ownership:

- Android XR app/platform code owns Java activity glue, Android OpenXR loader
  setup, Quest manifest metadata, app data paths, system properties, launch
  extras, headset wake/restore, and OpenXR session/swapchains.
- Shared engine crates stay free of Android activity, JNI, and OpenXR types.
- The desktop XR implementation remains the behavior reference for per-eye
  mclone rendering, controller actions, startup view pose, and locomotion.
- The flat Android package remains non-XR.

## Startup Configuration

Use Playbox's split:

- launch-scoped JSON argv extra for engine/session options:
  `mclone.startup.argv`
- Android debug properties for wrapper/runtime settings that are not promoted
  to shared argv yet:
  - `debug.mclone.xr_view_pose`
  - future: `debug.mclone.xr_display_refresh`
  - future: `debug.mclone.xr_foveation`
  - future: `debug.mclone.asset_mode`

The first scripts should already preserve this shape even if the Rust host only
logs the values until the OpenXR scene loop consumes them.

## Implementation Slices

### Slice 1 - Quest Package, Build, Install, Launch Plumbing

Goal: prove the standalone Quest package can build, install, launch as a VR
activity, receive launch parameters, write Android log markers, and be
validated/recovered by scripts.

- [x] Add `mclone-android-xr-client` as an Android `cdylib` app crate.
- [x] Add an `android-xr/` Gradle package adapted from Playbox, using
  `org.khronos.openxr:openxr_loader_for_android`.
- [x] Add a Java `McloneXrActivity` subclass of `NativeActivity` that loads the
  Rust library and exposes `mclone.startup.argv` to Rust through JNI.
- [x] Add Android logger, app-data-path asset-root setup, startup argv read,
  and first marker logging in Rust.
- [x] Add build/install/validate scripts with release/debug modes, Quest
  detection, wake/restore, launch-scoped startup argv, view-pose property, and
  ready/failure logcat scanning.
- [x] Add package scripts for build/install/validate.
- [x] Validate on the attached Quest.

This slice intentionally does not create an OpenXR session yet. The success
marker must be named so it cannot be mistaken for a rendered XR frame.

Recorded Slice 1 result:

- Added `native/apps/mclone-android-xr-client` as a small Android-only
  `cdylib` host crate.
- Added `android-xr/` with a Quest VR activity manifest, Khronos Android
  OpenXR loader dependency, release/debug APK build script, install script,
  validation script, startup-property helpers, and workflow README.
- Added `com.kzahel.mclone.xr.McloneXrActivity`, modeled after Playbox's Java
  activity shape but without the Playbox keyboard bridge. It keeps the Rust
  `android_main` entry and exposes `mclone.startup.argv` through JNI.
- Added Rust Android logging, `MCLONE_ANDROID_ASSET_ROOT` setup from app data
  paths, JNI startup argv read, `debug.mclone.xr_view_pose` property read, and
  `MCLONE_ANDROID_XR_PACKAGE_READY` package-slice marker.
- Added package scripts:
  - `pnpm native:android-xr:apk`
  - `pnpm native:android-xr:install`
  - `pnpm native:android-xr:validate`

Validation on the attached Quest 3, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
bash -n android-xr/build-apk.sh android-xr/install-quest-openxr.sh android-xr/validate-quest-openxr.sh android-xr/startup-properties.sh
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 30'
```

Observed validation result:

- Quest serial: `2G0YC1ZF93041Z`
- Device: `Oculus Quest 3`, API `34`, ABI `arm64-v8a`
- APK install: success
- Launch component:
  `com.kzahel.mclone.xr/com.kzahel.mclone.xr.McloneXrActivity`
- Startup argv extra:
  `["--seed","12345","--chunk-x","0","--chunk-z","0","--render-distance","2","--day-time","6000","--freeze-time"]`
- Startup view-pose property:
  `debug.mclone.xr_view_pose=0,120,-96,180`
- Log marker observed:
  `MCLONE_ANDROID_XR_PACKAGE_READY`
- Logcat:
  `/tmp/mclone-quest-openxr-logcat.txt`
- Validator restored Quest wake/proximity settings and force-stopped the app on
  exit.

Windows note: use Git Bash explicitly for Android scripts on this machine, for
example `"C:\Program Files\Git\bin\bash.exe" -lc '...'`. The default `bash`
resolved to WSL during validation and did not see the Windows Rust toolchain.

### Slice 2 - Android OpenXR Loader And Clear Submission

- [x] Initialize Khronos' Android OpenXR loader from the Android activity.
- [x] Create an OpenXR instance/system/session with Vulkan graphics binding.
- [x] Create one color swapchain and one depth target per eye.
- [x] Log `MCLONE_ANDROID_XR_SESSION_READY` after session/swapchain bring-up.
- [ ] Wait/begin/end frames and submit a stereo diagnostic clear.
- [ ] Log `MCLONE_ANDROID_XR_READY` after the first submitted stereo frame.
- [ ] Keep bounded validation and restore headset state on every exit path.

Recorded Slice 2 first-chunk result:

- Added Android OpenXR loader initialization using Khronos'
  `XR_KHR_android_create_instance` path from the `NativeActivity`.
- Added a narrowed Playbox-shaped OpenXR Vulkan graphics module for Quest:
  runtime-owned Vulkan instance/device creation, queue selection, OpenXR session
  graphics binding, and `wgpu-hal` device/queue wrapping.
- Added `STAGE` reference-space creation and per-eye color swapchains with
  matching depth targets.
- Added Playbox-aligned Quest runtime declarations to the Android XR manifest
  for passthrough, controller/hand input metadata, hand/body tracking, render
  models, and spatial scene/anchor permissions. These are launch/runtime
  compatibility declarations only; mclone does not use those product features
  yet.
- Added runtime permission grants and a VR system-dialog dismissal helper in
  the Quest validation/install scripts.
- Added `--session-only` to `android-xr/validate-quest-openxr.sh` so this
  milestone can accept `MCLONE_ANDROID_XR_SESSION_READY` while the default lane
  continues to require first-frame `MCLONE_ANDROID_XR_READY`.

Validation on the attached Quest 3, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash -n android/validate-common.sh android-xr/build-apk.sh android-xr/install-quest-openxr.sh android-xr/validate-quest-openxr.sh android-xr/startup-properties.sh'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed validation result:

- Quest serial: `2G0YC1ZF93041Z`
- Device: `Oculus Quest 3`, API `34`, ABI `arm64-v8a`
- OpenXR runtime: `Oculus v204.201.0`
- OpenXR system: `Meta Quest 3`
- Vulkan session: `Adreno (TM) 740`, Vulkan API `1.3.295`, queue family `0`
- Reference space: `STAGE`
- Swapchains: per-eye `1680x1760`, `3` color images per eye, plus depth target
  per eye
- Log marker observed: `MCLONE_ANDROID_XR_SESSION_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Known remaining blocker for completing Slice 2:

- Mclone reaches `XR_SESSION_STATE_IDLE` but does not yet receive the runtime
  `nativeOnActivityReady` callback or transition to `XR_SESSION_STATE_READY`.
- Playbox reaches `nativeOnActivityReady`, transitions from `IDLE` to `READY`,
  and logs its first-frame marker on the same headset.
- Next investigation target: isolate the Java `NativeActivity`, manifest, and
  Horizon launch/readiness delta between Playbox and mclone. Once READY arrives,
  the existing clear-loop path should be able to submit the first stereo
  diagnostic frame and log `MCLONE_ANDROID_XR_READY`.

### Slice 3 - Mclone Runtime Frame On Quest

- [ ] Stage/load `reference/minecraft-1.17.1/extracted.zip` from app external
  files.
- [ ] Reuse the desktop XR mclone-frame path for integrated server/client,
  render-section sync, texture atlas, sky, terrain, and actor resources.
- [ ] Convert Quest runtime eye poses/FOV into `ChunkRenderView` values.
- [ ] Render a small-radius real mclone scene per eye.
- [ ] Validate headset-visible terrain and log render-section/drawn-index
  diagnostics.

### Slice 4 - Controller Actions And Locomotion

- [ ] Port/reuse the desktop XR action set shape for Quest Touch controllers.
- [ ] Feed left-stick/right-stick/A-button into the shared locomotion path.
- [ ] Validate movement, yaw, and jump on-device with awake controllers.

### Slice 5 - Quest Runtime Hardening

- [ ] Add display-refresh and foveation toggles if the runtime exposes them.
- [ ] Add GPU/frame timing diagnostics.
- [ ] Tune default render distance and startup warmup for Quest.
- [ ] Add headset screenshot/log capture notes for repeatable visual evidence.

## Guardrails

- Do not put Android XR manifest/category/OpenXR behavior into the flat Android
  package.
- Do not add OpenXR, JNI, or Android activity types to shared client, server,
  protocol, mesh, worldgen, light, render-session, or app-runtime crates.
- Do not fork the mclone renderer or render-section cache for Quest.
- Do not add hand tracking, passthrough, spatial scene, render models, or
  in-world UI until the basic mclone frame and locomotion are validated.
- Keep screenshots and log captures under `/tmp`.

## Validation Lanes

Future first-frame validation:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash android-xr/build-apk.sh --debug
bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180
```

Current Slice 2 session milestone:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash android-xr/build-apk.sh --debug
bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180
```

Shared regression gates after touching common XR/render code:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
pnpm native:web:build
```

## Completion Criteria

This tactical is complete when Android XR / Quest standalone can launch through
its own package, create an OpenXR session on Quest, render real mclone terrain
in stereo, and accept basic controller locomotion through the shared XR/player
path.
