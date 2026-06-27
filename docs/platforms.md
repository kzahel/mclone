# Platform Direction

This document owns Mclone's current platform posture for the native Rust
engine. It is the entrypoint for supported client/platform lanes, validation
commands, and the boundaries that keep shared engine crates platform-neutral.

The durable rewrite roadmap remains
[`native-rewrite-roadmap.md`](native-rewrite-roadmap.md). This page is
narrower: app hosts, surfaces, packaging, validation lanes, and cross-platform
contract health.

## Current Status

Mclone currently has five supported client/platform validation lanes:

| Target | Status | Validation shape |
|---|---|---|
| Desktop flat | primary development lane | `native/apps/mclone-native-client` owns desktop `winit`, surface acquisition, keyboard/mouse input, frame pacing, and headless screenshots. Basics validated: integrated runtime, locomotion, world rendering, chunk loading/generation. |
| Desktop OpenXR | active XR lane | `mclone-native-client --features xr` owns desktop runtime selection and OpenXR startup. Shared XR crates provide host/session helpers, graphics wrapping, scene alignment, and controller locomotion. Validated with real stereo mclone terrain on Quest 3 through VirtualDesktopXR. |
| Android XR / Quest standalone | active XR lane | `native/apps/mclone-android-xr-client` plus [`../android-xr/`](../android-xr/) own Quest package, Android OpenXR loader, activity glue, asset staging, launch-scoped remote-address argv, and validation. Validated with staged assets, stereo terrain, controller actions, basic locomotion, and remote-dedicated play over direct LAN and through the `--adb-reverse` USB tunnel path. |
| Flat Android | active mobile lane | `native/apps/mclone-android-client` plus [`../android/`](../android/) own the non-XR `NativeActivity` package. Validated on x86_64 AVD with Vulkan-backed `wgpu`, real terrain pixels, staged assets, rendered-frame log marker, touch smoke, and app-owned New World / Join Remote scene replacement; arm64 remains the physical device / Quest-flat ABI. |
| Web/WASM | active browser lane | `native/apps/mclone-web-client` builds for `wasm32-unknown-unknown`, uses WebGPU through `wgpu`, and keeps browser workers, TypeScript glue, mobile web controls, shared Rust/WebGPU UI, and deployment alive. |

Additional host lane:

| Host | Status | Notes |
|---|---|---|
| Native dedicated server | active | `native/apps/mclone-dedicated-server` validates the protocol/server boundary without a renderer. It is not one of the five client display platforms, but it is part of the shared runtime contract. |

The retired TypeScript/browser engine is gone from the live tree. Use Git
history only when old behavior context is explicitly needed; retained oracle
helpers and fixtures remain active reference assets.

## Current Direction

Desktop flat remains the fastest daily loop. That is an iteration choice, not
permission to make shared engine APIs desktop-shaped.

Client platform and server host mode are separate axes. Local integrated play
is a useful default for bring-up and offline validation, but every supported
client lane should retain a path to dedicated-server play. Future P2P or
shared-session modes should reuse the same command/update protocol and runtime
contracts, with only the transport/session adapter changing.

The platform posture is now validation-backed across the five client targets.
New shared features should be designed against shared contracts first, then
checked through representative gates. Do not require every feature branch to
run every device and headset lane unless the change touches platform adapter,
renderer target/view ownership, OpenXR behavior, Android packaging, browser
worker/ABI glue, or another boundary where that platform can fail uniquely.

Near-term product gaps are feature parity and codebase alignment, not more
platform bring-up:

- lighting correctness and render integration still need continued parity work
- menu/HUD/options/loading UI need enough shared Rust/WebGPU coverage to stop
  each platform inventing its own surface
- platform adapters should be thinner around shared runtime/render/session
  contracts
- validation should move toward contract tests plus targeted platform smokes
  instead of broad manual matrix checks for every change

## Reference Engine

Use `~/code/playbox` as a pattern library only. Do not depend on it directly,
and do not copy its PhysX/VaM runtime shape into Mclone.

The useful Playbox references are:

| Reference | Use |
|---|---|
| `~/code/playbox/Cargo.toml` | `cdylib` library build shape, `winit` `android-native-activity` feature, Android target dependencies, optimized debug profile policy. |
| `~/code/playbox/src/core.rs` | Shared single-view runtime pattern used by desktop, flat Android, headless, and debug paths. |
| `~/code/playbox/src/android.rs` | Flat Android `NativeActivity` host: lifecycle, Vulkan-only `wgpu`, resume/suspend teardown, resize, redraw loop, touch orbit input, and surface error handling. |
| `~/code/playbox/src/render/mod.rs` and `src/render/targets.rs` | Explicit render view and target boundaries that let desktop, flat Android, headless, and XR hosts feed renderer facts instead of desktop windows. |
| `~/code/playbox/android/` | Flat Android Gradle wrapper, manifest, `cargo ndk` build script, AVD validator, screenshot capture, and logcat fatal scanning. |
| `~/code/playbox/src/xr/` and `~/code/playbox/android-xr/` | OpenXR loader/session/swapchain ownership, per-eye target acquisition, Quest manifest features, startup property/intent validation, and device validation scripts. |

