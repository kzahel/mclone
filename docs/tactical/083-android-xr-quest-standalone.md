# 083: Android XR / Quest Standalone

Status: active. Slice 1 package/build/install/launch plumbing is complete.
Slice 2's loader/session/swapchain/first-frame clear milestone is complete on
Quest. Slice 2A has landed shared-host chunks: desktop XR and Android XR now
share OpenXR session-state polling, frame counters, stereo config/view helpers,
STAGE reference-space setup, frame wait/begin/end helpers, swapchain image
acquire/release, diagnostic clear, stereo projection submission, controller
actions, validated view poses, and renderer-facing XR frame descriptors through
`mclone-xr-host`; desktop Vulkan and Android Vulkan now share the unsafe
OpenXR/Vulkan/wgpu graphics factory through `mclone-xr-graphics`. Slice 3 now
renders real mclone terrain on Quest through the shared `mclone-xr-scene`
terrain runtime. Continue into Quest controller locomotion before optional XR
features.

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
  and depth targets, receives Horizon `nativeOnActivityReady`, transitions
  through `XR_SESSION_STATE_READY`, submits the first clear stereo frame, and
  validates `MCLONE_ANDROID_XR_READY` on device.
- The Quest session code added so far is a bring-up spike in the Android app
  crate. It proves device/runtime facts, but it is not the architecture to keep
  extending by copying private desktop XR code.
- `native/crates/mclone-xr-host` now owns the first shared OpenXR host boundary:
  session-state event polling, READY/STOPPING begin/end transitions, poll
  status, host event reporting, frame counters, stereo view configuration,
  STAGE reference-space setup, frame wait/begin/end helpers, and stereo
  per-frame view/FOV lookup used by both desktop XR and Android XR. It also
  owns the shared swapchain-eye trait, acquired target wrapper, diagnostic
  clear pass, stereo projection-frame submission, OpenXR controller action
  set, controller snapshot contract, validated view-pose extraction, OpenXR FOV
  projection conversion, and renderer-neutral `XrRenderView` descriptors.
- `native/crates/mclone-xr-graphics` owns the shared unsafe Vulkan graphics
  bridge used by both desktop Vulkan XR and Android XR: OpenXR-driven Vulkan
  instance/device creation, wgpu wrapping, session creation, Vulkan swapchain
  format validation, and Vulkan swapchain image texture wrapping.
- `native/crates/mclone-xr-scene` owns the first shared mclone terrain XR
  runtime boundary: local integrated server/client bring-up, render-section
  compilation/upload, terrain atlas upload, sky/terrain full-frame rendering,
  startup view-pose alignment mode, and stereo `ChunkRenderView` conversion for
  Android XR. Desktop XR consumes the shared alignment type and clip-plane
  constants now; migrating the full desktop mclone terrain state onto the same
  crate remains a tightening follow-up.
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
native/crates/mclone-xr-host/
  Cargo.toml
  src/lib.rs
  src/session.rs
  src/frame_loop.rs
  src/actions.rs
  src/graphics_vulkan.rs
native/crates/mclone-xr-scene/
  Cargo.toml
  src/lib.rs
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
  extras, headset wake/restore, and Android-specific runtime readiness.
- The shared app/platform XR boundary owns reusable OpenXR host behavior:
  instance/session setup after platform bootstrap, session-state polling,
  frame wait/begin/end, reference spaces, per-eye view/FOV data, swapchain eye
  targets, diagnostic clear submission, controller action sets, and conversion
  into renderer-facing view/target facts.
- Desktop XR and Android XR consume that shared XR boundary instead of each
  keeping private copies of session loops, action wiring, and per-eye target
  logic.
- Platform-specific graphics and loader glue may stay behind platform modules
  or crate features. Android `NativeActivity`/JNI/Horizon behavior stays in the
  Android app; desktop runtime-selection/window behavior stays in the desktop
  app.
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
- [ ] Fold the local Android OpenXR host spike into the shared XR host boundary
  before extending frame, terrain, or controller behavior.
- [ ] Wait/begin/end frames and submit a stereo diagnostic clear through the
  shared XR host.
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

Resolved Slice 2 blocker:

- The `XR_SESSION_STATE_IDLE` hang was caused by a subtle Playbox parity miss:
  mclone resolved `android-activity` `0.6.1`, while Playbox pins
  `android-activity` `0.6.0`.
