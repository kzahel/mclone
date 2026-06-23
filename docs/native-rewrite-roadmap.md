# Native Rewrite Roadmap

This is the durable plan for the Rust/native rewrite. It supersedes older TS-first roadmap language in [`strategy.md`](strategy.md) and the exploratory posture in [`native-target.md`](native-target.md).

The TypeScript implementation remains valuable, but its role changes:

- legacy working implementation and behavior reference
- oracle scaffolding, fixture source, and parity target prior art
- experimental browser/runtime prototype
- not the primary engine implementation direction

The primary implementation direction is now:

```text
native-first Rust engine, desktop bring-up first, web target kept alive early, Android XR later
```

Reference Rust engine for native app/render/XR patterns:

- local path: `~/code/playbox`
- use it for `winit`/`wgpu` bring-up, frame pacing, headless capture, render target, camera, diagnostics, Android/OpenXR reference, and validation patterns
- start with `~/code/playbox/Cargo.toml` for debug-profile optimization policy
- use `~/code/playbox/docs/architecture/rendering.md` and `~/code/playbox/docs/architecture/platforms.md` for render/view/target and platform host boundaries
- use `~/code/playbox/android/README.md`, `~/code/playbox/android-xr/README.md`, and `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md` when planning flat Android, Quest/OpenXR, or desktop OpenXR companion/mirror work
- do not import it as a dependency or copy its PhysX/VaM-specific architecture

## Direction

Build the engine as normal Rust crates first, with desktop/native as the main development loop. Keep a thin WASM/web target compiling and booting early so browser constraints stay visible while APIs are still easy to adjust. Flat Android is now the next platform frontload target, limited to a non-XR single-view host with a real validation lane. Treat Android XR / Quest standalone as a real later native target, but do not pull OpenXR app scaffolding forward before the flat Android and desktop/XR boundaries are mature enough to validate them. Current platform posture lives in [`platforms.md`](platforms.md).

This is not equal effort across targets:

- native desktop is the first-priority bring-up and validation target
- web is an early compatibility gate
- flat Android is the next single-view native host workstream, tracked by [`tactical/074-flat-android-build-smoke.md`](tactical/074-flat-android-build-smoke.md)
- Android XR / Quest standalone is a later native XR host, not part of the flat Android workstream
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

apps/
  mclone_native_client
  mclone_dedicated_server
  mclone_web_client
```

Future app crates should stay out of the workspace until they have a validation lane. Flat Android now has a proposed validation-backed tactical:

```text
apps/
  mclone_android_client      # next flat Android single-view host
  mclone_android_xr_client   # future Quest/OpenXR host
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

The protocol/network split is first-class. Singleplayer should use the same client/server boundary through a local transport, not a private shortcut that makes multiplayer a retrofit.

## Target Topologies

Native client:

```text
input -> mclone_client -> mclone_protocol -> mclone_net
mclone_client -> mclone_render
```

Future flat Android client:

```text
Android lifecycle/input adapters -> mclone_client
mclone_client -> explicit single-view render target -> mclone_render
```

Future Android XR / Quest client:

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
   The native tactical sequence starts at [`tactical/000-native-render-bringup.md`](tactical/000-native-render-bringup.md); the index is [`tactical/README.md`](tactical/README.md). The parent checklist for reaching the current TypeScript capability horizon is [`tactical/003-native-ts-parity-roadmap.md`](tactical/003-native-ts-parity-roadmap.md).

3. **Web smoke very early**

   Compile the same renderer/client path to WASM. The browser target only needs to boot, create a WebGPU/`wgpu` device, draw a clear color or one quad, and load one tiny asset. Keep this as a CI/manual smoke gate.

4. **Engine systems native-first**

   Develop and profile worldgen, lighting, meshing, chunk scheduler, protocol, and local client/server loop natively.

5. **Promote web at subsystem boundaries**

   When threading, storage, networking, asset streaming, or renderer capabilities are introduced, make the web adapter real before the API freezes.

6. **Preserve future Android XR boundaries**

   Before adding Android or OpenXR app crates, make renderer view/projection inputs and render targets explicit enough that desktop, headless, web, flat Android, and stereo XR hosts can drive the same renderer without desktop `winit` assumptions leaking into shared crates. The near-term tactical for this is [`tactical/022-platform-target-contract-and-render-boundary.md`](tactical/022-platform-target-contract-and-render-boundary.md).

## Rule Of Thumb

```text
core logic: native tests first, web compile gate
renderer: native first, web smoke per milestone
threading/scheduler: design for web constraints immediately
storage/network: adapter-shaped from day one
Android/XR: document and protect boundaries now; defer app scaffolding until renderer view/target contracts are explicit
XR: native-only until there is a concrete WebXR path worth supporting
```

## Immediate Native Worldgen Arc

The current native rewrite is intentionally starting with worldgen because it is highly oracle-testable and independent of renderer/runtime decisions.

Current native shape:

- PRNG and JavaRandom-compatible worldgen seed helpers
- noise primitives and `NoiseSampler`
- terrain density fill
- `OverworldBiomeSource`
- surface and bedrock stage
- classic AIR and LIQUID carvers
- first placement/decorator foundation through range and heightmap placement

Next worldgen milestones:

1. decorator composition (`DecoratedDecorator`) and feature placement core
2. block/state palette boundary for generated chunks
3. ores and underground features as the first block-mutating feature family
4. biome decoration tables for a narrow fixture
5. full decorated native chunk parity against committed oracle fixtures

Only after that should renderer/lighting/meshing work compete for primary focus, unless a small native renderer smoke or platform-boundary cleanup is needed to keep the app path honest. The renderer should keep explicit view/projection and target ownership so future Android and XR hosts do not have to unwind desktop-only assumptions.

## Documentation Ownership

- This file owns the native rewrite direction and target topology.
- [`native/README.md`](../native/README.md) owns workspace mechanics and crate list.
- [`worldgen-status.md`](worldgen-status.md) continues to describe the existing TypeScript worldgen implementation until a native-specific status page exists.
- Numbered tactical docs remain useful work logs, but older TS-first tactical language should not override this roadmap.