## Architecture Boundary

Platform hosts own:

- event loop and lifecycle
- desktop window, Android activity, browser canvas, or OpenXR session
- `wgpu` surface/swapchain acquisition and presentation pacing
- OpenXR swapchain image acquisition/release where applicable
- platform input collection and translation
- platform storage and asset-source selection
- app package scripts and device/headset validation

Shared engine crates own:

- protocol and client/server session facts
- authoritative runtime, chunk scheduling, and chunk publication
- host-mode-neutral client/server command and update contracts
- client replica, movement/input intent, and interaction state
- asset parsing and packed asset source abstractions
- render-section meshing, dirty/cache policy, and compile scheduling
- renderer resources and frame drawing from explicit view/target facts
- shared Rust/WebGPU UI model and draw list

Shared app/runtime boundary crates currently include:

- `mclone-app-runtime`: shared single-view runtime helpers used by desktop,
  flat Android, headless captures, and XR terrain runtime construction
- `mclone-render-session`: shared render-section dirty state, compile request,
  cache update, neighbor-readiness, and camera-controller contracts used by
  desktop and web, and consumed by XR scene code
- `mclone-xr-host`: shared OpenXR event/session/frame/action/view helpers used
  by desktop XR and Android XR
- `mclone-xr-graphics`: shared unsafe Vulkan/OpenXR/`wgpu` graphics bridge used
  by desktop Vulkan XR and Android XR
- `mclone-xr-scene`: shared XR terrain/actor scene, startup view-pose
  alignment, and controller-to-engine locomotion mapper

Core shared crates must not depend on:

- `winit`
- `android-activity`, JNI, or Android package paths
- DOM, `web_sys`, or browser workers
- OpenXR sessions, action sets, or swapchains
- platform filesystem locations such as Android app files

The app/platform XR crates may depend on OpenXR and `wgpu`, but they must not
own simulation rules, chunk scheduler policy, private renderers, or platform
activity/window glue. Android-specific activity/JNI/Horizon behavior stays in
the Android XR app. Desktop runtime selection and launch helpers stay in the
desktop app/scripts.

`mclone-render` may depend on `wgpu` and own GPU resources, but host-facing
entry points should continue to accept explicit render target and view data.
This is already true through `RenderFrameContext`, `RenderFrameTarget`,
`ChunkRenderView`, `ChunkRenderTarget`, and the XR render-view descriptors.

## Host Shapes

Single-view hosts:

```text
platform input/lifecycle
  -> platform adapter
  -> shared client/runtime/render-session state
  -> explicit single render view + render target
  -> mclone-render
```

This includes desktop flat, flat Android, headless captures, and the web canvas
path. The app shells are not identical: desktop owns native threads and
keyboard/mouse, Android owns `NativeActivity` lifecycle and touch, and web owns
browser workers and canvas APIs. The convergence point is shared runtime/render
state and explicit frame facts. The server host mode is independent from that
platform shell: local integrated and remote dedicated should differ by
session/transport adapter, not by private client, simulation, or render-session
logic.

Stereo XR hosts:

```text
OpenXR runtime/actions/swapchains
  -> platform XR adapter
  -> shared XR host/graphics/scene helpers
  -> shared client/runtime/render-session state
  -> explicit per-eye render views + targets
  -> mclone-render
```

Desktop XR and Android XR should continue to share OpenXR host/session/action
and terrain-scene behavior where possible. They should diverge only at runtime
discovery, Android loader/activity glue, packaging, headset wake/restore, and
other true platform concerns.

## Validation Policy

Every supported platform lane has an executable gate. Use `/tmp` for screenshots
and logs.

Recommended default gates:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:desktop-chunk:smoke
pnpm native:web:build
```

Platform-specific gates:

```bash
# Desktop flat
pnpm native:movement:smoke
pnpm native:timedemo:smoke

# Desktop OpenXR, headset/runtime required
pnpm native:xr:check
pnpm native:xr:windows:smoke:connected
pnpm native:xr:windows:mclone:connected

# Flat Android, SDK/AVD required
pnpm native:android:apk
pnpm native:android:apk:avd
pnpm native:android:avd-smoke -- --skip-build
pnpm native:android:avd-touch-smoke -- --skip-build

