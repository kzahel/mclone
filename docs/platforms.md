# Platform Direction

This document owns Mclone's platform posture for the native Rust engine. It
captures the current target status, how to use the Playbox reference engine,
and how XR work should proceed after the flat Android baseline.

The durable rewrite roadmap remains [`native-rewrite-roadmap.md`](native-rewrite-roadmap.md).
This page is narrower: app hosts, surfaces, packaging, validation lanes, and
the boundaries that keep shared engine crates platform-neutral.

## Current Status

| Target | Status | Notes |
|---|---|---|
| Native desktop | active primary target | `native/apps/mclone-native-client` is the main interactive loop and validation surface. It owns desktop `winit`, surface acquisition, input, frame pacing, and headless/native screenshots. |
| Native web/WASM | active compatibility and deploy target | `native/apps/mclone-web-client` builds for `wasm32-unknown-unknown`, uses WebGPU through `wgpu`, and keeps browser workers, TypeScript glue, mobile web controls, and deployment alive. |
| Native dedicated server | active | `native/apps/mclone-dedicated-server` validates the protocol/server boundary without a renderer. |
| Flat Android | baseline validation lane complete | `native/apps/mclone-android-client` builds a flat `NativeActivity` APK, renders integrated-runtime terrain through Vulkan-backed `wgpu`, stages packed assets, validates on AVD with screenshot/logcat smokes, and has a touch-orbit smoke. Quest-flat hardware capture remains optional/pending. |
| Desktop OpenXR | next XR runtime target after multi-view cleanup | No mclone OpenXR code exists. The next XR sequence is multi-view render-boundary cleanup, then desktop OpenXR clear/session smoke, then a real mclone frame through OpenXR. |
| Android XR / Quest standalone | future | A real future native target, but gated behind flat Android confidence and a desktop OpenXR path. Do not fold Quest/OpenXR assumptions into the flat Android app. |
| Legacy TypeScript/browser engine | reference only | Useful for prior behavior and fixtures until retired, not a platform direction for new engine work. |

## Current Direction

Desktop native remains the main development loop. Native web/WASM remains a
compatibility gate because browser constraints can still expose bad shared API
decisions early.

Flat Android is now a baseline platform validation lane. The goal was not a
complete mobile product; it was to discover native Android packaging,
lifecycle, Vulkan-backed `wgpu`, asset packaging, and touch/control constraints
while app and renderer boundaries were still malleable.

XR stays out of the flat Android workstream. The likely XR sequence is now:

1. finish multi-view render-contract cleanup without OpenXR
2. harden platform-neutral input/startup seams where they affect XR
3. bring up desktop OpenXR when stereo/runtime risks become the next priority
4. add Android XR / Quest standalone after flat Android and desktop XR have
   proven the separate platform concerns

## Reference Engine

Use `~/code/playbox` as a pattern library only. Do not depend on it directly,
and do not copy its PhysX/VaM runtime shape into Mclone.

The useful Playbox references are:

| Reference | Use |
|---|---|
| `~/code/playbox/Cargo.toml` | `cdylib` library build shape, `winit` `android-native-activity` feature, Android target dependencies, optimized debug profile policy. |
| `~/code/playbox/src/core.rs` | `SingleViewRuntime`: one shared single-view runtime used by desktop, flat Android, headless, and debug paths. XR bypasses it and drives shared world/render data with stereo views. |
| `~/code/playbox/src/android.rs` | Flat Android `NativeActivity` host: `android_main`, `EventLoopBuilderExtAndroid`, Vulkan-only `wgpu`, resume/suspend teardown, resize, redraw loop, touch orbit input, and surface error handling. |
| `~/code/playbox/src/gpu.rs` | Desktop surface/device setup, preferred surface format, optional GPU features, and present-mode policy. |
| `~/code/playbox/src/render/mod.rs` and `src/render/targets.rs` | Explicit `RenderViewInput` and `SceneTarget` boundaries that allow desktop, flat Android, headless, and XR hosts to feed renderer facts instead of desktop windows. |
| `~/code/playbox/android/` | Flat Android Gradle wrapper, manifest, `cargo ndk` build script, AVD validator, Quest-flat validator, screenshot capture, and logcat fatal scanning. This is the most directly copyable part. |
| `~/code/playbox/src/xr/` and `~/code/playbox/android-xr/` | Later XR reference only: OpenXR loader/session/swapchain ownership, per-eye target acquisition, Quest manifest features, startup property/intent validation. |

## Architecture Boundary

Platform hosts own:

