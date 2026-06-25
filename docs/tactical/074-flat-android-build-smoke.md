# 074: Flat Android Build Smoke

Status: proposed high-priority platform slice; Slice 0 baseline/dependency probe
completed.

Prerequisite: land [`075-shared-single-view-runtime-prereq.md`](075-shared-single-view-runtime-prereq.md)
before starting Android runtime integration. Android should consume
`mclone-app-runtime` instead of copying the desktop or native-web shell.

## Purpose

Bring up a flat, non-XR Android build for the native Rust engine. The first
success criterion is an installable APK that launches a `NativeActivity`, uses a
Vulkan-backed `wgpu` surface, renders a nonblank mclone frame, and passes an
ADB screenshot/logcat smoke.

This is a platform frontload slice. It should reveal Android packaging,
lifecycle, surface, asset, and input constraints while the native app boundary
is still easy to adjust.

## Direction

Flat Android is the next platform target. It is a single-view host, not XR.

Use Playbox as the reference engine:

- `~/code/playbox/Cargo.toml`: `cdylib`, `winit` Android feature, Android
  target dependencies, debug optimization policy.
- `~/code/playbox/src/android.rs`: `android_main`, Android `winit` event loop,
  Vulkan `wgpu` surface/device setup, resume/suspend lifecycle, resize, redraw,
  and touch input.
- `~/code/playbox/android/`: Gradle project, manifest, `cargo ndk` APK build,
  AVD validator, Quest-flat validator, screenshot capture, and logcat fatal
  scanning.
- `~/code/playbox/src/core.rs`: single-view runtime pattern. Copy the idea, not
  Playbox's PhysX/VaM runtime shape.

The durable platform architecture is [`../platforms.md`](../platforms.md).

## Current Mclone Status

Landed:

- Native desktop window/headless renderer and full-frame composition.
- Native web/WASM app with WebGPU canvas rendering, workers, and mobile web
  controls.
- Dedicated native server.
- Explicit renderer frame target/context types in `mclone-render`.
- Shared render-session and render-section compile policy used by desktop and
  native web.

Missing:

- No `native/apps/mclone-android-client` crate.
- No Android `cdylib` target.
- No `android/` Gradle project, manifest, APK build script, or validation
  scripts.
- No Android asset-pack staging policy.
- No Android lifecycle/input adapter.

Resolved Slice 0 compile blocker:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --target aarch64-linux-android
```

Before Slice 0, this failed before mclone app code because `android-activity`
was selected through `winit`, but neither `native-activity` nor
`game-activity` was enabled. Matching Playbox's dependency shape, the workspace
`winit` dependency now enables `android-native-activity`; the command above
passes.

Next blocker: there is still no Android `cdylib` host crate or Gradle package
shell. Start Slice 1 with a thin `mclone-android-client` crate and Playbox-style
`android/` project.

## Non-goals

- Do not add OpenXR, Quest VR manifest categories, controller actions, hand
  tracking, passthrough, foveation, or XR swapchains.
- Do not fork renderer, client, server, mesh, asset, or protocol contracts for
  Android.
- Do not port Playbox's PhysX, VaM, egui, audio, or scene runtime shape.
- Do not make Android the primary engine development loop.
- Do not require a Quest headset for the baseline smoke; AVD should be enough.
- Do not solve polished mobile controls in the first clear/chunk smoke.

## Target Shape

Preferred app/package shape:

```text
native/apps/mclone-android-client/
  Cargo.toml              # cdylib Android host crate
  src/lib.rs              # exports android_main
android/
  app/src/main/AndroidManifest.xml
  app/build.gradle.kts
  build.gradle.kts
  settings.gradle.kts
  gradle.properties
  build-apk.sh
  build-common.sh
  validate-common.sh
  validate-avd.sh
  validate-quest-flat.sh
```

The Android host can start as a thin crate, but the playable path should consume
the shared runtime shell landed in
[`075-shared-single-view-runtime-prereq.md`](075-shared-single-view-runtime-prereq.md)
instead of copying desktop or native-web runtime orchestration.

Expected ownership boundary:

```text
mclone-app-runtime
  - ClientRuntime / EngineRenderSession orchestration
  - chunk-view state and host-exchange application
  - render-section cache synchronization through RenderSectionCompiler
  - platform-neutral world/time/query helpers