- On Quest, `0.6.1` let the Meta runtime create the OpenXR session but failed
  the Horizon volumetric-window association: logcat showed
  `clientDisplayId=-1`, an all-zero `clientVWToken`, no
  `nativeOnActivityReady`, and no transition past `IDLE`.
- Pinning mclone's workspace dependency to `android-activity = "=0.6.0"`
  restored Playbox-equivalent NativeActivity glue. Logcat then showed a real
  `clientVWToken`, `nativeOnActivityReady`, `IDLE -> READY`, and
  `MCLONE_ANDROID_XR_READY`.

### Slice 2A - Shared OpenXR Host Extraction

Goal: remove the architectural hazard before more Quest work lands. Desktop XR
currently lives inside the `mclone-native-client` binary app, while Android XR
has an app-local Quest session spike. The next implementation step is to
extract the reusable OpenXR host behavior into a shared app/platform XR crate
or module consumed by both apps. Do not continue by cloning
`mclone-native-client/src/xr_clear_smoke.rs` into Android.

Target boundary:

- [ ] Audit desktop `xr_clear_smoke.rs`, `xr_clear_smoke/actions.rs`, and
  desktop graphics modules for reusable host responsibilities versus
  desktop-only runtime/window behavior.
- [x] Create a shared app/platform XR boundary,
  `native/crates/mclone-xr-host`, that may depend on OpenXR and graphics
  backend crates but does not depend on Android activity/JNI or desktop window
  ownership.
- [x] Move OpenXR session-state polling and READY/STOPPING begin/end lifecycle
  transitions into the shared boundary.
- [x] Move simple shared frame counters into the shared boundary and use them
  from both desktop XR and Android XR loops.
- [x] Move frame wait/begin/end helpers, reference-space setup, stereo
  configuration validation, blend-mode selection, view-configuration
  formatting, and per-frame stereo pose/FOV lookup into the shared boundary.
- [x] Move diagnostic clear plumbing, per-eye swapchain target acquisition, and
  stereo projection-frame submission into the shared boundary.
- [x] Move renderer-facing XR frame descriptors into the shared boundary.
- [x] Move or expose the controller action set shape used by desktop XR so
  Quest Touch input can reuse the same locomotion-facing contract.
- [x] Extract shared desktop/Android Vulkan graphics-factory behavior without
  copying unsafe session/swapchain setup in each app.
- [x] Keep platform bootstrap adapters thin. Desktop loads/selects the runtime
  and owns desktop launch options. Android initializes the Khronos Android
  loader and owns activity/JNI/Horizon readiness. Each platform passes the
  prepared entry/instance requirements, graphics factory, startup pose, and
  validation markers into the shared host.
- [x] Rewire desktop XR to consume the shared boundary and keep current desktop
  terrain, controller, startup-pose, and locomotion validation passing.
- [x] Rewire Android XR to consume the same boundary and preserve the validated
  `MCLONE_ANDROID_XR_READY` Quest smoke.
- [x] Delete or shrink app-local duplicate session/frame/action code after both
  callers compile and validate.

Recorded Slice 2A first-chunk result:

- Added `native/crates/mclone-xr-host` to the workspace.
- Added shared `OpenXrPollStatus`, `OpenXrHostEvent`, `XrFrameStats`, and
  `poll_openxr_events`.
- Removed duplicate app-local OpenXR session event loops from desktop
  `mclone-native-client` and Android `mclone-android-xr-client`.
- Kept platform-specific logging in each app through an event callback so the
  shared crate does not depend on Android logging, desktop stdout policy, Java
  activity types, or desktop window ownership.
- Kept graphics creation and per-eye target management in app-local modules for
  this chunk; those remain next extraction work.

Validation after this chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- OpenXR runtime/system still probes as `Oculus v204.201.0` / `Meta Quest 3`
- Vulkan session still probes as `Adreno (TM) 740`, Vulkan API `1.3.295`,
  queue family `0`
