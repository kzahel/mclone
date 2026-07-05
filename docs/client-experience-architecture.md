# Client Experience Architecture

Status: draft for review. Opened 2026-07-05 after tactical
[`141-flat-client-platform-policy-convergence.md`](tactical/141-flat-client-platform-policy-convergence.md)
made enough desktop/web flat policy shared to expose the larger target: one
client experience core across flat, XR, web, Android, offscreen, and emulated
test profiles.

This document is a vision and naming draft, not a completed implementation
claim. It should be used to guide future tacticals and code review until it is
accepted, revised, or split into more specific architecture docs.

Related docs:

- [`platforms.md`](platforms.md) owns current platform lanes and validation.
- [`topics/platform-parity.md`](topics/platform-parity.md) owns feature and
  shared-contract parity matrices.
- [`native-engine-architecture.md`](native-engine-architecture.md) owns durable
  Rust crate/app ownership.
- [`architecture.md`](architecture.md) owns broad simulation/runtime/host
  architecture.
- [`tactical/061-shared-engine-web-adapter-refactor.md`](tactical/061-shared-engine-web-adapter-refactor.md),
  [`tactical/067-shared-render-worker-architecture.md`](tactical/067-shared-render-worker-architecture.md),
  [`tactical/084-single-view-platform-alignment.md`](tactical/084-single-view-platform-alignment.md),
  [`tactical/095-shared-session-coordinator.md`](tactical/095-shared-session-coordinator.md),
  [`tactical/105-offscreen-flat-client-host.md`](tactical/105-offscreen-flat-client-host.md),
  [`tactical/123-ui-v2-menu-rebuild.md`](tactical/123-ui-v2-menu-rebuild.md),
  [`tactical/134-shared-persistence-architecture.md`](tactical/134-shared-persistence-architecture.md),
  [`tactical/136-world-catalog-and-crud-ui.md`](tactical/136-world-catalog-and-crud-ui.md),
  and [`tactical/141-flat-client-platform-policy-convergence.md`](tactical/141-flat-client-platform-policy-convergence.md)
  are the immediate history.

## Desired Shape

Mclone should feel like one game client with different presentation and host
profiles, not several platform-specific clients that happen to share some
crates.

The target is:

```text
ClientExperienceCore
  shared policy, state machines, commands, projections, and effects

Profiles
  desktop flat, web flat, flat Android, desktop XR, Android XR,
  offscreen/headless, XR-emulated-on-desktop/test

Platform Adapters
  winit/window/surface, browser canvas/workers/IndexedDB,
  Android activity/storage, OpenXR sessions/swapchains/controllers,
  headless frame sinks and scripted input sources
```

The core should be broad enough that desktop could emulate an XR profile for
tests, or offscreen could exercise the same session/menu/interaction logic
without a window. The adapters should be narrow enough that platform-specific
constraints do not force duplicated gameplay, session, catalog, UI action, or
runtime policy.

## Why This Exists

We keep hitting the same friction:

- desktop flat is the fastest lane, so policy tends to land there first;
- web has real shape differences: promises, workers, IndexedDB, TypeScript
  glue, `SharedArrayBuffer`, browser lifecycle;
- XR has real shape differences: stereo views, world-space panels,
  controller-ray interaction, comfort constraints, OpenXR swapchains;
- Android has real lifecycle, storage, package, and device constraints;
- offscreen/headless wants the same client behavior without a user-facing
  window or headset.

Those differences are legitimate adapter differences. They are not a good
reason to duplicate user-facing policy. The success pattern from earlier work
is to share the logical policy and keep transport/executor/storage details
behind adapters.

Examples already in the codebase:

- render compile converges on a shared logical `RenderSectionCompiler` while
  desktop uses Rust channels and web uses a browser worker transport;
- session coordination shares request/state/failure vocabulary while each host
  owns runtime factory details;
- persistence uses shared logical requests/completions while native uses
  filesystem/SQLite and web uses IndexedDB;
- catalog/session UI action policy now lives in `mclone-app-runtime` helpers
  while desktop and web execute effects through their host adapters.

## Naming Model

These terms are intentionally precise.

| Term | Owns | Does not own |
|---|---|---|
| **Core** | platform-neutral client experience policy and state machines | OS/browser/XR/Android resources |
| **Profile** | selected capability/display/input/runtime shape | host API calls or device handles |
| **Adapter** | platform facts, resources, async completion, lifecycle, packaging | user-facing policy |
| **Runtime** | running world/client/server/render execution resources | menu/catalog/session policy unless explicitly shared |
| **Controller** | deterministic policy state machine that emits effects | direct storage/runtime/window/worker/swapchain work |
| **Driver** | app-local bridge that owns platform resources and executes effects | durable cross-platform game policy |