platform adapters
  - desktop winit keyboard/mouse/window/frame pacing/assets/full-frame capture
  - Android NativeActivity lifecycle/touch/surface/package paths
  - browser canvas/DOM/worker/storage/JsValue glue
```

Temporary Android-local code is acceptable only for the first clear or static
chunk smoke.

## Implementation Slices

### Slice 0 - Baseline And Dependency Probe

Goal: make the current Android state explicit and fix the shallow dependency
gate without creating inert app scaffolding.

- [x] Confirm workstream: native Rust platform.
- [x] Run and record:
  - `rustup target list --installed`
  - `which cargo-ndk`
  - Android SDK/NDK discovery under `$ANDROID_HOME`, `$ANDROID_SDK_ROOT`, or
    `~/Android/Sdk`
  - `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --target aarch64-linux-android`
- [x] Enable Android NativeActivity support in the host dependency path:
  - `winit = { version = "0.30", features = ["android-native-activity"] }`
    or the workspace-equivalent target-specific shape.
  - Add target Android dependencies only where needed.
- [x] Re-run an Android target check and record the next real blocker.

Recorded local probe:

- Installed Android Rust targets include `aarch64-linux-android`,
  `armv7-linux-androideabi`, `i686-linux-android`, and
  `x86_64-linux-android`.
- `cargo-ndk`: `/Users/kgraehl/.cargo/bin/cargo-ndk`, version `4.1.2`.
- SDK: `$ANDROID_HOME=/Users/kgraehl/Android/Sdk`.
- NDK: `$ANDROID_NDK_HOME=/Users/kgraehl/Android/Sdk/ndk/27.0.12077973`;
  SDK also has NDK `28.2.13676358`.
- SDK platforms present: `android-34`, `android-35`, `android-36`.
- Java: OpenJDK `17.0.18`.
- `adb`: `/Users/kgraehl/Android/Sdk/platform-tools/adb`, version `36.0.2`.
- `sdkmanager`: `/Users/kgraehl/Android/Sdk/cmdline-tools/latest/bin/sdkmanager`.
- First target check reproduced the `android-activity` feature error.
- After enabling `winit/android-native-activity`, the target check passed.

Validation:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --target aarch64-linux-android
```

### Slice 1 - Android Host Crate And APK Skeleton

Goal: build an APK that launches native Rust code, even if it only clears the
screen.

- [ ] Add `native/apps/mclone-android-client` as a `cdylib` Android host crate.
- [ ] Export `android_main(app: AndroidApp)` from the crate.
- [ ] Build the Android `winit` event loop with `EventLoopBuilderExtAndroid`.
- [ ] Add an `android/` Gradle project adapted from Playbox:
  - application id such as `com.kzahel.mclone`
  - app label `Mclone`
  - `android.app.NativeActivity`
  - `android.app.lib_name` matching the Rust shared library
  - Vulkan level feature
  - no XR or Quest VR categories
- [ ] Add `android/build-apk.sh` and `android/build-common.sh` adapted from
  Playbox, including SDK/NDK/cargo-ndk preflight and `libc++_shared.so`
  bundling.
- [ ] Add a package script if useful, for example `pnpm native:android:apk`.

Validation:

```bash
bash android/build-apk.sh
```

### Slice 2 - Clear Frame AVD Smoke

Goal: prove Android lifecycle, surface acquisition, Vulkan `wgpu`, present, and
validation scripts before loading the full mclone runtime.

- [ ] Create an Android GPU state adapted from Playbox:
  - `wgpu::Backends::VULKAN`
  - preferred sRGB surface format when available
  - `PresentMode::Fifo`
  - explicit resize/reconfigure
  - surface-lost/outdated/skipped handling
- [ ] Render a clear frame or simple GUI/title frame on redraw.
- [ ] Drop window/GPU state on `suspended`.
- [ ] Add `android/validate-common.sh` adapted from Playbox:
  - find `adb`
  - wait for boot
  - install APK
  - launch activity
  - verify process/focus/resumed state
  - collect logcat
  - fail on fatal exception, fatal signal, Rust panic, or segfault
  - capture screenshot to `/tmp`
- [ ] Add `android/validate-avd.sh`.
- [ ] Inspect the captured screenshot before proceeding.

Validation:

```bash
bash android/validate-avd.sh --screenshot /tmp/mclone-android-avd-clear.png
```

