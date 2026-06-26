# Native Rewrite Roadmap

This is the durable plan for the Rust/native engine. It supersedes older TS-first roadmap language in [`strategy.md`](strategy.md) and the exploratory posture in [`native-target.md`](native-target.md).

The retired browser engine has been removed from the live tree. Retained reference value now lives in the Java oracle harness under [`../oracle/`](../oracle/), shared oracle fixtures under [`../test/fixtures/`](../test/fixtures/), and Git history.

The primary implementation direction is now:

```text
native-first Rust engine with five validated client/platform lanes:
desktop flat, desktop OpenXR, Android XR / Quest, flat Android, and web/WASM
```

Reference Rust engine for native app/render/XR patterns:

- local path: `~/code/playbox`
- use it for `winit`/`wgpu` bring-up, frame pacing, headless capture, render target, camera, diagnostics, Android/OpenXR reference, and validation patterns
- start with `~/code/playbox/Cargo.toml` for debug-profile optimization policy
- use `~/code/playbox/docs/architecture/rendering.md` and `~/code/playbox/docs/architecture/platforms.md` for render/view/target and platform host boundaries
- use `~/code/playbox/android/README.md`, `~/code/playbox/android-xr/README.md`, and `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md` when planning flat Android, Quest/OpenXR, or desktop OpenXR companion/mirror work
- do not import it as a dependency or copy its PhysX/VaM-specific architecture

## Direction

Build the engine as normal Rust crates first, with desktop/native as the fastest main development loop. Keep the other validated lanes alive through explicit contracts and targeted smokes rather than platform-specific feature forks. Current platform posture lives in [`platforms.md`](platforms.md), and the completed XR frontload sequence is recorded through [`tactical/076-native-xr-frontload-plan.md`](tactical/076-native-xr-frontload-plan.md), [`tactical/077-multiview-render-contract.md`](tactical/077-multiview-render-contract.md), [`tactical/079-desktop-openxr-mclone-frame.md`](tactical/079-desktop-openxr-mclone-frame.md), and [`tactical/083-android-xr-quest-standalone.md`](tactical/083-android-xr-quest-standalone.md).

This is not equal effort across targets:

- desktop flat is the first-priority daily development and screenshot target
- desktop OpenXR is the desktop stereo/runtime validation lane
- Android XR / Quest standalone is the standalone headset validation lane
- flat Android is the single-view native mobile validation lane, tracked by [`tactical/074-flat-android-build-smoke.md`](tactical/074-flat-android-build-smoke.md)
- web/WASM is the browser compatibility/deploy lane
- XR remains native-only until there is a concrete WebXR path worth supporting

## Why Native-First

Native is the better proving ground for engine internals:

- faster iteration for worldgen, lighting, meshing, scheduling, and renderer internals
- easier profiling/debugging
- real filesystem and persistence options
- native threads and lower-friction worker scheduling
- native `wgpu` without browser lifecycle/header/storage constraints
- direct path to Android and OpenXR later

Native debug builds are intentionally optimized at `opt-level = 2` in `native/Cargo.toml`, following Playbox's policy. Use release builds for final budgets, but debug `cargo run` and pnpm native smokes should still be close enough to avoid chasing artifacts from completely unoptimized hot loops.

The browser build is still a product promise, but it should not be the place where every low-level engine decision is first discovered.

## Why Keep Web Alive Early

Do not postpone web entirely. WASM/web constraints affect architecture:

- threading and `SharedArrayBuffer` availability
- blocking vs async boundaries
- asset loading and packaging
- storage APIs
- network transports
- `wgpu`/WebGPU feature limits
- browser lifecycle and page visibility behavior

If these are discovered after building a native-only engine, core APIs may need to be unwound. The web target should catch those issues at subsystem boundaries before they freeze.

## Crate Split

The native workspace is intentionally protocol/client/server shaped from the beginning:

```text
crates/
  mclone_core          # pure data/model/math
  mclone_protocol      # wire/schema/versioned messages
  mclone_net           # transports: local, TCP/QUIC/WebSocket/WebTransport/etc.
  mclone_server        # authoritative world/session runtime
  mclone_client        # client runtime, prediction, interpolation, replica
  mclone_worldgen      # vanilla 1.17.1 worldgen
  mclone_light         # packed sky/block lighting
  mclone_mesh          # chunk meshing
  mclone_assets        # blockstate/model/texture/NBT asset loading
  mclone_render        # wgpu renderer
  mclone_render_session # render-section dirty/cache/compile policy
  mclone_app_runtime   # shared single-view runtime/render helpers
  mclone_ui            # shared Rust/WebGPU UI model
  mclone_xr_host       # shared OpenXR host/session/action/view helpers
  mclone_xr_graphics   # shared Vulkan OpenXR/wgpu graphics bridge
  mclone_xr_scene      # shared XR terrain/runtime/locomotion scene

apps/
  mclone-native-client
  mclone-dedicated-server
  mclone-web-client
  mclone-android-client
  mclone-android-xr-client
```

Future app crates should stay out of the workspace until they have a validation lane. The current app crates are already validation-backed:

```text
apps/
  mclone-native-client       # desktop flat + opt-in desktop OpenXR
  mclone-web-client          # Rust/WASM browser client
  mclone-android-client      # flat Android single-view host
  mclone-android-xr-client   # Quest/OpenXR standalone host
  mclone-dedicated-server    # headless server host
```

Responsibilities:

| Crate | Owns | Does not own |
|---|---|---|
| `mclone_protocol` | versioned game messages, serialization, join/session, chunk snapshots, block/entity/light deltas, player commands, acks, snapshots | sockets, game rules |
| `mclone_net` | native and web transport adapters, local loopback channels | protocol semantics, game rules |
| `mclone_server` | authoritative state machine, sessions, ticks, chunk interest, worldgen scheduling, persistence hooks, snapshot publication | renderer state, client prediction |
| `mclone_client` | replicated client world, command stream, prediction/reconciliation, interpolation, render-facing presentation state | authoritative simulation |
| `mclone_worldgen` | Java 1.17.1 seed-parity generation | rendering, networking, storage adapters |
| `mclone_light` | vanilla-shaped sky/block light data and solving | mesh ownership |
| `mclone_mesh` | renderer-ready chunk mesh data | GPU handles |
| `mclone_render` | native/web `wgpu` presentation backend | authoritative world state |
| `mclone_render_session` | render-section dirty state, compile requests, cache updates, neighbor readiness, camera controller contracts | platform windows, GPU swapchains |
| `mclone_app_runtime` | shared single-view runtime/render helpers, asset loading, full-frame composition | platform event loops or package glue |
| `mclone_ui` | shared GUI draw model and renderer-facing UI facts | platform DOM or native menu systems |
| `mclone_xr_host` | OpenXR session/event/frame/action/view helpers shared by desktop XR and Android XR | Android activity/JNI, desktop runtime launcher scripts |
| `mclone_xr_graphics` | shared unsafe Vulkan OpenXR/wgpu graphics bridge | platform loader/bootstrap policy |
| `mclone_xr_scene` | shared XR terrain runtime, startup pose alignment, and controller locomotion mapping | Quest package or desktop window ownership |

The protocol/network split is first-class. Singleplayer should use the same client/server boundary through a local transport, not a private shortcut that makes multiplayer a retrofit.

## Target Topologies

Native client:

```text
input -> mclone_client -> mclone_protocol -> mclone_net
mclone_client -> mclone_render
```

Flat Android client:

```text
Android lifecycle/input adapters -> mclone_client
mclone_client -> explicit single-view render target -> mclone_render
```

Android XR / Quest client:

```text
OpenXR session/actions/swapchains -> XR host app
XR host app -> mclone_client
XR host app -> per-eye render views/targets -> mclone_render
```

Dedicated server:

```text
mclone_net -> mclone_protocol -> mclone_server
```

Singleplayer:

```text
mclone_client <-> local transport <-> mclone_server
```

Web client:

```text
browser input/storage/socket adapters
wasm mclone_client
mclone_protocol
web transport adapter
render via wgpu/web
```

