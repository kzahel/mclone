# Native target notes

Historical note on preserving native host/renderer paths for `mclone`.

Archived: this document has been superseded by [`../native-rewrite-roadmap.md`](../native-rewrite-roadmap.md). The committed direction is now native-first Rust with five validated client/platform lanes: desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM, plus an emerging offscreen flat client validation host. Use [`../platforms.md`](../platforms.md) for the current platform matrix and validation policy.

## Current posture

Native desktop is the current product and validation loop because it is the fastest way to bring up worldgen, scheduling, meshing, rendering, persistence, and diagnostics. The browser build remains a compatibility promise, but not the place where every low-level engine decision is first discovered.

Future native targets matter for cases where desktop and browser are a poor fit:

- **Direct OpenXR integration**: browser XR exists via WebXR, but if we decide we need direct OpenXR runtime/headset integration, native is the clearer path.
- **Android XR / Quest standalone**: the standalone headset target now reuses shared client/server/render data while owning its own Android lifecycle, OpenXR loader/session, stereo swapchains, and controller input.
- **Flat Android**: useful as a single-view native host and packaging baseline, but less compelling than Quest/XR for product direction.
- **High-tier rendering experiments**: compute-heavy simulation, indirect multi-draw, bindless-style resource models, and other advanced GPU techniques may fit better outside the browser sandbox.

Native would be a host tier, not a separate game or separate simulation codebase. The goal would be to reuse the same simulation/content logic where practical while swapping host/runtime adapters and the renderer backend.

The important near-term discipline is to avoid desktop lock-in while still moving quickly on desktop. Do not add Android or OpenXR scaffolding just to reserve the target; instead, keep the renderer and runtime boundaries explicit enough that those app crates can be added later without undoing the desktop path.

## Architectural shape

At this level, the shape we need to preserve is simple:

```
┌──────────────────────────────────────────────┐
│ Native host / platform adapter               │
│  ├─ window / input / Android / XR integration│
│  ├─ renderer backend                         │
│  └─ engine runtime                           │
│     ├─ simulation core                       │
│     ├─ server/client runtime                 │
│     └─ meshing / upload handoff              │
└──────────────────────────────────────────────┘
```

This document intentionally does **not** commit to:

- host language
- JS/TS embedding strategy
- frame-loop ownership details
- FFI shape between runtime and renderer

Those are implementation choices to evaluate only if native work becomes real.

## Why this fits the existing architecture

The runtime split in [`../architecture.md`](../architecture.md) already separates simulation, server runtime, client runtime, and renderer. Three parts of that split matter here:

- **The renderer is a consumer, not the owner of world state.** A native renderer should consume the same chunk snapshots, mesh payloads, and upload inputs as the browser renderer.
- **Host concerns are adapter-shaped.** Transport, persistence, and runtime bootstrap already want host-specific implementations behind engine-defined contracts.
- **Data boundaries are becoming explicit.** The more the engine converges on stable serializable shapes between simulation, meshing, and rendering, the easier a future host swap becomes.

In short, the same engine logic should ideally remain usable in:

- native desktop app + `wgpu` renderer
- browser/WASM app + WebGPU renderer
- dedicated native server (no renderer)
- flat Android app + Vulkan-backed `wgpu`
- Android XR / Quest app + OpenXR swapchains wrapped for renderer submission

The exact embedding and threading model can differ by host without changing parity-critical simulation logic.

## Capability tiers

If we ever add a second renderer/backend, the renderer-facing capability surface should expose the features the client/runtime layer needs to make presentation decisions:

- `supportsXR`
- `supportsCompute` and relevant limits
- `supportsIndirectMultiDraw`
- `supportsBindless`
- `maxAnisotropy`, `maxSamples`, and similar limits

Branching on these capabilities should live in renderer-facing feature code or client/runtime presentation code, not in parity-critical simulation rules.

## Frame timing and GC

If native XR work starts, likely constraints include:

- **Decouple sim tick from render tick.** Sim should run at a fixed cadence; rendering should interpolate from snapshots toward predicted display time.
- **Avoid synchronous host/runtime crossings in the hot frame path.** Frame submission should consume prepared state, not wait on arbitrary simulation work.
- **Design for dropped-frame tolerance.** XR runtimes can hide some misses, but the app still needs stable timing and predictable snapshot production.
- **Minimize allocations in hot paths.** Simulation, meshing, and presentation code should keep allocation pressure low enough that GC pauses are not the frame budget.
- **Move hotspots only when measured.** If a future native host needs some work moved out of TS, do it one narrow boundary at a time based on profiling, not preemptively.

## Discipline to maintain today

Keeping a native option open is cheap now and expensive later. The single most important rule is still:

**No WebGPU types above the renderer interface.** `GPUBuffer`, `GPUTexture`, `GPUDevice`, `GPURenderPassEncoder`, and related WebGPU types should only exist inside the WebGPU renderer implementation.

Related rule:

**No browser-only host types inside shared simulation/runtime contracts.** `Worker`, `MessagePort`, DOM nodes, canvas handles, and similar host-specific objects should not leak into simulation-core or engine-level protocol shapes.

Concretely:

- Game code, world client code, and meshing-output schemas should describe geometry and materials in renderer-agnostic terms.
- Mesh data should cross boundaries as typed arrays or other documented plain-data records, not GPU handles.
- Camera, transform, and material description types should live in engine/client runtime code, not in backend implementations.
- Shared contracts should stay serializable and host-neutral.

If a `GPUBuffer` shows up in `worldgen/` or `runtime/`, or a `Worker` shows up in simulation-core contracts, that is a regression worth fixing immediately.

## Start gate for Android/OpenXR scaffolding

Native work has started in earnest; this start gate now applies to Android and OpenXR app scaffolding. Before those targets are added, all of the following should be true:

- desktop/headless rendering already uses explicit view/projection and render-target data rather than a desktop-only camera/swapchain shape
- shared client/server/mesh/asset crates do not depend on `winit`, Android activity glue, OpenXR sessions, or other host-specific objects
- there is a concrete validation lane: attached Quest, Android emulator, desktop OpenXR runtime, or a focused compile/build smoke
- the first slice is intentionally small: flat Android clear/chunk smoke, desktop OpenXR stereo smoke, or Android XR loader/session smoke
- packaging and asset decisions are made from that validation lane, not from speculative scaffolding

## Open questions

These are only worth answering once the start gate above is met:

- What host language and embedding model best fit the actual requirement?
- Can the native renderer share WGSL directly, or does it need a target-specific shader path?
- Should assets stay shared across browser and native, or be processed per target?
- Does audio need the same kind of host-agnostic interface the renderer and storage layers need?
- What packaging/distribution work would the chosen native target imply?

## Non-goals

- Replacing the browser build. Web/WASM stays a compatibility target.
- Letting desktop bring-up leak `winit` or desktop filesystem assumptions into shared engine contracts.
- Starting Android or OpenXR implementation work before renderer view/target contracts and a validation lane exist.
- A separate game-logic codebase. There is one engine; native would be one of its hosts.