Current names do not always match this model. For example,
`FlatClientDriver` is a useful desktop/offscreen adapter wrapper, but it should
not be the durable owner for behavior needed by web, Android, or XR.

## Layer Topology

The durable topology should be:

```text
raw platform events / async completions / runtime reports
  -> PlatformAdapter
  -> ClientExperienceCore
  -> profile-specific projection
  -> shared UI/render/input/session contracts
  -> PlatformEffects
  -> PlatformAdapter executes host work
```

The same core should be able to run with different profiles:

```text
desktop flat profile
  core -> flat overlay projection -> winit/surface adapter

web flat profile
  core -> flat overlay projection -> canvas/worker/IndexedDB adapter

Android flat profile
  core -> flat/touch projection -> NativeActivity/storage adapter

desktop XR profile
  core -> XR world-panel projection -> OpenXR desktop adapter

Android XR profile
  core -> XR world-panel projection -> Quest/OpenXR Android adapter

offscreen profile
  core -> automation/headless projection -> frame sink/script adapter

desktop XR-emulation profile
  core -> XR projection with emulated poses/actions -> desktop/offscreen adapter
```

Flat and XR should not be separate product logic. They are different display
and input projections of the same game-client experience.

## What The Core Should Own

The core should own policy that is the same product behavior across profiles:

- local world create/open/delete semantics;
- active-world delete guards;
- local/remote session request, start, success, failure, and quit transitions;
- status text and high-level loading/session projection;
- shared `GameUiAction` semantics when the action is user-facing rather than
  host-resource-specific;
- capability projection for actions that are unavailable, read-only, pending,
  or unsupported;
- input intent vocabulary and action routing above raw device events;
- common world interaction intent shape: ray/pointer target, break/place/use,
  hotbar selection, menu activation;
- player/session/runtime command requests that are independent of how they are
  transported;
- effect records for storage, runtime startup/shutdown, network connect, UI
  screen transitions, audio, haptics, and diagnostics.

The core should use an effect/completion model:

```text
core.apply_event(event, profile_facts)
  -> effects

adapter executes effects
adapter later feeds completion/report/error back into core
```

Native can complete some work synchronously. Web can complete the same logical
work after an IndexedDB promise or worker message. XR can complete after
OpenXR lifecycle or controller-action events. Completion timing is adapter
shape, not product policy.

## What Profiles Should Own

Profiles are data and small policy knobs, not separate apps.

Profiles should describe:

- display class: flat, stereo XR, offscreen/headless;
- input class and capabilities: keyboard/mouse, touch, controller rays,
  emulated XR poses, scripted input;
- UI placement policy: fullscreen overlay, touch overlay, world-space panel,
  automation report;
- comfort policy: snap turn, teleport/push locomotion availability, XR fades;
- default runtime choices: local integrated, remote dedicated, transient
  smoke, persistent catalog;
- validation expectations and smoke gates.

Profiles should not directly own:

- storage implementation;
- worker creation;
- OpenXR swapchain ownership;
- `winit` event loop;
- browser DOM or TypeScript glue;
- gameplay/session/catalog rules.

## What Adapters Should Own

Adapters own real host resources and platform constraints:

- desktop `winit`, window focus, cursor lock, surface acquisition, frame
  pacing, desktop diagnostics;
- browser canvas, `wasm-bindgen`, TypeScript event bridge, Web Workers,
  `SharedArrayBuffer` setup, IndexedDB promises, WebSocket construction;
- Android activity lifecycle, app-private storage roots, asset staging, Vulkan
  surface lifecycle, touch event collection;
- OpenXR runtime/session/swapchain/action-set ownership, per-eye frame loops,
  controller pose collection, headset validation;
- offscreen targets, readback, PNG/video/network/model frame sinks, scripted
  input sources;
- platform-specific validation harnesses and launch configuration.

Adapters may translate raw platform facts into core facts. They should not
invent independent user-facing state machines when the behavior should be the
same across profiles.

## Runtime Unification Standard

"Same runtime" does not require one concrete Rust struct that owns every
resource on every platform. Browser workers, OpenXR swapchains, Android
activity lifecycle, and desktop windows cannot literally be the same object.

The standard is:

- the same logical runtime graph;
- the same command/update/session contracts;
- the same client-world and interaction semantics;
- the same render-session policy;
- the same session/catalog/menu policy;
- different executors, transports, storage adapters, and presentation targets.

Acceptable divergence:

- desktop uses native OS threads while web uses workers;
- desktop transport uses Rust channels or TCP while browser transport uses
  worker messages, shared memory, or WebSocket;
- native persistence uses filesystem/SQLite while web uses IndexedDB;
- flat rendering targets one view while XR renders per-eye or multiview;
- XR menus use world-space panels while flat menus use fullscreen overlays.