## Implementation Sequence

1. **Core/oracle first**

   Build `mclone_core` and `mclone_worldgen` as normal Rust crates. Native CLI/unit tests load existing oracle fixtures. No renderer required.

2. **Native minimal app**

   Add `winit` + `wgpu`, clear screen, camera/input, and one chunk. This becomes the main development loop.
   The native tactical sequence starts at [`tactical/000-native-render-bringup.md`](tactical/000-native-render-bringup.md); the index is [`tactical/README.md`](tactical/README.md). The parent checklist for reaching the retired engine's capability horizon is [`tactical/003-native-ts-parity-roadmap.md`](tactical/003-native-ts-parity-roadmap.md).

3. **Web smoke very early**

   Compile the same renderer/client path to WASM. The browser target only needs to boot, create a WebGPU/`wgpu` device, draw a clear color or one quad, and load one tiny asset. Keep this as a CI/manual smoke gate.

4. **Engine systems native-first**

   Develop and profile worldgen, lighting, meshing, chunk scheduler, protocol, and local client/server loop natively.

5. **Promote web at subsystem boundaries**

   When threading, storage, networking, asset streaming, or renderer capabilities are introduced, make the web adapter real before the API freezes.

6. **Preserve platform boundaries while adding features**

   Renderer view/projection inputs and render targets are now explicit enough for desktop, headless, web, flat Android, and stereo XR hosts to drive shared rendering. Keep that boundary intact while lighting, UI, entities, and gameplay grow. The first boundary pass is [`tactical/022-platform-target-contract-and-render-boundary.md`](tactical/022-platform-target-contract-and-render-boundary.md); the multi-view pass is [`tactical/077-multiview-render-contract.md`](tactical/077-multiview-render-contract.md).

## Rule Of Thumb

```text
core logic: native tests first, web compile gate
renderer: desktop/headless first, web smoke per milestone, device/headset smoke for target/view/platform boundary changes
threading/scheduler: design for web constraints immediately
storage/network: adapter-shaped from day one
Android/XR: keep package/activity/session/swapchain glue in app/platform adapters
XR: native-only until there is a concrete WebXR path worth supporting
```

## Current Health Arc

The platform bring-up arc is now broad enough that the highest-value work is
shared feature parity and boundary consolidation, not more app scaffolding.

Current native shape:

- Java-shaped worldgen, terrain, carvers, surface/decorated chunk foundation,
  server scheduler, client replica, movement, interaction, and persistence
- first-pass sky/block lighting pipeline and render-light integration
- shared render-section dirty/cache/compile policy across desktop and web
- shared native single-view scene shell consumed by desktop/headless and flat
  Android, with concrete transport/config kept in app crates
- shared XR host/graphics/scene crates consumed by desktop XR and Android XR
- Rust/WebGPU UI path replacing the old web DOM UI, with menu/options/loading
  feature parity still needed

Recommended next alignment milestones:

1. keep the platform parity/contract matrices in
   [`topics/platform-parity.md`](topics/platform-parity.md) current and connect
   each shared boundary to an explicit smoke/test sentinel
2. reconcile host-mode convergence for web and XR so local-integrated versus
   remote-dedicated remains a shared runtime/session contract across all lanes
3. finish desktop XR terrain-state convergence onto `mclone-xr-scene` so
   desktop XR and Quest do not diverge before actors/UI/comfort features
4. advance lighting correctness/rendering and shared menu/HUD/options/loading
   UI as platform-neutral features
5. add adapter conformance tests for render targets/views, asset discovery,
   input intent mapping, and render-section compile contracts

## Documentation Ownership

- This file owns the native rewrite direction and target topology.
- [`native/README.md`](../native/README.md) owns workspace mechanics and crate list.
- [`platforms.md`](platforms.md) owns the current platform matrix and validation policy.
- [`worldgen-status.md`](worldgen-status.md) should describe native worldgen status and oracle fixture coverage.
- Numbered tactical docs remain useful work logs, but older TS-first tactical language should not override this roadmap.
