# Client Experience Architecture

Status: accepted rulebook; tactical
[`143-client-experience-convergence-burn-down.md`](tactical/143-client-experience-convergence-burn-down.md)
closed 2026-07-05. Opened 2026-07-05 after tactical
[`141-flat-client-platform-policy-convergence.md`](tactical/141-flat-client-platform-policy-convergence.md)
made enough desktop/web flat policy shared to expose the larger target: one
client experience core across flat, XR, web, Android, offscreen, and emulated
test profiles. Revised 2026-07-05 after a code audit of all four
`GameUiAction` dispatch sites and XR session ownership; the audit results are
recorded in [Current Violations](#current-violations-burn-down) and drove the
migration order and enforcement rules below.

Adopted stance (2026-07-05): convergence and enforcement take priority over
new user-facing feature work. Tactical 143 closed the initial convergence
burn-down; future feature tacticals must route through the shared core from
their first slice rather than being ported later.

The executable checklist and validation log for this work — per-slice
deliverables, non-goals, observable exit criteria, validation blocks, and the
implementing-agent contract — is tactical
[`143-client-experience-convergence-burn-down.md`](tactical/143-client-experience-convergence-burn-down.md).
That tactical is closed; this document stays the durable rulebook.

This document is the architecture target and rulebook for future client
experience work. It is not a claim that all platform parity gaps are closed.
It should guide future tacticals and code review until it is revised or split
into more specific architecture docs.

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
  while desktop and web execute effects through their host adapters;
- offscreen already reuses the desktop driver headless
  (`offscreen_flat_client.rs` wraps `FlatClientDriver`), so desktop flat and
  offscreen are one policy implementation today.

The 2026-07-05 audit added an important conclusion: **most of the divergence
we keep hitting is mechanical duplication, not platform impossibility.** Web
lacks the far-LOD toggle not because browsers cannot do far LOD, but because a
hand-copied match arm was left as `=> {}`. Web adopted the shared catalog and
session controllers without forking policy; the async gap was absorbed as
completion timing. The lane that actually grew a second product is XR, whose
scene crate owns a private session state machine. Web is not the exception
that breaks the model. Discipline is the missing piece, so this revision adds
enforcement, not just intent.

## Current Violations (Burn-Down)

These were the live violations of this architecture as of the 2026-07-05
audit. Tactical 143 closed this acceptance burn-down on 2026-07-05. Line
numbers are anchors from that audit; if a line has moved, the symbol name is
the durable reference. This list remains as the rationale for the enforcement
gates below.

### V1. Four parallel `GameUiAction` dispatch matches

Resolved 2026-07-05 by tactical 143 Slices 1, 2, 4, 6, and 7. At the audit
point, each app lane hand-wrote the same top-level action dispatch:

- desktop: `mclone-native-client/src/flat_client_driver.rs:1169`
  (`apply_ui_action`)
- web: `mclone-web-client/src/web_canvas.rs:3220` (`apply_web_ui_action`)
- XR: `mclone-xr-scene/src/lib.rs:5204` (`apply_xr_ui_action`)
- Android: `mclone-android-client/src/lib.rs:1180` (`apply_ui_action`)

The settings/toggle bodies were copy-pasted per lane: `SetMovementMode`
(desktop `:1303`, web `:3249`, XR `:5305`, Android `:1267`), and likewise
`SetFlySpeed`, `SetMovementSpeed`, `ToggleFullbright`, `ToggleCrosshair`, and
`ToggleFirstPersonPlayer`. This was the half of `GameUiAction` that had not
yet moved behind a shared controller.

Contrast with the desktop winit layer (`mclone-native-client/src/app.rs:534`,
also named `apply_ui_action`): that match delegates all policy to the driver
and only executes returned `FlatClientHostAction` effects — process quit,
frame pacing, world teardown. That two-layer split remains the target shape
for every lane: adapter-layer executor matches are allowed, duplicated policy
arms are not.

### V2. Silent inert arms for visible shared actions — in both directions

Resolved 2026-07-05 by tactical 143 Slices 2, 6, and 7. At the audit point,
the representative issues were:

- Far LOD worked on desktop (`flat_client_driver.rs:1231-1259`) but was a
  silent no-op on web (`web_canvas.rs:3231-3232`).
- `SetXrTurnMode` worked in XR (`mclone-xr-scene/src/lib.rs:5311`) but was a
  silent `=> {}` on desktop (`flat_client_driver.rs:1309`), web
  (`web_canvas.rs:3253`), and Android (`mclone-android-client/src/lib.rs:1276`).
- Catalog CRUD was a `log::warn!` no-op on XR
  (`mclone-xr-scene/src/lib.rs:5362`) and Android
  (`mclone-android-client/src/lib.rs:1324`).

These are representative, not exhaustive: the tripwire grep in
[Enforcement](#enforcement) is the durable gate. Migration slice 1's
classification audit owns the full list.

An action a profile cannot support must surface as a shared capability
projection (hidden, disabled, or visibly unsupported), never as a silent empty
arm. The inert-arm pattern is exactly how lanes drift apart without anyone
deciding they should.

### V3. World-catalog policy re-implemented in TypeScript (third copy)

Resolved 2026-07-05 by tactical 143 Slice 3. At the audit point,
`mclone-web-client/www/mclone-web-world-catalog.ts` re-implemented policy that
Rust already owned in `mclone-app-runtime/src/world_catalog.rs`:

- id length limit: TS `LOCAL_WORLD_ID_MAX_LEN` (`:12`) vs Rust (`:21`);
- error strings: TS `` `local world \`${id}\` already exists` `` (`:82`) and
  `was not found` (`:113`) vs Rust `world_catalog.rs:796` and `:788`;
- the active-delete guard itself: TS re-throws
  `` `cannot delete active local world \`${id}\`; quit to title first` ``
  (`:130`) — a byte-for-byte copy of the guard Rust owns in
  `world_catalog.rs` (`validate_delete_inactive_world`, `:363`);
- id generation: TS `availableLocalWorldIdFromDisplayName` (`:271`) vs Rust
  `available_from_display_name` (`world_catalog.rs:45`);
- world sort order duplicated in TS.

This violated the "async is completion timing, not a policy fork" rule from
tactical 141 in its strongest form: the fork crossed a language boundary,
where drift is hardest to notice. TypeScript must be a dumb IndexedDB
executor; validation, id generation, ordering, and message text belong in
Rust, reachable from wasm before/after the raw storage operation.

### V4. XR forks the session state machine, not just the presentation

Resolved 2026-07-05 by tactical 143 Slices 4, 5, and 7.

At the audit point, `mclone-xr-scene` (8,217-line `lib.rs`) shared the session
*vocabulary*
(`SessionStartRequest`, `ActiveSessionDescriptor`, `StatusOverlay`,
`GameUiAction`, `host_mode`) but none of the shared state machines. It did not
reference `GameSessionCoordinator`, the shared catalog policy controller, or
the client session policy effect helpers at all. Instead it owned:

- a private session-replacement machine (`replace_session_for_request`,
  `lib.rs:3238`);
- a private status projection (`session_status: StatusOverlay`, `lib.rs:1023`,
  fed from shared `starting_message()` but with its own transition rules);
- its own full action dispatch (V1) and no catalog CRUD (V2).

This was the largest single divergence in the tree and the one that compounded
fastest: every session/catalog/status improvement landed twice or drifted.
Merging XR onto the shared session machine was the flagship migration slice
and proved that the core is not flat-shaped.

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

### One Noun, One Owner

When the same conceptual noun — session, catalog, status projection, action
dispatch, loading progress — exists as separately implemented types or state
machines in two or more crates, that is the fork smell, regardless of how the
copies are named. Every policy noun gets exactly one shared owner; other
crates may hold handles, adapters, and executors for it, never a second
implementation. V4 (XR's private session machine) and V3 (the TypeScript
catalog rules) were the original violations that motivated this rule.

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

## Core Execution Model: Sans-I/O Is A Hard Rule

This is the single design rule that makes "one core everywhere" achievable,
and it is a requirement, not a preference. Code in the client-experience core
(controllers, state machines, projections, and the facade that composes them):

- performs no I/O: no filesystem, network, storage, GPU, or platform API
  calls;
- never blocks, sleeps, or waits on channels, locks, threads, or futures;
- never spawns or owns threads, workers, or tasks;
- is never `async` and never awaits — completions arrive later as plain
  method calls carrying data;
- never reads wall-clock or monotonic time and never draws entropy; time,
  timestamps, and generated seeds enter as adapter-provided facts (desktop
  already provides the reroll seed this way);
- depends on no platform crates: no `winit`, `web-sys`, `wasm-bindgen`,
  `openxr`, `ndk`, `rusqlite`, `tokio`, or equivalents;
- is a deterministic function shape: (events, profile facts, completions) in,
  (state, effects) out;
- compiles for `wasm32-unknown-unknown` and is testable with no platform host
  at all.

Why each lane needs this:

- **web**: the browser main thread cannot block, and storage is promise-based.
  A sans-I/O core makes web a first-class peer instead of a carve-out — the
  entire concession web requires is that policy be written in
  effect/completion style, paid once per subsystem at design time instead of
  as a permanent fork.
- **XR**: nothing in the core can stall an OpenXR frame loop, because the core
  never waits on anything.
- **offscreen/emulated/test profiles**: with time and entropy injected as
  facts, the same core runs deterministically under scripted input, which is
  what makes emulation a trustworthy validation shape.
- **desktop**: unaffected — adapters may still complete effects synchronously
  on the same call stack, as the desktop catalog adapter does today.

The core makes no assumption about who owns the loop. Its entry points must be
callable from a `winit` event loop, a `requestAnimationFrame` tick, an OpenXR
frame loop, or a test harness for-loop, in any order the adapter's platform
imposes.

Scope note: this rule governs the client-experience policy layer this document
owns. Engine runtime internals — server tick, worldgen, meshing and render
jobs — have their own executor contracts (native threads on desktop, Web
Workers with shared memory on web) and are owned by the runtime/job
workstreams, not by this rule.

The effect/completion loop, concretely:

```text
core.apply_event(event, profile_facts)
  -> effects

adapter executes effects
adapter later feeds completion/report/error back into core
```

Native can complete some work synchronously. Web can complete the same logical
work after an IndexedDB promise or worker message. XR can complete after
OpenXR lifecycle or controller-action events. Completion timing is adapter
shape, not product policy. This is already proven in the tree: desktop
executes catalog requests synchronously
(`flat_client_driver.rs` `execute_world_catalog_request`) while web feeds the
same `apply_catalog_response` after an IndexedDB promise
(`web_canvas.rs` `applyWorldCatalogResponse`), and both converge on identical
state transitions.

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
- routing from input intents to experience actions, above raw device events;
- routing for the common world interaction intents: ray/pointer target,
  break/place/use, hotbar selection, menu activation;
- player/session/runtime command requests that are independent of how they are
  transported;
- effect records for storage, runtime startup/shutdown, network connect, UI
  screen transitions, audio, haptics, and diagnostics.

The core **composes** vocabularies owned elsewhere; it does not re-own them.
Input intent and capability vocabulary stay in `mclone-input`; screen, widget,
and action vocabulary stay in `mclone-ui`; sound event vocabulary stays in
`mclone-audio`. The core owns the routing, sequencing, and state transitions
that connect those vocabularies into one product behavior. If a slice finds
itself redefining an intent or widget vocabulary inside the core, the
vocabulary crate is the right home and the core should import it.

The corollary guardrail: the core must stay a thin facade over focused
controllers (catalog, session, settings, interaction routing) rather than
becoming one large state machine. A shared god-crate is a better failure mode
than four forks, but it is still a failure mode — it recreates
`FlatClientDriver` gravity one level up.

## What Profiles Should Own

Profiles are data and small policy knobs, not separate apps.

Profile facts are represented as one `ClientExperienceProfile` struct composed
of small typed per-subsystem facts (display class, input capabilities, UI
placement policy, storage capabilities, comfort policy). Not capability
bitsets — bitsets invite platform-identity checks by another name — and not
loose booleans threaded through call sites.

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

### TypeScript Executes, Never Decides

The browser adapter includes TypeScript glue, and that glue is an adapter in
the strictest sense: it may open IndexedDB, run promises, construct workers
and sockets, and shuttle bytes and completions across the wasm boundary. It
may not validate, generate ids, order lists, compose user-facing message
text, or make any other policy decision. If a rule is needed on the web path,
it is implemented in Rust and reached from wasm; the TS side receives a
request it executes verbatim and returns raw results or raw errors. V3 is the
standing violation of this rule.

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

Each unacceptable-divergence bullet becomes a conformance test as the
corresponding policy is adopted into the core. They are written to be
testable; treat this list as the seed of the conformance suite, not as prose.

## Relationship To Existing Crates

Decided ownership (see [Decisions](#decisions-and-open-questions)):

- `mclone-app-runtime`: home for the client-experience facade and
  controllers, because it already owns session, flat catalog, host mode,
  loading projection, and single-view runtime helpers. A dedicated crate
  (`mclone-client-experience` or similar) is created only if XR and Android
  adoption produce real dependency pressure, not preemptively.
- `mclone-ui`: shared widgets, retained UI state, draw lists, hit testing, and
  display-neutral screen/action vocabulary.
- `mclone-input`: device capability and intent vocabulary.
- `mclone-client`, `mclone-server`, `mclone-protocol`, `mclone-net`: world,
  authority, replica, command/update, and transport contracts.
- `mclone-render-session` and `mclone-render`: render-session policy and
  drawing from explicit view/target facts.
- `mclone-scene`: shared client scene/session/UI orchestration for mono, stereo,
  and multiview topologies. OpenXR session/swapchain ownership stays in
  `mclone-xr-host`/platform drivers; neutral controller and view contracts live
  below the scene in `mclone-input` and `mclone-render-session`.

The implementation should not start by creating a large new crate. It should
move policy behind the shared facade in `mclone-app-runtime` while watching
dependencies.

## Migration Direction

The order below is deliberate: it burns down the violations in order of
compounding cost, and it front-loads the XR merge because that fork grows
harder to close with every session/catalog improvement that lands twice.
Desktop remains the first validation lane for each slice, per
[`platforms.md`](platforms.md); that is validation order, not ownership order.
Tactical
[`143-client-experience-convergence-burn-down.md`](tactical/143-client-experience-convergence-burn-down.md)
carried the executable version of these slices (plus an enforcement-baseline
slice 0) with exit criteria and validation blocks; the completed status and
validation log live there.

1. **Facade plus full `GameUiAction` classification (closes V1, V2).**
   Introduce the display-neutral facade in `mclone-app-runtime` (working name
   `ClientExperienceController`), composing the existing
   `client_catalog_policy` and `client_session_policy` helpers. As part of the
   same slice, audit every `GameUiAction` variant and classify it: core
   action, host-effect action, capability-gated (present but unsupported on
   some profiles), or projection-specific. Move the duplicated
   settings/toggle bodies into a shared settings controller behind the
   facade. Replace every silent inert arm with a capability projection.
   Record the classification table in this document when the audit lands.
2. **Desktop and web adopt the facade.** Both already consume the constituent
   controllers, so this is mostly mechanical: the four-hundred-line dispatch
   matches shrink to host-effect execution and completion feeding. The
   remaining app-local arms must be true host actions only (quit process,
   pointer lock, surface rebuild, capture).
3. **Demote web TypeScript catalog glue to a dumb executor (closes V3).**
   Validation, id generation, sort order, and message text route through the
   Rust `world_catalog` module from wasm; `mclone-web-world-catalog.ts` keeps
   only IndexedDB mechanics. Delete the duplicated constants and strings.
4. **XR adopts the shared session machine (closes V4; the flagship slice).**
   Replace `replace_session_for_request` and the private status projection in
   `mclone-xr-scene` with `GameSessionCoordinator`, the shared session
   effects, and the shared session projection, consumed through the same
   facade. XR keeps its runtime factory payloads and world-panel projection.
   This is the slice that proves the core is not flat-shaped.
5. **Emulated-XR desktop profile test (lands with or immediately after 4).**
   Desktop/offscreen drives the title → world list → create/open flow through
   an emulated XR profile: controller-ray-as-pointer, world-panel placement
   facts, no headset. This becomes the standing regression gate for
   display-neutrality.
6. **Flat Android adopts the facade** for menu/session/catalog policy, keeping
   activity, touch, storage-root, and packaging concerns in the app crate.
   (Android's catalog arms were warn no-ops at the audit point, V2.)
7. **XR catalog CRUD through the same controller**, projected onto world-space
   panels, replacing the XR warn no-ops (V2) — persistent worlds land in XR
   through the shared owner, never as an XR-local wiring pass.

Each migration slice must reduce app-local policy, not add a wrapper around
existing duplication, and each slice lands with the conformance tests and
gates from [Enforcement](#enforcement) that cover the policy it moved.

## Guardrails

### Do Not Name Generic Policy Flat

If behavior must be shared by XR, Android, web, offscreen, and flat desktop,
its durable owner should not be named `Flat*`. A flat facade may exist, but the
policy core should be display-neutral. `client_session_policy` is the neutral
shared session action/status owner; `client_catalog_policy` is the neutral
shared catalog action/status owner.

### One Noun, One Owner

See [Naming Model](#one-noun-one-owner). A policy noun implemented in two
crates is a fork even when both copies work.

### Sans-I/O Core

See [Core Execution Model](#core-execution-model-sans-io-is-a-hard-rule). No
I/O, no blocking, no threads, no `async`, no wall-clock or entropy reads in
core policy code.

### TypeScript Executes, Never Decides

See [What Adapters Should Own](#typescript-executes-never-decides). No policy
in browser glue; the wasm boundary is an executor boundary.

### One Action, One Shared Owner — No Inert Arms

Every `GameUiAction` variant has exactly one owner: the shared core (normal
user-facing policy), a platform adapter (true host actions only: quit process,
pointer/mouse lock, surface rebuild, capture, XR session operations), or a
documented capability gate with visible shared "unsupported/pending" state.
No app crate may add a silent `=> {}` arm for a visible shared action. An arm
that cannot be wired yet requires a linked tactical note and a capability
projection in the UI.

### No Parallel Status Strings

User-facing status and error text for shared operations is produced in shared
code. Adapters may contribute platform error details, but only through shared
status/error types. If desktop and web can emit different strings for the
same semantic failure, the status policy has already forked — this includes
strings composed in TypeScript (V3).

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

Web promises and workers are real, but they change when a completion arrives,
not what the policy is. If a platform needs a promise, the shared operation is
represented as pending and completed later. Duplicating an operation's
validation, status, or transition rules in an adapter — Rust or TypeScript —
is a policy fork.

### XR Is Not A Later Port

When defining shared client policy, ask how it projects into XR now. If the
answer is "flat fullscreen overlay," the policy is probably too high in the
stack. Core policy should say "open world list" or "start local world"; the
profile decides whether that appears as a flat menu or a world-space panel.
The migration order enforces this: the XR session merge is slice 4, not the
tail of the plan, and the emulated-XR profile test (slice 5) makes
display-neutrality a standing gate rather than a device-lane afterthought.

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
testable without waiting for every device lane. The sans-I/O rule is what
makes emulation trustworthy: with time and entropy injected as facts, an
emulated profile exercises the identical state machine.

## Enforcement

Intent has not held against the "desktop is the fastest lane" gradient; these
mechanical gates are part of the architecture, not optional hygiene. They are
listed roughly in adoption order.

### Diff footprint tripwire (available now)

If a change touches two or more of the dispatch sites with matching
`GameUiAction` logic, stop and extract the shared owner first unless the
change is purely platform plumbing. Manual audit until it becomes a check:

```bash
rg -n "fn apply_ui_action|fn apply_web_ui_action|fn apply_xr_ui_action" \
  native/apps/mclone-native-client/src native/apps/mclone-web-client/src \
  native/apps/mclone-android-client/src native/crates/mclone-xr-scene/src

# silent inert arms for visible shared actions
rg -n "GameUiAction::[A-Za-z]+(\(_\))? => \{\}" \
  native/apps native/crates/mclone-xr-scene/src

# policy strings in browser glue
rg -n "already exists|was not found|cannot delete" \
  native/apps/mclone-web-client/www
```

### wasm32 compile gate for policy crates (available now)

The sans-I/O rule has a cheap mechanical proxy: shared policy code must build
for the browser target in isolation, so a platform dependency added to a
policy module fails fast with a clear owner instead of deep inside the web
app build:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime \
  --target wasm32-unknown-unknown --all-targets
```

`--all-targets` is intentional: `mclone-app-runtime` also ships native-only
diagnostic binaries (for example `terrain_texture_coverage`), and those
binaries keep their native implementation behind target cfgs so the wasm gate
still catches target-gating drift. This passes today (verified 2026-07-07,
including its `mclone-server` dependency chain).

Add this as a named `pnpm` script alongside the existing `native:web:*` lanes
and run it in the same gate set as `cargo test -p mclone-app-runtime`. When a
dedicated experience crate exists, it inherits this gate.

### Conformance tests per action family (lands with each migration slice)

Every shared action family gets fake-adapter tests in the shared crate,
covering at minimum (carried forward from tactical 141):

- list, select, create, open, delete, and delete-confirm flow;
- duplicate display name / duplicate id errors;
- delete-active rejection;
- storage unavailable / read-only catalog;
- async pending operation followed by success, and followed by failure;
- create/open queuing a shared session request;
- active session requiring teardown before replacement;
- quit-to-title clearing active state before CRUD re-enables;
- settings/toggle round-trips once the settings controller exists;
- capability-gated actions projecting visible unsupported/pending state.

Platform adoption is a thin smoke layer over these, never a second source of
policy truth. Each "unacceptable divergence" bullet above gets a conformance
test as its policy is adopted.

### Inert-arm and string-drift checks (after slice 1)

Once slice 1 lands, the tripwire greps graduate into a small script or test
that fails CI when a visible shared action has a silent empty arm in any app
crate, or when catalog/session policy strings appear outside the shared
owners. Candidate later hardening: `clippy.toml` disallowed methods
(`std::thread::spawn`, `std::fs`, `std::time::Instant::now`,
`std::time::SystemTime::now`, blocking channel receives) scoped to the policy
modules, once the facade gives those modules a stable boundary.

### Parity matrix discipline (standing)

When a slice changes user-visible support or contract adoption, update
[`topics/platform-parity.md`](topics/platform-parity.md) in the same change
and link the tactical. Matrix 2's ⚑/✗ cells are the standing fork ledger; the
definition of done in that document applies to every slice here.

## Out Of Scope, Adjacent Workstreams

Adopting the client-experience core does not close these; they are executor-
and renderer-layer convergence tracked elsewhere, listed so nobody mistakes
this document for their plan:

- the web inline render path and `frame_render` reimplementation (the ⚑ forks
  in parity Matrix 2), which should converge on the shared full-frame render
  path;
- the browser job/worker lifecycle on `SharedArrayBuffer` shared memory —
  browser CPU work converges on the same job/scheduler contracts as desktop,
  with inline synchronous WASM paths remaining temporary fallbacks behind the
  same interfaces;
- web audio adoption of `mclone-audio`;
- text entry / IME and the connect-screen UI, owned by the `mclone-ui` text
  model workstream, though its actions route through this core when they
  exist.

## Decisions And Open Questions

Decided in this revision (2026-07-05):

- **Naming**: the `ClientExperience*` family. `GameClientController` is
  rejected because "client" already names the replica/prediction crate
  (`mclone-client`) and overloading it invites confusion. The concept is the
  `ClientExperienceCore`; the first concrete artifact is a facade controller
  in `mclone-app-runtime`.
- **Home**: `mclone-app-runtime`, as a module family, until XR and Android
  adoption create measured dependency pressure; only then consider a
  dedicated crate.
- **Profile representation**: one `ClientExperienceProfile` struct of small
  typed per-subsystem facts. No capability bitsets, no platform-identity
  enum consulted by policy code.
- **First XR slice**: replace the private XR session-replacement machine and
  status projection with the shared coordinator and session effects
  (migration slice 4). Emulated-XR menu flow (slice 5) is its regression
  gate.
- **Execution model**: sans-I/O core, enforced per
  [Enforcement](#enforcement).
- **Priority stance**: convergence and enforcement before new user-facing
  feature work, per the status note.
- **Facade entry point**: `ClientExperienceController::apply_ui_action`
  routes one `GameUiAction` into per-family effects. The facade composes the
  existing `client_catalog_policy` controller, the existing
  `client_session_policy` helper functions, and the new settings controller
  without moving adapter execution into shared code.

`GameUiAction` classification, produced by migration slice 1:

| Variant | Classification | Owner / notes |
|---|---|---|
| `StartWorld` | projection-specific | UI projection closes the start/title surface; adapters may pair pointer focus. |
| `EnterScenario` | host-effect action | Shared scenario controller emits a path-free launch intent; native scene policy executes it, while unsupported profiles suppress projection with a reason-bearing capability result. |
| `OpenWorldList` | core action | Catalog controller. |
| `OpenWorldCreate` | core action | Catalog controller; adapter supplies the seed fact. |
| `SelectWorld` | core action | Catalog controller. |
| `OpenWorld` | core action | Catalog controller; completion can emit a session start. |
| `CreateCatalogWorld` | core action | Catalog controller. |
| `ConfirmDeleteWorld` | core action | Catalog controller. |
| `DeleteWorld` | core action | Catalog controller. |
| `CancelDeleteWorld` | core action | Catalog controller. |
| `OpenNewWorld` | core action | Session helper. |
| `OpenJoinRemote` | core action | Session helper. |
| `RerollSeed` | core action | Session helper; adapter supplies the seed fact. |
| `CreateWorld` | core action | Session helper emits a local session start request. |
| `JoinRemote` | core action | Session helper emits a remote session start request. |
| `Resume` | projection-specific | UI projection returns to gameplay; adapters may pair pointer focus. |
| `OpenBlockPalette` | projection-specific | UI projection. |
| `OpenHelp` | projection-specific | UI projection. |
| `CloseHelp` | projection-specific | UI projection. |
| `AssignHotbarBlock` | core action | Facade emits a gameplay command effect for the adapter to execute. |
| `OpenOptions` | projection-specific | UI projection. |
| `OpenServerSettings` | projection-specific | UI projection. |
| `BackToTitle` | core action | Session helper clears inactive session status; UI projection owns screen change. |
| `BackToPause` | projection-specific | UI projection. |
| `QuitToTitle` | core action | Session helper emits teardown/quit-to-title host effect. |
| `ToggleSectionOcclusion` | core action | Settings controller. |
| `ToggleFullbright` | core action | Settings controller. |
| `ToggleFarLod` | capability-gated | Settings controller; unsupported profiles project shared unavailable state. |
| `SetFarLodRange` | capability-gated | Settings controller; unsupported profiles project shared unavailable state. |
| `TogglePlayerCollisionBox` | core action | Settings controller. |
| `ToggleFirstPersonPlayer` | core action | Settings controller. |
| `ToggleCrosshair` | capability-gated | Settings controller; hidden/unsupported profiles project shared unavailable state. |
| `ToggleFramePipelineOverlay` | capability-gated | Settings controller; desktop flat/offscreen profiles support the shared diagnostics overlay, while web/Android/XR profiles project unavailable state until their report sink or world-panel projection exists. |
| `SetPlayerModel` | core action | Settings controller emits player-appearance sync effect. |
| `SetMovementMode` | core action | Settings controller. |
| `SetXrTurnMode` | capability-gated | Settings controller; non-XR profiles project shared unavailable state. |
| `CycleFramePacing` | capability-gated | Settings controller emits a host-executed frame pacing effect. |
| `CycleFpsCap` | capability-gated | Settings controller emits a host-executed FPS-cap effect. |
| `SetRenderDistance` | capability-gated | Settings controller clamps and emits runtime render-distance effect. |
| `SetFlySpeed` | core action | Settings controller. |
| `SetMovementSpeed` | core action | Settings controller. |
| `SetTouchLookSensitivity` | capability-gated | Settings controller; non-touch profiles project shared unavailable state. |
| `SetTouchControlsMode` | capability-gated | Settings controller; non-touch profiles project shared unavailable state. |
| `SetServerSimulationCadence` | capability-gated | Settings controller validates cadence and emits local-server cadence effect. |
| `Quit` | host-effect action | Session helper emits process-quit host effect. |

Slice 5 recorded the emulated-XR facts needed for the menu/session gate:

- a synthetic stereo view pair/head pose used to place the XR world panel;
- an identity stage-to-world transform for deterministic desktop/offscreen
  tests;
- a right-hand controller aim ray plus trigger press/release hysteresis;
- persistent catalog capabilities and injected list/create/open completions;
- a deterministic New World seed supplied by the test adapter.

The gate did not need comfort fades, snap-turn increments, swapchains, stereo
render targets, or a headset. Those remain covered by XR-specific unit tests
and device smokes.

## Acceptance Criteria

This document's target shape is reached when:

- a shared client-experience facade owns catalog, session, and
  settings/toggle action dispatch, and every `GameUiAction` variant has a
  recorded classification;
- the four app dispatch sites contain only true host-effect arms and
  completion feeding — measured by the tripwire greps returning no shared
  policy in app crates;
- `mclone-web-world-catalog.ts` contains no validation, id-generation,
  ordering, or message-text policy;
- `mclone-xr-scene` no longer defines a private session-replacement machine
  or status projection, and XR session flows are described in the same
  core action/effect vocabulary as flat;
- no visible shared action has a silent inert arm on any lane — unsupported
  actions project shared capability state instead;
- the wasm32 policy-crate gate and the slice-1 conformance suites run in the
  standard validation set;
- the emulated-XR desktop profile test drives the shared menu/session flow
  without a headset;
- parity Matrix 2 rows for the experience contracts point at this document
  and show adoption (not fork) for desktop, web, Android, and XR lanes.