### Slice 3 - Static Chunk Or Full-Frame Render Smoke

Goal: render mclone pixels through the Android surface, still with the smallest
runtime footprint that proves renderer compatibility.

Two acceptable paths:

- Static chunk path: load packed assets, build one deterministic generated
  chunk/section set, and render it through existing chunk draw resources.
- Full-frame path: reuse/extract `render_full_frame` enough to render sky,
  chunks, GUI, and debug-free frame composition.

Requirements:

- [ ] Use existing `mclone-render` target/view types; do not create Android-only
  renderer APIs.
- [ ] Own depth target in the Android host and recreate it on resize.
- [ ] Save validation screenshots to `/tmp`, not the repo.
- [ ] Keep desktop headless and native web checks green after any shared
  extraction.

Validation:

```bash
bash android/validate-avd.sh --screenshot /tmp/mclone-android-avd-chunk.png
cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-render-session
```

Inspect `/tmp/mclone-android-avd-chunk.png`.

### Slice 4 - Integrated Runtime And Asset Pack

Goal: run the same client/server/render-session path as desktop, with Android
owning only platform lifecycle, input, surface, and package paths.

- [ ] Consume `mclone-app-runtime` for shared single-view runtime state,
  update exchange application, world queries, and render-section streaming.
- [ ] Load packed Minecraft assets from Android app files or a staged external
  files directory.
- [ ] Add an Android asset-pack staging/install helper, likely copying the
  current reference pack into:

```text
/sdcard/Android/data/com.kzahel.mclone/files/assets/packs/
```

- [ ] Use `WindowSceneRuntime` or its extracted shared successor for local
  integrated server/client replica/render-section synchronization.
- [ ] Render a real full frame with sky, terrain, and GUI/title state.
- [ ] Add minimal touch handling:
  - one-finger look or orbit for smoke
  - optional simple move/look controls only if cheap to wire
- [ ] Do not implement polished mobile controls here if it blocks platform
  bring-up.

Validation:

```bash
bash android/validate-avd.sh --screenshot /tmp/mclone-android-avd-runtime.png
pnpm native:movement:smoke
pnpm native:web:build
```

Inspect `/tmp/mclone-android-avd-runtime.png`.

### Slice 5 - Quest-Flat Optional Smoke

Goal: validate the flat Android APK as a 2D panel on Quest without introducing
OpenXR.

- [ ] Add `android/validate-quest-flat.sh` adapted from Playbox.
- [ ] Detect attached Quest-like devices.
- [ ] Wake headset for test and restore headset settings afterward.
- [ ] Launch the same flat `NativeActivity`, not a VR activity.
- [ ] Capture `/tmp/mclone-quest-flat.png` and logcat.

Validation:

```bash
bash android/validate-quest-flat.sh --screenshot /tmp/mclone-quest-flat.png
```

## Expected Risks

- Android `winit`/`android-activity` feature resolution is the first known
  blocker.
- `mclone-native-client` is currently a binary app with reusable modules hidden
  inside it; sharing with Android may require a small library/extraction slice.
- Android asset lookup cannot use repo-relative paths.
- AVD Vulkan/WebGPU behavior may vary by emulator image and host GPU.
- Android lifecycle teardown needs to be real; keeping stale `wgpu` surface
  state across suspend/resume will cause flaky validation.
- Desktop keyboard/mouse assumptions in the app adapter should not leak into
  shared runtime/controller code.

## Completion Criteria

This tactical is complete when:

- `bash android/build-apk.sh` builds a flat Android APK.
- `bash android/validate-avd.sh` installs, launches, captures a nonblank
  screenshot, and passes logcat fatal scanning.
- The rendered frame comes from mclone renderer/runtime code, not only a clear
  color.
- Android packaging and validation docs are linked from `README.md` or
  `native/README.md`.
- Desktop native validation and native web build still pass after shared
  extraction.

## Follow-ups

- Real mobile gameplay controls should move through shared `mclone-ui` and the
  native-web touch lessons, not a permanent Android-only UI fork.
- Desktop OpenXR should be a separate tactical after flat Android has proven
  the native mobile package/lifecycle path.
- Android XR / Quest standalone should remain separate from flat Android and
  reuse the future OpenXR host path rather than extending the flat
  `NativeActivity` loop.