# Android XR / Quest, attached authorized Quest required
pnpm native:android-xr:apk
pnpm native:android-xr:validate -- --debug --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:validate -- --debug --skip-build --adb-reverse --start-server --view-pose 0,120,-96,180
pnpm native:android-xr:validate -- --debug --skip-build --start-server --server-listen 0.0.0.0:25565 --remote-addr HOST:25565 --view-pose 0,120,-96,180

# Web/WASM
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
```

Run the narrowest lane that can catch the bug class:

- simulation/content changes: native tests, oracle fixtures, and web compile
  gates before platform device lanes
- renderer/view/target changes: desktop headless screenshot first, then web or
  one device/headset lane depending on the affected boundary
- shared runtime/render-session changes: desktop flat plus web build/smoke;
  add Android/XR checks when app-runtime or XR scene contracts change
- Android activity/package changes: the relevant Android APK and validation
  lane
- OpenXR host/graphics/action changes: desktop XR and Android XR compile gates;
  run at least one real headset lane before treating the change as validated
- browser worker/ABI changes: web typecheck/smoke lanes before unrelated
  native device work

## Recommended Alignment Work

Highest-value next steps to keep features from requiring constant full-matrix
manual checks:

1. **Keep the platform parity tracker current.** The feature/platform and
   shared-contract matrices live in
   [`topics/platform-parity.md`](topics/platform-parity.md). Update those cells
   when a slice changes platform capability or shared-boundary ownership.
2. **Make the contract matrix more executable.** For each shared crate boundary,
   keep the sentinel smoke/test close to scripts so platform coverage is
   deliberate instead of remembered manually.
3. **Finish dynamic session replacement on native XR scene lanes.** All display
   lanes now have shared initial local/remote session identity through
   `mclone-app-runtime::session`; desktop flat, web, and flat Android also
   restart from menu actions. Desktop XR and Android XR still need app-owned
   scene replacement paths for New World and Join Remote on top of
   `NativeSingleViewSessionRuntime<S>`.
4. **Finish host-mode convergence for web and make Android XR remote validation repeatable.**
   Native desktop, desktop XR, flat Android, and Android XR now share
   `mclone-app-runtime` host-mode and native scene-shell contracts where
   applicable. Web still has an async `WebRuntimeHost`; Android XR remote works
   over LAN after host firewall allow and through the first-class
   `--adb-reverse` validator path.
5. **Keep XR scene convergence complete as features grow.** `mclone-xr-scene`
   now owns shared terrain/actor rendering, startup pose, locomotion, and
   local/remote-capable host shape. Keep future UI, comfort, and interaction
   work behind that shared scene boundary instead of reintroducing app-local XR
   forks.
6. **Promote lighting and UI as shared feature contracts.** Lighting and
   menus/HUD/options/loading UI are the next user-visible parity blockers.
   Land them once through shared data/UI/render contracts instead of per
   platform paths.
7. **Add adapter conformance tests.** Prefer tests for render-target/view
   descriptors, asset-source discovery, input intent mapping, and render-section
   compile contracts over running every device for every feature branch.
8. **Keep device/headset smokes as boundary sentinels.** Run full Android,
   Quest, and desktop XR validation when touching platform glue, packaging,
   OpenXR session/swapchain/action code, graphics wrapping, or shared contracts
   they uniquely exercise.

## Tactical Links

- Flat Android: [`tactical/074-flat-android-build-smoke.md`](tactical/074-flat-android-build-smoke.md)
- Shared single-view runtime prerequisite: [`tactical/075-shared-single-view-runtime-prereq.md`](tactical/075-shared-single-view-runtime-prereq.md)
- XR frontload sequence: [`tactical/076-native-xr-frontload-plan.md`](tactical/076-native-xr-frontload-plan.md)
- Multi-view render contract: [`tactical/077-multiview-render-contract.md`](tactical/077-multiview-render-contract.md)
- Desktop OpenXR clear/frame/controller/locomotion: [`tactical/078-desktop-openxr-clear-smoke.md`](tactical/078-desktop-openxr-clear-smoke.md), [`tactical/079-desktop-openxr-mclone-frame.md`](tactical/079-desktop-openxr-mclone-frame.md), [`tactical/081-desktop-openxr-controller-actions.md`](tactical/081-desktop-openxr-controller-actions.md), [`tactical/082-desktop-xr-player-locomotion.md`](tactical/082-desktop-xr-player-locomotion.md)
- Android XR / Quest standalone: [`tactical/083-android-xr-quest-standalone.md`](tactical/083-android-xr-quest-standalone.md)
- Shared session coordinator: [`tactical/095-shared-session-coordinator.md`](tactical/095-shared-session-coordinator.md)
