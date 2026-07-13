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
- the shared cross-platform scene/session/UI/orchestration host
  (`mclone-scene`)
- one shared create/open/join session-start planner (`mclone-app-runtime`),
  producing typed local/remote runtime plans and active-session descriptors
- one label-parameterized native TCP remote-session adapter
  (`mclone-app-runtime`); platform factories still choose endpoint and label
- client frame-pipeline accounting, neutral queue/peer builders, and shared
  report presentation (`mclone-app-runtime`, over the leaf
  `mclone-diagnostics` schema/math)
- adaptive client render-section admission (`mclone-scene`), consuming the
  shared frame report and timed sync costs; platform drivers provide only their
  target frame period, while an XR upload cap is an optional grant clamp
- one exhaustive native client-experience settings dispatcher
  (`mclone-scene`); thin `HostEffects` adapters own mouse lock, desktop frame
  pacing requests, touch-mode forwarding, and host/session exit requests
- surface-neutral frame-pacing/timing snapshots and debug-overlay aggregation
  (`mclone-app-runtime`); the winit pacing driver remains desktop-app-local
- shared Rust/WebGPU UI model and draw list

Platform app crates own:

- event loops and lifecycle glue
- desktop window, Android activity, browser canvas, or OpenXR session ownership
- surface/swapchain acquisition and presentation pacing
- platform input collection and translation
- platform storage, transport setup, package scripts, and device validation

## Host Shapes

All display clients share one `mclone-scene::McloneSceneHost`, configured with
`McloneSceneHostOptions`, behind thin cadence and surface drivers (Tacticals
168 and 170). Flat hosts use Mono; the headset-free gate uses synthetic Stereo:

```text
platform input/lifecycle
  -> platform adapter
  -> mclone-scene shared session/runtime/UI orchestration
  -> explicit Mono(view) or Stereo([view; 2]) targets
  -> mclone-render
```

The host owns one direct active `DrawableWorldSlot` and one optional detached
standby slot. Each aggregate retains stable world identity, descriptor,
storage and lifecycle facts, the asset epoch, canonical scene options,
runtime/startup state, camera, interaction/player model, terrain draw store,
traversal readiness, upload coordinator, Far LOD state, render statistics,
accepted entry pose, and any detached CPU startup seed. All existing frame
methods still address `active_world` directly. All initial, local-completion,
external/web-completion, and native-replacement paths stage the same
target-neutral core install aggregate before publication. Shared assets and
renderers, physical presentation state, UI/session coordination, clocks, and
numerical budgets remain on `McloneSceneHost`.

This is Tactical 174 Slice 5's bounded two-world ownership boundary, not a
product multi-world manager: there is no registry, world-id lookup, selection
branch in ordinary frames, gate, or simultaneous rendering. Without the
launch-only standby request both optional owners are `None`; no second server,
renderer shell, or startup work is constructed. With the request, the detached
runtime advances
to acknowledged-pose/CPU-seed/endpoint readiness, then reuses the active slot's
extracted preparation logic under explicit one-result/compile/upload caps. The
active slot always prepares first. The standby publishes `Switchable` only
after its initial upload lifecycle is conserved and drained, entry-support
terrain is GPU-resident and traversal-ready, and any required multiview terrain
renderer is materialized. It remains invisible; atomic selection is exposed
only through a shared scene command and scripted smoke.
That command reconciles and maps the destination camera/interest through the
paired terrain-relative endpoints, restores runtime cadence, then exchanges
the two complete slot values at a frame boundary. The old active becomes the
switchable return slot. Selection clears only host presentation caches; it
does not rebuild a runtime, draw store, traversal cache, upload coordinator, or
renderer. The first selected frame uses a one-request/result/upload preparation
cap, after which ordinary active budgets resume. Interactive selection waits
for Tactical 174's opaque-gate slice.