- Swapchains still allocate per-eye `1680x1760`, `3` color images per eye
- Log marker observed: `MCLONE_ANDROID_XR_SESSION_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 2A second-chunk result:

- Added shared `PRIMARY_STEREO_VIEW_TYPE`, `XrEyeConfig`, `XrStereoConfig`,
  `XrStereoFrameViews`, blend-mode selection, view-configuration formatting,
  stereo configuration validation, STAGE reference-space creation,
  `wait_begin_frame`, `end_skipped_frame`, `end_frame_with_layers`, and
  `locate_stereo_views` to `mclone-xr-host`.
- Rewired desktop XR and Android XR to use the shared frame/reference/view
  helpers.
- Removed duplicate app-local blend-mode selection, view-configuration
  formatting, STAGE space setup, frame wait/begin/skip/end, and direct stereo
  `locate_views` calls from both apps.
- Kept swapchain image acquisition/release, clear pass encoding, projection
  layer construction, desktop mclone terrain rendering, and controller actions
  app-local for the next extraction chunk.

Validation after the second chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- OpenXR runtime/system still probes as `Oculus v204.201.0` / `Meta Quest 3`
- Vulkan session still probes as `Adreno (TM) 740`, Vulkan API `1.3.295`,
  queue family `0`
- Swapchains still allocate per-eye `1680x1760`, `3` color images per eye
- Log marker observed: `MCLONE_ANDROID_XR_SESSION_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 2A third-chunk result:

- Added `wgpu` to `mclone-xr-host`, keeping it as an app/platform XR crate
  rather than a shared engine crate.
- Added shared `XrEyeSwapchain`, `XrAcquiredEyeTarget`, `XrClearTarget`,
  swapchain image acquire/wait/release, diagnostic clear colors, diagnostic
  stereo color/depth clear, and stereo projection-frame submission.
- Implemented `XrEyeSwapchain` for desktop Vulkan, desktop Metal, and Android
  Vulkan eye-state types.
- Removed duplicate app-local swapchain acquisition/release, diagnostic clear
  colors, clear-pass encoding, and projection-layer construction from desktop
  XR and Android XR.
- Kept platform graphics factories, desktop mclone terrain rendering, and
  controller actions app-local for the next extraction chunks.

Validation after the third chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- OpenXR runtime/system still probes as `Oculus v204.201.0` / `Meta Quest 3`
- Vulkan session still probes as `Adreno (TM) 740`, Vulkan API `1.3.295`,
  queue family `0`
- Swapchains still allocate per-eye `1680x1760`, `3` color images per eye
- Log marker observed: `MCLONE_ANDROID_XR_SESSION_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 2A fourth-chunk result:

- Moved the desktop OpenXR action set into `mclone-xr-host` as
  `OpenXrControllerActions`, `XrControllerSnapshot`, and `XrHand`.
- Added shared OpenXR controller binding suggestions for simple controller,
  Oculus Touch, Valve Index, HTC Vive, and Microsoft motion controller
  profiles.
- Kept binding warning output app-owned through a callback so the shared host
  crate does not depend on desktop stdout or Android logging policy.
- Kept the desktop-only controller input summary printer app-local in
  `mclone-native-client`.
- Deleted the old private desktop `xr_clear_smoke/actions.rs` module.
- Added `glam` to `mclone-xr-host` because the shared input contract exposes
  controller positions and thumbstick axes in `Vec3`/`Vec2`.

Validation after the fourth chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo test --manifest-path native/Cargo.toml -p mclone-xr-host
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- OpenXR runtime/system still probes as `Oculus v204.201.0` / `Meta Quest 3`
- Vulkan session still probes as `Adreno (TM) 740`, Vulkan API `1.3.295`,
  queue family `0`
- Swapchains still allocate per-eye `1680x1760`, `3` color images per eye
- Log marker observed: `MCLONE_ANDROID_XR_SESSION_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 2A fifth-chunk result:

- Added shared `XrViewPose` and `XrRenderView` descriptors to
  `mclone-xr-host`.
- Moved validated OpenXR `xr::View` pose extraction, FOV-to-projection
  conversion, FOV aspect calculation, and world-pose-to-render-view descriptor
  construction out of desktop `xr_clear_smoke.rs`.
- Kept desktop app-specific XR rig alignment in `mclone-native-client`, then
  adapted the shared renderer-neutral descriptor into
  `mclone_render::chunk::ChunkRenderView`.
- Added focused `mclone-xr-host` tests for the projection convention and camera
  vectors.
- Preserved the Quest READY path while moving this render-view math into the
  shared host crate.

Validation after the fifth chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- APK install succeeded for
  `android-xr/app/build/outputs/apk/debug/app-debug.apk`
