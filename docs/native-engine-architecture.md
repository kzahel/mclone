# Native Engine Architecture

This document owns the durable architecture shape for the current Rust engine. Mclone is no longer in a transition from an older live engine; the native Rust workspace under [`../native/`](../native/) is the default implementation surface.

Current platform posture, validation lanes, Playbox references, and platform boundary details live in [`platforms.md`](platforms.md). Numbered implementation plans and work logs live in [`tactical/`](tactical/README.md).

The draft target for keeping flat, XR, web, Android, offscreen, and emulated
validation lanes on one product behavior surface lives in
[`client-experience-architecture.md`](client-experience-architecture.md).

## Direction

The engine is native-first Rust with five validated client/platform lanes:

- desktop flat
- desktop OpenXR
- Android XR / Quest standalone
- flat Android
- web/WASM

Desktop flat remains the fastest daily interactive development loop. That is an iteration choice, not permission to make shared engine APIs desktop-shaped. For feature work that does not name a platform, use the default framing: **shared implementation, desktop validation first**.

Client platform and server host mode are separate axes. Local integrated play, remote dedicated play, and future session/P2P modes should reuse shared command/update contracts instead of becoming platform forks.
The durable command/update topology and local-vs-remote transport boundary live
in [`session-network-architecture.md`](session-network-architecture.md).

## Crate Ownership

The workspace is intentionally protocol/client/server shaped:

```text
native/
  crates/
    mclone_core
    mclone_protocol
    mclone_net
    mclone_server
    mclone_client
    mclone_worldgen
    mclone_light
    mclone_mesh
    mclone_assets
    mclone_input
    mclone_audio
    mclone_render
    mclone_render_session
    mclone_app_runtime
    mclone_ui
    mclone_xr_host
    mclone_xr_graphics
    mclone_scene
  apps/
    mclone-native-client
    mclone-dedicated-server
    mclone-web-client
    mclone-android-client
    mclone-android-xr-client
```

Shared engine crates own:

- protocol and client/server session facts
- authoritative runtime, chunk scheduling, and chunk publication
- host-mode-neutral command and update contracts
- client replica, movement/input intent, and interaction state
- asset parsing and packed asset-source abstractions
- render-section meshing, dirty/cache policy, and compile scheduling
- renderer resources and frame drawing from explicit view/target facts
- the shared native scene/session/UI/orchestration host (`mclone-scene`)
- shared Rust/WebGPU UI model and draw list

Platform app crates own:

- event loops and lifecycle glue
- desktop window, Android activity, browser canvas, or OpenXR session ownership
- surface/swapchain acquisition and presentation pacing
- platform input collection and translation
- platform storage, transport setup, package scripts, and device validation

## Host Shapes

Native clients are converging on one `mclone-scene` host with thin cadence and
surface drivers (tactical 168). Single-view hosts use the mono topology:

```text
platform input/lifecycle
  -> platform adapter
  -> mclone-scene shared session/runtime/UI orchestration
  -> explicit single render view + render target
  -> mclone-render
```

This covers desktop flat, offscreen flat, flat Android, and web canvas paths.
Player-controlled flat cameras cross this boundary as renderer-facing poses:
`mclone-render-session` converts engine yaw/pitch snapshots into
`PerspectiveRenderPose`, and `mclone-render` validates that pose while building
finite `ChunkRenderView` matrices and camera bases. `ChunkCamera` remains a
compatibility shape for fixed overview/headless diagnostics; new flat player
camera paths should not reconstruct rendering from `eye + target + world_up`.

Stereo XR hosts use the same scene host with OpenXR confined to the rim:

```text
OpenXR runtime/actions/swapchains
  -> platform XR adapter
  -> mclone-xr-host OpenXrFrameDriver / mclone-xr-graphics swapchain glue
  -> mclone-scene shared session/runtime/UI orchestration
  -> explicit per-eye render views + targets
  -> mclone-render
```

Desktop XR and Android XR should diverge only at runtime discovery, Android loader/activity glue, packaging, headset wake/restore, and other true platform concerns.
The shared driver owns OpenXR poll/wait/begin/skip/end ordering and timing facts;
platform handlers own event pumping, target acquisition/render callbacks, and
presentation of outcomes.

Neutral tracked-controller snapshots and hand identity belong to
`mclone-input`. Neutral view pose/FOV/projection contracts belong to
`mclone-render-session`. `mclone-xr-host` translates OpenXR values into those
contracts; `mclone-scene` does not depend on OpenXR or winit. Window surface
support in `mclone-render` is feature-gated and enabled by the desktop app,
not by shared scene consumers.

## Core Rules

- Keep gameplay, runtime, asset, mesh, UI, renderer, and XR contracts host-neutral.
- Keep `winit`, Android activity glue, browser glue, and OpenXR session/swapchain ownership in app/platform adapters.
- Keep renderer-facing view/projection and render-target data explicit.
- Keep headless/offscreen validation available.
- Use `reference/minecraft-1.17.1/src/` as the source of truth for vanilla gameplay, assets, rendering semantics, and visual correctness; see [`reference-minecraft.md`](reference-minecraft.md).
- Use `~/code/playbox` as a native app/render/XR pattern library only; see [`platforms.md`](platforms.md#reference-engine).

## Current Alignment Work

The platform bring-up arc is broad enough that the highest-value work is shared feature parity and boundary consolidation, not more app scaffolding.

Current alignment areas:

- drive shared client-experience policy through
  [`client-experience-architecture.md`](client-experience-architecture.md)
- keep the platform parity/contract matrix in [`topics/platform-parity.md`](topics/platform-parity.md) current
- keep host-mode convergence shared across web, desktop, Android, and XR lanes
- keep XR terrain, actor, UI, and comfort features in shared XR crates where practical
- advance lighting correctness, rendering parity, menu/HUD/options/loading UI, and persistence as platform-neutral features
- add adapter conformance tests for render targets/views, asset discovery, input intent mapping, and render-section compile contracts

## Historical Notes

The older transition-era roadmap is archived at [`archive/native-rewrite-roadmap.md`](archive/native-rewrite-roadmap.md). It is historical context, not active implementation guidance.
