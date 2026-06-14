# Native Rewrite Roadmap

This is the durable plan for the Rust/native rewrite. It supersedes older TS-first roadmap language in [`strategy.md`](strategy.md) and the exploratory posture in [`native-target.md`](native-target.md).

The TypeScript implementation remains valuable, but its role changes:

- legacy working implementation and behavior reference
- oracle scaffolding, fixture source, and parity target prior art
- experimental browser/runtime prototype
- not the primary engine implementation direction

The primary implementation direction is now:

```text
native-first Rust engine, web target kept alive from the beginning
```

## Direction

Build the engine as normal Rust crates first, with desktop/native as the main development loop. Keep a thin WASM/web target compiling and booting early so browser constraints stay visible while APIs are still easy to adjust.

This is not equal effort across targets:

- native desktop is the primary engine target
- web is an early compatibility gate
- XR remains native-only until there is a concrete WebXR path worth supporting

## Why Native-First

Native is the better proving ground for engine internals:

- faster iteration for worldgen, lighting, meshing, scheduling, and renderer internals
- easier profiling/debugging
- real filesystem and persistence options
- native threads and lower-friction worker scheduling
- native `wgpu` without browser lifecycle/header/storage constraints
- direct path to OpenXR later

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

3. **Web smoke very early**

   Compile the same renderer/client path to WASM. The browser target only needs to boot, create a WebGPU/`wgpu` device, draw a clear color or one quad, and load one tiny asset. Keep this as a CI/manual smoke gate.

4. **Engine systems native-first**

   Develop and profile worldgen, lighting, meshing, chunk scheduler, protocol, and local client/server loop natively.

5. **Promote web at subsystem boundaries**

   When threading, storage, networking, asset streaming, or renderer capabilities are introduced, make the web adapter real before the API freezes.

## Rule Of Thumb

```text
core logic: native tests first, web compile gate
renderer: native first, web smoke per milestone
threading/scheduler: design for web constraints immediately
storage/network: adapter-shaped from day one
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

Only after that should renderer/lighting/meshing work compete for primary focus, unless a small native renderer smoke is needed to keep the app path honest.

## Documentation Ownership

- This file owns the native rewrite direction and target topology.
- [`native/README.md`](../native/README.md) owns workspace mechanics and crate list.
- [`worldgen-status.md`](worldgen-status.md) continues to describe the existing TypeScript worldgen implementation until a native-specific status page exists.
- Numbered tactical docs remain useful work logs, but older TS-first tactical language should not override this roadmap.