- Startup argv intent extra:
  `["--seed","12345","--chunk-x","0","--chunk-z","0","--render-distance","2","--day-time","6000","--freeze-time"]`
- Launch activity:
  `com.kzahel.mclone.xr/com.kzahel.mclone.xr.McloneXrActivity`
- Log marker observed by validator: `MCLONE_ANDROID_XR_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 2A sixth-chunk result:

- Added `native/crates/mclone-xr-graphics` as the unsafe backend crate for
  shared XR graphics factories.
- Moved the duplicated desktop Vulkan / Android Vulkan OpenXR graphics
  implementation into `mclone-xr-graphics::vulkan`: runtime Vulkan version
  checks, OpenXR-created Vulkan instance/device, physical-device and queue
  selection, wgpu-hal wrapping, OpenXR session creation, swapchain format
  validation, and Vulkan swapchain image texture wrapping.
- Replaced desktop and Android Vulkan modules with thin adapters that choose
  labels and own app-specific depth target shape only.
- Removed app-level direct `ash` dependencies from the desktop and Android XR
  app crates; `ash` is now owned by the shared graphics crate.
- Kept desktop Metal local because it has no Android counterpart in this
  workstream.

Validation after the sixth chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-xr-graphics
cargo test --manifest-path native/Cargo.toml -p mclone-xr-graphics
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo test --manifest-path native/Cargo.toml -p mclone-xr-host
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- APK install succeeded for
  `android-xr/app/build/outputs/apk/debug/app-debug.apk`
- Startup argv intent extra:
  `["--seed","12345","--chunk-x","0","--chunk-z","0","--render-distance","2","--day-time","6000","--freeze-time"]`
- Launch activity:
  `com.kzahel.mclone.xr/com.kzahel.mclone.xr.McloneXrActivity`
- Log marker observed by validator: `MCLONE_ANDROID_XR_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

### Slice 2B - Quest READY / First Clear Frame

Goal: resolve the Horizon runtime readiness blocker without copying Playbox
application features into mclone.

Recorded Slice 2B result:

- Compared mclone and Playbox Android XR manifests, Gradle package metadata,
  Java `NativeActivity` subclasses, launch commands, and same-headset logcat.
- Verified the source manifests and merged manifests were equivalent except
  expected package, activity, label, and library names.
- Reproduced Playbox passing on the same Quest 3 and isolated the key runtime
  difference: Playbox's OpenXR client registered with `clientDisplayId=0` and
  a real Horizon volumetric-window token, while mclone registered with
  `clientDisplayId=-1` and an all-zero token.
- Ruled out launch-intent extras and a UI-thread Java-to-native readiness hook.
  The default launch and startup-argv launch both failed before the dependency
  pin and both passed after it.
- Pinned `native/Cargo.toml` to Playbox's exact
  `android-activity = "=0.6.0"` NativeActivity glue and updated
  `native/Cargo.lock`.