- event loop and lifecycle
- native window, Android `NativeActivity`, browser canvas, or OpenXR session
- `wgpu` surface/swapchain acquisition and presentation pacing
- platform input collection and translation
- platform storage and asset-source selection
- app package scripts and device validation

Shared engine crates own:

- protocol and client/server session facts
- authoritative runtime and chunk scheduling
- client replica and movement/input intent state
- asset parsing and packed asset source abstractions
- render-section meshing and render-session policy
- renderer resources and frame drawing from explicit view/target facts

Shared crates must not depend on:

- `winit`
- `android-activity`
- DOM, `web_sys`, or browser workers
- OpenXR sessions, action sets, or swapchains
- platform filesystem locations such as Android app files

`mclone-render` may depend on `wgpu` and own GPU resources, but host-facing
entry points should continue to accept explicit render target and view data.
This is already partly true through `RenderFrameContext`, `RenderFrameTarget`,
`ChunkRenderView`, and `ChunkRenderTarget`.

## Single-View Host Shape

Desktop, flat Android, headless captures, and native web should converge on the
same single-view engine concepts:

```text
platform input/lifecycle
  -> platform adapter
  -> shared client/runtime/render-session state
  -> explicit single render view + render target
  -> mclone-render
```

This does not mean all app shells use identical code. Desktop can own native
threads and `winit` keyboard/mouse behavior; web can own browser workers and
canvas APIs; Android can own `NativeActivity` lifecycle and touch translation.
The convergence point is shared runtime/render state and explicit frame facts,
not a universal platform ABI.

Mclone's current desktop code already has most of the pieces:

- `WindowSceneRuntime` owns local/remote runtime, client replica, render-section
  cache synchronization, asset loading, and actor assets.
- `render_full_frame` composes sky, chunks, actors, underwater overlay, GUI, and
  debug pane.
- `RenderFrameContext` describes host-acquired device/queue/encoder/target
  facts independent of window, canvas, or offscreen readback.

Flat Android now reuses these pieces instead of forking the engine. Future
platform work should preserve that shape: host adapters own lifecycle, surface,
input, and package paths; shared crates own runtime/render-session policy and
rendering from explicit frame facts.

## Flat Android Target

The first Android target is a 2D, flatscreen `NativeActivity` app:

- Rust `cdylib` built with `cargo ndk` for `arm64-v8a`
- Android package with a minimal `NativeActivity` manifest
- `winit` Android event loop through `EventLoopBuilderExtAndroid`
- Vulkan-only `wgpu` instance for the Android surface
- one color target and one host-owned depth target
- packed Minecraft assets loaded from Android app files or a deliberately
  staged location
- minimal touch/gameplay input, initially enough for smoke validation
- AVD screenshot/logcat smoke and optional Quest-flat validation

It should not include:

- OpenXR loader/session/action/swapchain code
- Quest VR manifest categories
- Gradle/Java UI beyond what flat `NativeActivity` requires
- a separate renderer or separate gameplay/runtime fork

## Android Asset Policy

Desktop currently discovers assets from repo-relative loose assets and packed
asset candidates. Android cannot rely on repo-relative paths.

The Android path should prefer a packed Minecraft asset file and search
platform-owned locations first, such as:

```text
/sdcard/Android/data/<mclone package>/files/
/storage/emulated/0/Android/data/<mclone package>/files/
```

Long term, asset source selection belongs in a shared adapter-friendly helper
that can be driven by desktop env vars, browser fetch/bundle URLs, and Android
app files without changing model/atlas loading code.

## Validation Policy

Every platform bring-up must have an executable validation lane.

For flat Android, the baseline lane is:

1. build the Rust shared library with `cargo ndk`
2. build an APK
3. install and launch on an AVD
4. assert the process is alive and the `NativeActivity` is resumed/focused
5. capture a screenshot to `/tmp`
6. scan logcat for fatal exception, fatal signal, Rust panic, or segfault

Quest-flat validation is useful after AVD works, but it is not a substitute for
an emulator smoke because Quest availability should not be required for every
platform regression check.

For desktop OpenXR, the first lane should be bounded and scriptable:

1. build/check the XR feature without affecting non-XR builds
2. launch the active OpenXR runtime
3. create a real session and per-eye swapchains
4. submit a bounded number of clear/test-pattern frames
5. log runtime/backend/swapchain facts and exit cleanly

## Tactical Link

The completed flat Android workstream lives in
[`tactical/074-flat-android-build-smoke.md`](tactical/074-flat-android-build-smoke.md).
The XR frontload sequence lives in
[`tactical/076-native-xr-frontload-plan.md`](tactical/076-native-xr-frontload-plan.md).