Unacceptable divergence:

- desktop and web implement separate active-delete rules;
- XR and flat implement different session-start failure semantics;
- Android and desktop produce different status strings for the same shared
  failure;
- one profile silently lacks a visible action because an app crate added an
  inert match arm;
- web reimplements render/session scheduling policy because its ABI is
  different.

## Relationship To Existing Crates

Current likely ownership:

- `mclone-app-runtime`: near-term home for the draft
  `ClientExperienceCore` because it already owns session, flat catalog, host
  mode, loading projection, and single-view runtime helpers.
- future dedicated crate, if dependency pressure warrants it:
  `mclone-client-experience` or similar, owning profile-neutral
  session/catalog/menu/action policy.
- `mclone-ui`: shared widgets, retained UI state, draw lists, hit testing, and
  display-neutral screen/action vocabulary where practical.
- `mclone-input`: device capability and intent vocabulary.
- `mclone-client`, `mclone-server`, `mclone-protocol`, `mclone-net`: world,
  authority, replica, command/update, and transport contracts.
- `mclone-render-session` and `mclone-render`: render-session policy and
  drawing from explicit view/target facts.
- `mclone-xr-scene`: XR-specific projection/scene shell, but not a separate
  product policy fork.

The implementation should not start by creating a large new crate. It should
first move policy behind a shared facade while watching dependencies.

## Migration Direction

Near-term migration should be incremental:

1. Rename the target shape from "flat controller" to a generic
   `ClientExperienceCore` / `ClientExperienceController` concept.
2. Keep existing `flat_client_catalog` and `flat_client_session` helpers as
   building blocks while adding a small facade that composes them.
3. Adopt the facade in desktop flat first because `FlatClientDriver` currently
   has the most policy gravity.
4. Adopt the same facade in web without changing browser storage/worker
   ownership.
5. Route flat Android through the same facade for menu/session/catalog policy.
6. Route XR scene/menu panels through the same core, with an XR projection
   layer rather than flat overlay assumptions.
7. Add offscreen and emulated-XR profile tests so desktop can exercise XR-like
   input/action paths without requiring a headset for every regression.

Each migration slice should reduce app-local policy. It should not just add a
new wrapper around existing duplication.

## Guardrails

### Do Not Name Generic Policy Flat

If behavior must be shared by XR, Android, web, offscreen, and flat desktop,
its durable owner should not be named `Flat*`. A flat facade may exist, but the
policy core should be display-neutral.

### Capabilities Before Platform Branches

Prefer profile/capability facts over `if desktop`, `if web`, or `if xr`:

- has persistent local catalog;
- supports text entry;
- supports controller ray;
- supports pointer lock;
- supports world-space panels;
- supports filesystem world roots;
- supports async storage completions.

Platform branches belong at adapter boundaries only.

### Async Is Completion Timing

Web promises and workers are real, but they should change completion timing,
not create a second policy implementation.

### XR Is Not A Later Port

When defining shared client policy, ask how it projects into XR now. If the
answer is "flat fullscreen overlay," the policy is probably too high in the
stack. Core policy should say "open world list" or "start local world"; the
profile decides whether that appears as a flat menu or a world-space panel.

### Desktop Is A Fast Lane, Not The Architecture

Desktop flat remains the fastest validation lane. That should make it the
first adopter for many shared slices, not the permanent owner.

### Emulation Is A First-Class Validation Shape

Desktop/offscreen should be able to emulate profile facts:

- XR controller rays and poses;
- touch input sequences;
- no-window frame sinks;
- local/remote session changes;
- world catalog action flows.

Emulation does not replace device validation, but it keeps profile behavior
testable without waiting for every device lane.

## Review Questions

- What should the durable name be: `ClientExperienceCore`,
  `ClientExperienceController`, `GameClientController`, or something else?
- Should the first facade live in `mclone-app-runtime`, or is a new crate
  justified once both flat and XR consume it?
- Which `GameUiAction` variants are display-neutral core actions, and which
  are flat/XR projection actions?
- How should profile facts be represented: one `ClientExperienceProfile`
  struct, capability bitsets, or smaller typed facts per subsystem?
- What is the first XR adoption slice that proves the core is not flat-shaped?
- Which desktop/offscreen emulation tests should become mandatory before
  touching profile-shared policy?

## Near-Term Acceptance Criteria

This draft becomes actionable when the next implementation slices can say:

- a shared client-experience facade owns catalog/session action dispatch;
- desktop flat consumes it through `FlatClientDriver` with less app-local
  policy than today;
- web consumes the same facade through async IndexedDB/worker completions;
- at least one XR menu/session path is described in terms of the same core
  action/effect vocabulary;
- platform parity matrix rows point to this document for the shared-core
  target, not to one-off desktop/web tactical decisions.