Retained-slot lifecycle follows complete ownership. Backgrounding attempts to
flush both slot runtimes; a remote runtime remains a no-op because its server
owns persistence. Cancellation takes and drops the entire standby slot before
changing diagnostics, synchronously joining its owned native runner/compiler
and releasing its terrain resources. Asset replacement performs that drop
before committing a new epoch. A surface/device render-resource rebuild also
cancels the standby before creating active resources against the replacement
device; retained-world GPU migration is not implicit.

Shared launch values enter through
`mclone-app-runtime::startup_args::StartupOptions`. Its nested
`StartupSceneOptions` is the canonical retained scene/session configuration:
platforms collect argv, Android launch properties, or browser query values,
then carry that record without restating its fields. Desktop `SceneOptions`
and `McloneSceneHostOptions` nest the canonical value beside desktop/harness or
scene-host policy. Storage remains an unresolved intent until a platform
projects its root/resource, while render and initial-camera options remain
canonical siblings rather than fields copied into app DTOs.

The browser keeps parsed `StartupOptions` behind opaque wasm-bindgen
`WebStartupConfig`. TypeScript receives only a narrow browser plan for choosing
local worker versus remote WebSocket and presenting initial render state; both
scene constructors return the same handle to Rust. Shared fields, defaults,
and validation must not become TypeScript interfaces or positional wasm
parameters.

`mclone-scene::SceneCameraConfig` is the single application boundary for
launch-time camera policy. It applies movement speed, first-person visibility,
and reducer-normalized movement/collision to initial local, remote, XR, and
mono cameras. Replacement paths use the same factory and explicitly preserve
runtime-adjusted speed where required. Platform adapters must not apply shared
startup camera setters themselves.

This covers desktop flat, offscreen flat, headset-free XR emulation, flat
Android, and the browser canvas. The browser's `WebFrameDriver` owns rAF,
canvas/surface acquisition, DOM input, promise execution, browser resource
fetching, and presentation, while its wasm-bindgen `WebSceneHost` wrapper owns
only concrete WebGPU targets and browser service adapters around the same
`McloneSceneHost`. Local worker, IndexedDB local-world, and remote WebSocket
sessions do not introduce another scene-policy owner.
The live desktop flat path now reaches this boundary through the app-local
`WinitFrameDriver`: winit owns redraw cadence, surface acquisition,
keyboard/mouse translation, mouse lock, and final presentation, while
`mclone-scene` owns session startup/replacement, runtime polling, camera/input
application, render admission/sync/upload, traversal, actors, world overlays,
HUD/menu/status assembly, and frame-accounting feedback. Offscreen/perf drive
the same host through `OffscreenDriver`, which owns Mono or synthetic Stereo
targets but no scene policy. Flat Android reaches it
through `AndroidSurfaceDriver`: the app retains `NativeActivity` lifecycle,
Vulkan surface targets, raw input translation, startup properties, and fixed
FIFO cadence facts; the host owns the same session/runtime/render/UI policy as
desktop. Tactical 168 Slices 7c–8 record these migrations.
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
The shared `mclone-xr-host::OpenXrFrameDriver` owns OpenXR
poll/wait/begin/skip/end ordering and timing facts;
platform handlers own event pumping, target acquisition/render callbacks, and
presentation of outcomes.

Neutral tracked-controller snapshots and hand identity belong to
`mclone-input`. Neutral view pose/FOV/projection contracts belong to
`mclone-render-session`. `mclone-xr-host` translates OpenXR values into those
contracts; `mclone-scene` does not depend on OpenXR or winit. Window surface
support in `mclone-render` is feature-gated and enabled by the desktop app,
not by shared scene consumers.

The executable `pnpm native:thin-adapters:purity` source gate protects this
boundary across the native apps and browser driver. It composes scene-host,
OpenXR-frame-driver, and enforced web-adoption source checks. `AGENTS.md`
carries the same shared-first ownership rule; no Tactical 168 or 170 guidance
depends on app-local orchestration.

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