Validation after Slice 2B, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo tree --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --wait-seconds 45'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
```

Observed Quest result with startup argv:

- Quest serial: `2G0YC1ZF93041Z`
- Runtime/system: `Oculus v204.201.0` / `Meta Quest 3`
- Runtime client: `clientDisplayId=0` and nonzero
  `clientVWToken=8621cbbc-456f-4d8f-9650-696089251c5b`
- Horizon callback: `nativeOnActivityReady:
  com.kzahel.mclone.xr/com.kzahel.mclone.xr.McloneXrActivity`
- Session states observed: `IDLE`, `READY`, `SYNCHRONIZED`, `VISIBLE`,
  `FOCUSED`
- Vulkan session: `Adreno (TM) 740`, Vulkan API `1.3.295`, queue family `0`
- Swapchains: per-eye `1680x1760`, `3` color images per eye
- Log markers observed: `MCLONE_ANDROID_XR_SESSION_READY`,
  `MCLONE_ANDROID_XR_READY`
- First frame: `OpenXR clear frame submitted: submitted=1 runtime_frames=1
  skipped=0`
- Logcat: `/tmp/mclone-quest-openxr-aa060-startup-logcat.txt`

Exit criteria:

- Desktop XR and Android XR share one OpenXR session/frame/action host path.
- App-local code is limited to platform bootstrap, launch/config plumbing,
  graphics factory selection, validation markers, and app lifecycle concerns.
- The shared boundary exposes renderer-facing view/target data rather than
  leaking platform activity/window concepts into shared engine crates.
- No new Quest terrain or controller feature is added on top of duplicated
  Android-only copies of desktop XR logic.

### Slice 3 - Mclone Runtime Frame On Quest

- [x] Stage `reference/minecraft-1.17.1/extracted.zip` into the Android XR app
  external files directory from the install/validate scripts.
- [x] Load the staged asset pack from the Android XR app external files
  directory in the mclone runtime path.
- [x] Reuse the shared XR host plus the desktop-proven mclone-frame path for
  integrated server/client, render-section sync, texture atlas, sky, and terrain
  resources.
- [ ] Add Quest actor resources/rendering after terrain and locomotion are
  stable.
- [x] Convert Quest runtime eye poses/FOV into `ChunkRenderView` values.
- [x] Render a small-radius real mclone scene per eye.
- [x] Validate headset-visible terrain and log render-section/drawn-index
  diagnostics.

Recorded Slice 3 first-chunk result:

- Added default asset-pack staging to `android-xr/install-quest-openxr.sh` and
  `android-xr/validate-quest-openxr.sh`.
- The XR scripts now stage
  `reference/minecraft-1.17.1/extracted.zip` to
  `/sdcard/Android/data/com.kzahel.mclone.xr/files/assets/packs/extracted.zip`
  unless `--skip-assets` is passed.
- Added `--asset-pack PATH` to install and validate flows so alternate local
  packs can be staged without changing environment variables.
- Reused the flat Android staging helper while passing the XR package id, so
  the pack lands under `com.kzahel.mclone.xr` rather than the flat Android app.
- Kept runtime terrain loading open for the next chunk; this only proves the
  Quest package has the pack available where `MCLONE_ANDROID_ASSET_ROOT`
  already points.

Validation after the first Slice 3 chunk, June 26, 2026:

```bash
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash -n android-xr/install-quest-openxr.sh android-xr/validate-quest-openxr.sh'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 45'
"C:\Program Files\Git\bin\bash.exe" -lc 'export MSYS2_ARG_CONV_EXCL=/sdcard; adb -s 2G0YC1ZF93041Z shell ls -l /sdcard/Android/data/com.kzahel.mclone.xr/files/assets/packs/extracted.zip'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- Staged local asset pack:
  `/c/Users/sox/Documents/code/mclone/reference/minecraft-1.17.1/extracted.zip`
- Device asset path:
  `/sdcard/Android/data/com.kzahel.mclone.xr/files/assets/packs/extracted.zip`
- Device file size: `5828345`
- Log marker observed by validator: `MCLONE_ANDROID_XR_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 3 second-chunk result:

- Added `mclone-app-runtime` to the Android XR app dependencies.
- Android XR now loads the staged pack through `load_asset_source`,
  `load_textured_mesh_assets_from_source`, and
  `load_actor_texture_assets_from_asset_source` before OpenXR startup.
- The loaded terrain and actor texture assets are retained in
  `AndroidXrRuntimeAssets`, so the next terrain chunk can consume them instead
  of reloading the pack.
- Added the `MCLONE_ANDROID_XR_ASSETS_READY` marker and made
  `android-xr/validate-quest-openxr.sh` require it in addition to
  `MCLONE_ANDROID_XR_READY`.

Validation after the second Slice 3 chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash -n android-xr/validate-quest-openxr.sh'
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 60'
"C:\Program Files\Git\bin\bash.exe" -lc 'grep -E "loaded Minecraft asset pack|Android XR runtime assets|MCLONE_ANDROID_XR_ASSETS_READY|MCLONE_ANDROID_XR_READY|MCLONE_ANDROID_XR_FAILURE" /tmp/mclone-quest-openxr-logcat.txt | head -20'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- Asset pack loaded by app:
  `/storage/emulated/0/Android/data/com.kzahel.mclone.xr/files/assets/packs/extracted.zip`
- Asset pack manifest: `asset_set=mclone-vanilla-1.17.1`, `files=6982`
- Runtime asset facts: `terrain_atlas=512x2048`, `actor_atlas=65x32`
- Runtime asset load time in debug build: `92.602 ms`
- Log markers observed: `MCLONE_ANDROID_XR_ASSETS_READY`,
  `MCLONE_ANDROID_XR_READY`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

Recorded Slice 3 third-chunk result:

- Added `native/crates/mclone-xr-scene` as the shared mclone terrain XR runtime
  crate. It depends on OpenXR/`wgpu` as an app/platform XR boundary and stays
  free of Android activity/JNI and desktop window ownership.
- Android XR now parses the launch-scoped `mclone.startup.argv` for `--seed`,
  `--chunk-x`, `--chunk-z`, `--render-distance`, `--day-time`, and
  `--freeze-time`; `debug.mclone.xr_view_pose` is parsed into the shared
  startup view-pose shape.
- Android XR constructs `XrMcloneTerrainState` after OpenXR Vulkan device and
  swapchain creation, using the staged terrain asset pack, local integrated
  server/client runtime, render-section compile worker, uploaded terrain atlas,
  `SkyRenderer`, and `TexturedSectionDrawResources`.
- Android XR eye depth targets now use `mclone_render::chunk::ChunkDepthTarget`,
  matching the shared full-frame renderer contract.
- The old Android diagnostic clear frame loop now renders real mclone terrain
  into each acquired OpenXR eye target before stereo projection submission.
- Added `MCLONE_ANDROID_XR_TERRAIN_READY`; the Quest validator now requires it
  for full submitted-frame validation.
- Desktop XR consumes `mclone-xr-scene` for shared XR alignment mode and clip
  planes. Full desktop terrain-state migration is intentionally left as a
  follow-up because the current desktop path still includes controller
  locomotion, remote-session support, and actor rendering.

Validation after the third Slice 3 chunk, June 26, 2026:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug'
"C:\Program Files\Git\bin\bash.exe" -lc 'cd /c/Users/sox/Documents/code/mclone && bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 90'
"C:\Program Files\Git\bin\bash.exe" -lc 'grep -E "Android XR scene options|mclone XR terrain runtime|MCLONE_ANDROID_XR_TERRAIN_READY|Android XR terrain first-frame summary|OpenXR mclone terrain frame submitted|MCLONE_ANDROID_XR_READY|MCLONE_ANDROID_XR_FAILURE" /tmp/mclone-quest-openxr-logcat.txt | head -40'
```

Observed Quest result:

- Quest serial: `2G0YC1ZF93041Z`
- Runtime scene options: `seed=12345`, center `(0, 0)`, render distance `2`,
  `day_time=Some(6000)`, `freeze_time=true`
- Terrain runtime: `chunks=49`, `sections=122`, `faces=96699`,
  `indices=580194`
- Session-ready marker is emitted before terrain warmup:
  `MCLONE_ANDROID_XR_SESSION_READY`
- Initial debug-build terrain warmup: `initial_polls=10244`,
  `poll_ms=476.192`, `elapsed_ms=17848.920`
- Terrain-ready marker: `MCLONE_ANDROID_XR_TERRAIN_READY sections=122
  indices=580194`
- First-eye frame summary: `frames=1`, `drawn_sections=27`,
  `drawn_indices=236490`
- Submitted-frame marker: `MCLONE_ANDROID_XR_READY`
- First submitted frame: `OpenXR mclone terrain frame submitted: submitted=1
  runtime_frames=1 skipped=0`
- Logcat: `/tmp/mclone-quest-openxr-logcat.txt`

### Slice 4 - Controller Actions And Locomotion

- [ ] Reuse the shared XR host action set shape for Quest Touch controllers.
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
- Do not continue Quest implementation by copying desktop XR private modules
  into `mclone-android-xr-client`. The app-local Android OpenXR code from the
  session smoke is temporary bring-up evidence and must be folded into the
  shared app/platform XR boundary before terrain or controller work advances.
- A dedicated app/platform XR crate may depend on OpenXR, `wgpu`, `wgpu-hal`,
  and graphics backend crates behind platform features. It must not own Android
  activity/JNI or desktop window/event-loop concerns.
- Do not fork the mclone renderer or render-section cache for Quest.
- Do not add hand tracking, passthrough, spatial scene, render models, or
  in-world UI until the basic mclone frame and locomotion are validated.
- Keep screenshots and log captures under `/tmp`.

## Validation Lanes

Current terrain-frame validation:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash android-xr/build-apk.sh --debug
bash android-xr/validate-quest-openxr.sh --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --wait-seconds 90
```

Historical Slice 2 session-only milestone:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash android-xr/build-apk.sh --debug
bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180
```

Shared XR host extraction gates:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
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
