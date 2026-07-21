# Shared Platform Host Boundary

Topic: `platform-host-boundary`

Status: direction accepted and implementation in progress 2026-07-21.
Tacticals
[`203`](../tactical/203-shared-interactive-router-native-adoption.md),
[`204`](../tactical/204-browser-raw-input-adoption.md), and
[`205`](../tactical/205-browser-diagnostic-observer-isolation.md) completed the
shared synchronous interactive route, adopted it on desktop, flat Android,
and browser raw input, and removed the semantic diagnostic mirror from the
ordinary browser adapter. Active Tactical
[`206`](../tactical/206-browser-preferences-and-bootstrap-policy.md) owns the
remaining browser preference and initial-bootstrap policy seams. The existing
shared scene, input, actor/mailbox, rendering, and platform-operation contracts
remain the foundation; this series is converging the host rim without
replacing them with a new universal framework.
A code-grounded verification pass on 2026-07-21 confirmed the audit against
the current code and extended it with the browser-Rust dispatch surface, the
desktop frame-driver dispatch role, the XR emulation path, and the
undocumented browser input behaviors Stage 0 must record.

## Top-Level Frame

> What is the smallest clean platform-host boundary that lets desktop,
> browser, flat Android, XR, and test hosts drive one shared Rust engine without
> duplicating input, UI, lifecycle, or gameplay decisions, while still letting
> each platform initialize and operate its environment naturally?

There are two equally important goals:

1. TypeScript and other leaf platform glue must not understand game actions or
   advance engine state machines.
2. Removing that knowledge must produce one clean shared Rust structure, not
   relocate the same duplication into web-specific Rust or preserve parallel
   native/web dispatch paths.

The desired shape is:

```text
native winit / Android / OpenXR facts       browser DOM/API facts
                 |                                  |
                 | thin physical conversion         | thin forwarding/conversion
                 +----------------+-----------------+
                                  |
                                  v
                    neutral platform facts/events
                         shared Rust contracts
                                  |
                                  v
             bindings, input contexts, UI/game routing,
                client/scene policy, and engine decisions
                                  |
                                  v
                     shared scene/client/runtime owners
                                  |
                                  v
              optional mechanical platform dispositions
                                  |
                 +----------------+-----------------+
                 |                                  |
        native API execution              browser API execution
```

This is one logical boundary, not necessarily one Rust trait. Input,
presentation, asynchronous operations, persistence, networking, audio, and
diagnostics have different lifetime and performance requirements. They should
use small coherent contracts assembled at the host edge rather than one
catch-all `PlatformAdapter` or a browser-shaped ABI imposed on native.

## The Important Initialization Distinction

A thin platform adapter does **not** mean Rust must micromanage the browser or
operating system.

TypeScript may initialize and operate browser machinery autonomously whenever
no engine decision is involved. It may create a canvas, load Wasm, install DOM
listeners, start `requestAnimationFrame`, observe resize/visibility, construct
Workers, open IndexedDB, create WebSockets, probe capabilities, and satisfy
browser user-gesture requirements. Native adapters may likewise create
windows, surfaces, event loops, threads, audio devices, filesystem roots, and
OpenXR sessions without first receiving a Rust-domain command for every step.

Rust becomes the owner when timing, parameters, acceptance, or consequences
depend on engine state or product policy. The split is:

| Platform-owned | Shared-Rust-owned |
|---|---|
| API availability and construction | what a control means |
| event-loop, callback, and promise mechanics | bindings, rebinding, and input context |
| surface/swapchain/canvas acquisition | gameplay, UI, camera, and session decisions |
| browser gesture and permission constraints | whether an engine-dependent action is allowed |
| OS/browser error capture | typed engine failure and recovery policy |
| local bookkeeping required to use an API safely | operation identity, stale-result handling, and lifecycle meaning |
| executing an already-decided mechanical request | choosing the request from engine state |

For example, TypeScript owns the call to `requestPointerLock()`, the promise or
error behavior, and `pointerlockchange`. If the product rule is simply “a click
on this interactive canvas always requests capture,” TypeScript may implement
that reusable rule itself. If capture depends on a Rust-owned UI/input context,
Rust should expose a neutral capture disposition or desired capture state.
TypeScript still must not learn which menu or gameplay mode caused it.

Likewise, the browser should normally run its own rAF loop. Rust need not emit
`RequestRedraw` once per browser frame merely because native `winit` uses
explicit redraw requests. Shared semantics do not require identical physical
control flow.

## Vocabulary

| Term | Meaning here |
|---|---|
| **Platform fact** | A value observed from a host API: physical key code, pointer position, focus, visibility, size, device capability, time, or surface result. |
| **Neutral physical event** | A typed Rust event that identifies a control or physical observation without assigning a game action to it. |
| **Binding/resolver** | Shared Rust state that maps neutral controls to engine intents and maintains held/repeat/dead-zone state. |
| **Input context/router** | Shared Rust policy that decides whether resolved input belongs to UI, gameplay, camera, text entry, diagnostics, or no current consumer. |
| **Host disposition** | A small mechanical result such as handled/prevent-default, redraw useful, pointer capture desired, cursor mode, or exit. It does not describe why. |
| **Platform capability** | An injected facility such as presentation, audio output, storage execution, network transport, worker wakeup, clipboard, or text input. |
| **Platform adapter** | Leaf code that converts platform facts, owns platform resources, and executes platform mechanics. It does not own engine meaning. |
| **Platform assembly** | Code that initializes capabilities and connects them to shared owners. Assembly may legitimately differ by target. |
| **Diagnostic observer** | A read-only test/support projection of Rust-owned state. It is not a production state mirror or an alternate command API. |

The existing `KeyboardMouseInputAdapter` name in `mclone-input` refers to a
shared binding/held-state resolver, not a leaf OS/browser adapter. A future
cleanup may rename it to make that distinction obvious, but the ownership is
already correct.

## Decision Tests

Use these tests when ownership is unclear:

1. **Different-game test:** could a different game reuse this TypeScript or
   platform module unchanged? Canvas creation, raw event forwarding, Worker
   construction, and generic IndexedDB execution usually pass. `Space ->
   Jump`, hotbar rules, lobby phases, and block interaction do not.
2. **State test:** must the code inspect scene, UI, session, world, or gameplay
   state to decide what happens? If yes, the decision belongs in shared Rust.
3. **Vocabulary test:** does the platform layer need names such as Jump,
   Attack, Use, hotbar, pause menu, world behavior, generation profile, or
   readiness phase to execute correctly? If yes, the boundary is too high.
4. **Cross-platform change test:** would changing a binding or behavior require
   coordinated native, browser-Rust, and TypeScript edits? If yes, ownership is
   duplicated.
5. **Mechanics test:** is the difference imposed by browser/OS API rules,
   device shape, memory layout, or presentation substrate? If yes, it may
   remain platform-specific even when the logical contract is shared.
6. **Round-trip test:** would routing a local invariant through Rust add a
   command/completion round trip without letting Rust make a real decision? If
   yes, keep the invariant in the platform adapter.
7. **Native-cost test:** would the abstraction make native serialize values,
   adopt promises, copy browser frames, or indirect a hot path without a
   semantic benefit? If yes, the physical interface is too uniform.
8. **Test-surface test:** does production expose the field or command only
   because a smoke test reads it? If yes, move it to a test-only observer or
   replace it with direct Rust validation.

## Accepted Invariants

1. Game and engine semantics have one shared Rust owner.
2. Native, web, Android, XR, and offscreen hosts use the same logical owner for
   a behavior even when their physical event loops differ.
3. Platform adapters collect facts and execute mechanics; they do not map a
   physical control directly to a gameplay/UI action.
4. Bindings, rebinding, held state, repeat handling, dead zones, action
   contexts, and game-facing input intents are Rust-owned.
5. `mclone-scene` owns routing between shared UI, gameplay, camera,
   interaction, and presentation behavior because it owns the relevant state.
6. Platform setup may run autonomously when it is invariant and reusable.
7. Rust-to-platform dispositions exist only where Rust makes a real decision.
8. One logical boundary may be implemented as several small capability ports;
   it must not become a universal platform trait or byte protocol.
9. Native retains direct typed calls, moves, and platform-native scheduling.
10. Browser TypeScript may retain DOM, rAF, promise, Worker, SAB, IndexedDB,
    WebSocket, Web Lock, fullscreen, pointer-lock, and similar API mechanics.
11. Browser Rust may lower platform facts and shared decisions, but it must not
    become a second web-only gameplay or scene-policy owner.
12. Product TypeScript must not retain independent mirrors of semantic engine
    state for ordinary execution.
13. Smoke and support diagnostics are read-only observations, are clearly
    isolated, and do not define the production ABI.
14. Tests and scripted/offscreen harnesses may use semantic actions directly;
    they are engine clients, not platform adapters.
15. A production cut removes superseded native/web/TypeScript dispatch paths
    rather than retaining permanent conformance-by-duplication.
16. Capability absence is explicit. Shared policy may respond to an unavailable
    capability, but platform identity is not used as a substitute for a
    capability contract.

## Current-State Audit

Audit date: 2026-07-21.

The current architecture is not a failed design. Most deep semantic ownership
has already converged successfully. Tacticals 203 and 204 closed the
interactive platform rim, and Tactical 205 isolated browser diagnostics. The
remaining active problem is concentrated in browser preference and initial
bootstrap policy.

At the start of this topic, the browser ownership gate reported:

- 5,275 authored TypeScript lines across 16 registered modules;
- five Worker entry points and one generic Worker construction site;
- 56 registered domain-aware Worker/scene debts at zero; and
- seven explicit SAB copy facts.

Those gates correctly proved that server jobs, render compilation, socket
protocol state, persistence continuations, session-start selection, lobby
start sequencing, and readiness policy are not duplicated in TypeScript. They
did **not** prove that TypeScript was free of input, UI, gameplay, or diagnostic
vocabulary: the registries classified the former 886-line
`mclone-web-input.ts`/`mclone-web-touch.ts` family as ordinary browser
translation and did not inspect its action mapping. Tactical 204 closed that
input gap and added a host-boundary source lock. Tactical 205 then moved the
diagnostic global and semantic report projection into an explicit query-gated
observer. The current gate reports 4,090 authored TypeScript lines across 17
modules; `mclone-web-app.ts` is 1,261 lines and the separately classified smoke
observer is 391 lines. All 56 registered Worker/scene ownership debts remain
at zero.

### Summary Matrix

| Area | Current state | Assessment | Desired direction |
|---|---|---|---|
| Shared scene/client ownership | `McloneSceneHost` owns session, render admission, frame assembly, camera movement application, UI model, interaction, warm-world, and lifecycle policy | Healthy foundation | Keep; route all interactive hosts through it |
| Shared input vocabulary | `mclone-input` owns keyboard keys, pointer buttons, bindings, `FlatInputIntent`, `FlatInputFrame`, keyboard/mouse held state, touch state, gamepad state, capabilities, and preferences; `mclone-scene` owns the interactive router | Healthy converged outer boundary for flat hosts | Keep neutral event ingestion and shared routing; avoid parallel final dispatch |
| Desktop flat input | `WinitFrameDriver` owns `MonoInteractiveInputRouter`; its leaf adapter normalizes winit facts and executes cursor/redraw mechanics while ordinary final UI/game dispatch is shared | Native route converged; desktop-only renderer/development shortcuts remain classified diagnostics | Keep the typed direct route and prevent app-local semantic dispatch from returning |
| Flat Android input | The surface driver owns the same `MonoInteractiveInputRouter`; keyboard, pointer, shared touch hit testing/state, look, and held cadence use it while Android retains activity/surface/coordinate mechanics | Native route and shared touch model converged | Keep the typed route and platform mechanics |
| XR input | `mclone-xr-host` converts OpenXR action state into neutral `XrControllerSnapshot`; `mclone-scene` owns locomotion, comfort, teleport, menu pointer, and action meaning | Strong precedent | Preserve modality-specific shared scene policy; do not force XR through a flat/browser event ABI |
| Offscreen/test input | Scripts construct `FlatInputFrame`, `FlatInputAction`, and `GameUiAction` directly | Appropriate semantic test client | Keep; do not mistake tests for platform glue |
| Web keyboard/mouse | **Closed in Tactical 204:** TypeScript forwards raw DOM facts; browser Rust normalizes them and the shared router resolves meaning | Converged production input path | Keep source lock and observer-only diagnostics |
| Web touch | **Closed in Tactical 204:** TypeScript retains contacts/capture/synthetic-event mechanics while shared Rust owns layout, roles, dead zone, sensitivity, and actions | Converged with flat Android | Keep the shared touch owner and thin DOM adapter |
| Web frame input | **Closed in Tactical 204:** Rust holds input state and `renderFrame` receives time only | Correct boundary | Prevent semantic frame reconstruction in TypeScript |
| Browser Rust dispatch | **Closed in Tacticals 204-205:** raw key/pointer/wheel/touch exports feed `MonoInteractiveInputRouter`; retained semantic commands are observer-only and unused exports were deleted | Production route and diagnostic placement converged | Keep the product source lock and explicit observer boundary |
| UI routing | Shared `mclone-ui`/scene and `MonoInteractiveInputRouter` own ordinary flat UI/world routing; leaf adapters obey neutral handled/capture/redraw dispositions | Converged ordinary path | Keep development/test semantic clients explicit rather than returning their shortcuts to platform glue |
| Shared host effects | `mclone-scene::HostEffects` already carries mouse-lock, frame pacing, touch-mode, quit-to-title, and exit effects for shared UI policy | Good but not yet the interactive input boundary | Reuse/extend narrowly where Rust really decides; do not route autonomous setup through it |
| Cadence and presentation | Browser owns rAF, visibility callbacks, canvas resize, WebGPU presentation; native owns winit redraw/surface; XR owns OpenXR frame/swapchain sequencing | Correct asymmetry | Keep physical loops platform-owned behind shared time/render contracts |
| Workers/jobs/render compiler | Worker-resident Rust actors own meaning; TS handles module loading, SAB/message transport, wakeups, and failure envelopes | Healthy | Preserve existing actor/mailbox boundaries |
| WebSocket | Worker-resident Rust owns protocol/lifecycle/backpressure; TS executes open/send/event/close actions | Healthy model | Preserve; use as an example of a thin mechanical executor |
| Persistence | Rust owns record meaning, revisions, continuations, and writer authority; TS maps stable namespaces and executes IndexedDB requests | Healthy with one exception | Preserve; keep the legacy v5-to-v6 Overworld migration quarantined |
| Catalog | Rust owns catalog continuation and UI meaning; TS executes stable store/index actions | Healthy specialized adapter | Preserve; do not fold into a universal host interface |
| Startup assembly | TS loads Wasm/assets and initializes browser facilities, but still branches initial remote versus integrated host construction and mirrors semantic startup options | Mixed | Keep loading/fetch mechanics; let one Rust-authored bootstrap plan choose engine/session meaning |
| Asset loading | TS fetches three hard-coded role-bearing pack URLs and passes them in semantic order | Mechanical fetch plus semantic assembly | Rust owns logical pack roles/selection; TS fetches requested URLs/bytes generically |
| Preferences | TS owns look-sensitivity defaults/clamping, touch-mode parsing, localStorage keys, and action-sensitive persistence while Rust has shared input preferences/settings | Duplicated policy | Rust owns value schema/defaults/clamping; TS may provide a generic browser key/value executor |
| Audio | Shared `AudioOutputCapability` and scene sound decisions feed native audio; browser audio is explicitly unavailable | Clean capability shape, incomplete web feature | Future web output may use WebAudio mechanics without moving sound meaning to TS |
| Diagnostics/smoke | **Closed in Tactical 205:** a query-gated test observer exposes Rust-authored reports and retained test commands; ordinary pages install no semantic global | Correct explicit test-client boundary | Keep product behavior independent of observer presence and prefer real input/shared tests where appropriate |
| Bootstrap failures | TS may author a last-resort error when Wasm/module/browser API setup fails before a Rust owner exists | Legitimate exception | Keep narrow and clearly distinguish from ordinary Rust-authored failures |

### Shared Rust Foundation Already In Place

`mclone-input` already owns most of the reusable input model:

- `KeyboardKey::from_code_name` parses the stable names shared by winit debug
  formatting and browser `KeyboardEvent.code`;
- `KeyboardMouseBindings::default` is the single existing table for W/A/S/D,
  arrows, Space, Shift, Control, Escape, F1/F5, pointer buttons, mouse motion,
  wheel, and hotbar digits;
- `KeyboardMouseInputAdapter` resolves controls, tracks held state, handles
  repeat, and emits `FlatInputFrame` values;
- `TouchInputAdapter` owns contact state, movement/look conversion, and action
  frames for the flat Android path;
- `GamepadInputAdapter`, capability/preference types, XR controller snapshots,
  and flat/XR conversion helpers are also shared; and
- `McloneSceneHost::advance_mono_input_frame` owns gameplay-start gating,
  movement application, embedded activation progression, and player-pose
  publication cadence.

`mclone-scene::HostEffects` and the exhaustive client-experience settings
dispatcher are another important foundation. Shared Rust already decides many
UI/settings effects once, while native, Android, XR, and offscreen hosts supply
mechanical implementations.

The shared outer interactive router now exists as
`mclone-scene::MonoInteractiveInputRouter`. It accepts neutral keyboard,
pointer, wheel, touch-derived flat frames, and held cadence, resolves them
using the existing input components, routes them according to scene/UI
context, and returns a mechanical `MonoInputDisposition`. Desktop, flat
Android, and browser have adopted it. Browser Rust parses raw DOM encodings;
TypeScript retains only event and browser-mechanics handling. Tactical 205
also moved rich observations and semantic test commands behind the explicit
smoke observer. The remaining active boundary is preference/bootstrap policy.

Wheel and Descend binding drift were concrete reasons for introducing the
outer route: all flat hosts now resolve those controls through the shared
binding/router owner rather than maintaining browser/native action tables.

### Desktop Flat Audit

The desktop path now has the target native shape:

```text
winit KeyCode / MouseButton
  -> DesktopFlatInputAdapter
  -> shared KeyboardKey / PointerButton
  -> shared MonoInteractiveInputRouter
  -> existing McloneSceneHost policy
  -> mechanical MonoInputDisposition
```

`mclone-native-client/src/app.rs` still owns desktop-only renderer and
development shortcuts such as movement-mode/debug visualization toggles.
Those are explicitly classified diagnostics rather than ordinary product
bindings. It no longer owns:

- UI-active checks and direct `GuiKey` routing;
- separate menu/help/block-palette/hotbar dispatch from `FlatInputFrame`;
- separate Attack/Use dispatch;
- UI pointer routing and direct pause-menu/block-palette dispatch in
  `WinitFrameDriver`; or
- held-input and focus-clearing policy distributed across parallel owners.

Winit window creation, surface ownership, cursor-grab API calls, monitor/frame
pacing, device events, and redraw scheduling are valid desktop responsibilities.
The remaining diagnostic shortcuts are not part of the ordinary input route.

### Flat Android Audit

Flat Android is now converged at the same final route. `surface_driver.rs`
receives winit Android events, uses shared keyboard/mouse and touch adapters,
and feeds their frames through `MonoInteractiveInputRouter`. Its former
`apply_flat_frame` dispatcher for menu, block palette, help, camera view,
hotbar selection, Attack, Use, and look is deleted. This proves the browser
does not need a separate TypeScript joystick/action implementation.

Touch-control hit testing (`touch_control_at`) and the stateful touch resolver
are now shared by flat Android and browser without forcing native through a
DOM-shaped ABI.

Android activity/window/surface recreation, lifecycle save calls, redraw
cadence, audio-device construction, and conversion from winit touch coordinates
are valid platform responsibilities. Final action routing should converge with
desktop and web.

Desktop and flat Android both use winit. A tactical should evaluate a small
shared winit normalizer module/crate so they do not each maintain physical key,
button, wheel, focus, and pointer conversion. `mclone-input` itself should not
gain a winit dependency merely for convenience.

### XR Audit

XR is the strongest current example of the desired split:

- `mclone-xr-host` owns OpenXR action-set creation, interaction-profile
  bindings, action polling, spaces, and conversion to `XrControllerSnapshot`;
- `XrControllerSnapshot` is a neutral shared `mclone-input` value containing
  hand, poses, axes, buttons, trigger, and squeeze facts; and
- `mclone-scene` owns the meaning of those facts: locomotion, snap/smooth turn,
  jump/descend/sprint/sneak mapping, teleport, thruster/hand-push modes, UI
  pointer behavior, comfort, and scene interaction.

Desktop OpenXR and Android XR therefore reuse one OpenXR host adapter and one
scene policy path. Their app crates still contain substantial session,
swapchain, performance, and device glue, but this audit found no TypeScript-like
second XR gameplay engine to replace.

Desktop XR emulation (`xr_emulation.rs`) is a small exception on the desktop
side: it dispatches its own keyboard handling, synthesizes an open-menu input
frame, and enters a scenario through `GameUiAction` directly. It is a
development harness rather than a product input path, but the tactical should
either route it through the shared router or explicitly document it as a
test-client exception so it does not survive as a stray semantic dispatcher.

XR should not be forced through the exact flat `PhysicalInputEvent` vocabulary
if doing so erases pose, per-hand, tracking-validity, or analog information.
The common rule is shared semantic ownership, not identical device data.

### Browser Input And UI Audit

Tactical 204 closed the former browser input violation. `mclone-web-input.ts`
and `mclone-web-touch.ts` now forward raw key, pointer, wheel, touch/pen,
focus, and viewport facts. Browser Rust normalizes browser encodings, shared
Rust maintains held and touch state, and `MonoInteractiveInputRouter` owns
binding plus UI/game routing. `WebSceneHost::renderFrame` receives time only.

The TypeScript modules no longer contain action-state names, hotbar/menu
mapping, break/place strings, camera-speed policy, touch button roles,
joystick/dead-zone policy, or semantic overlay construction. Browser and flat
Android share the touch hit-test and stateful adapter. A focused Rust source
lock rejects reintroduction of these production paths.

Retained semantic `WebSceneHost` commands now have no ordinary product caller;
the query-gated smoke observer is their explicit test-client facade. Tactical
205 deleted nine unused direct exports and a dead camera-aim helper, and the
source lock prevents representative semantic methods and state from returning
to the product module.

The browser may still:

- install and remove the listeners;
- choose passive/capture listener options;
- call `preventDefault()` according to a generic handled disposition;
- perform pointer capture and pointer-lock API calls;
- translate CSS coordinates to physical canvas coordinates;
- suppress synthetic mouse events after touch as a browser-input hygiene rule;
- satisfy fullscreen and pointer-lock user-gesture constraints; and
- track enough local state to call those browser APIs safely.

It must not need to know that the handled key was Jump, that the right button
means Use, that a menu is specifically the pause menu, or that a touch button
represents Attack.

### Browser Bootstrap, Cadence, And Async Audit

Much of `WebFrameDriver` is legitimate browser assembly:

- dynamic Wasm loading;
- canvas lookup/focus and WebGPU host construction;
- asset byte fetching;
- Worker URL construction and generic `PolledWorkerTransport` assembly;
- non-overlapping rAF and wasm-bindgen mutable-borrow exclusion;
- visibility callbacks, canvas sizing, device-pixel conversion, and final
  presentation;
- yielding while Rust-owned readiness is false; and
- starting Rust-owned opaque session, lobby, catalog, and asset operations and
  returning their completions.

Remaining semantic/bootstrap seams include:

- parsing a Rust browser startup plan back into TypeScript fields;
- TypeScript choosing initial remote versus integrated host construction;
- TypeScript re-clamping render distance and mirroring generation, render,
  occlusion, and fullbright settings already owned by Rust;
- TypeScript assigning logical roles to the reference/authored/fallback asset
  pack URLs;
- TypeScript choosing the default debug-overlay visibility from touch/media
  facts instead of reporting those capabilities to shared policy;
- `startupStatusLabel` interpreting startup phase strings into product-facing
  loading text after Rust is available;
- semantic method-by-method Wasm export validation; and
- UI-action-sensitive pointer lock and preference persistence branches.

The desired bootstrap is inversion of control rather than Rust commanding
every browser API. TypeScript initializes available browser capabilities and
passes their handles/URLs/executors to one browser-Rust bootstrap boundary.
Rust consumes the startup configuration and chooses engine/session meaning.
Where Rust needs bytes, it may emit an opaque resource request that TypeScript
fetches mechanically.

The current rAF loop, visibility observation, and canvas resizing are not in
themselves debts. They should remain browser-owned unless a specific engine
policy currently leaks into them.

### Complete Authored Browser Module Inventory

The ownership checker inventories all 17 authored TypeScript modules. This
table extends its existing Worker-focused classification with the interactive
host findings from this audit:

| Module | Current responsibility | Host-boundary assessment |
|---|---|---|
| `mclone-web-app.ts` (1,261 lines) | browser assembly, rAF, canvas, async service driving, operational projection, preference/bootstrap seams | Mixed: healthy mechanics plus the focused Tactical 206 debt; semantic diagnostics are gone |
| `mclone-web-input.ts` (161) | raw DOM keyboard/mouse forwarding and browser event mechanics | Healthy thin physical adapter |
| `mclone-web-touch.ts` (151) | raw touch/pen forwarding, pointer capture, and synthetic-mouse hygiene | Healthy thin physical adapter over shared Rust touch policy |
| `mclone-web-settings.ts` (86) | semantic localStorage settings plus an unused interaction formatter | Active Tactical 206 debt: storage mechanics are valid but identity/defaults/clamping/formatting are not platform policy |
| `mclone-web-smoke-observer.ts` (391) | query-gated semantic reports, test aliases/receipts, and retained smoke commands | Appropriate explicit test client; absent from ordinary pages |
| `mclone-web-world-catalog.ts` (414) | IndexedDB schema, store/index resolution, catalog transactions | Healthy specialized executor except the locked legacy Overworld migration |
| `mclone-web-persistence-executor.ts` (471) | namespace-to-store addressing, IndexedDB record requests, error classification | Healthy mechanical executor |
| `mclone-web-world-lease.ts` (51) | Web Lock writer lease | Healthy mechanical lifetime adapter |
| `mclone-worker-transport.ts` (47) | generic Worker construction, post/poll/terminate | Healthy reusable platform machinery |
| `mclone-integrated-server-worker.ts` (568) | Worker timer, opaque Rust server actor, persistence continuations, SAB transport | Healthy mechanical shell; retain its bounded continuation circuit breaker |
| `mclone-remote-websocket-worker.ts` (161) | WebSocket API execution around Rust actor actions | Healthy thin executor and a target pattern |
| `mclone-server-job-worker.ts` (157) | transferred/SAB job frames and Rust actor invocation | Healthy domain-blind Worker shell |
| `mclone-render-compiler-worker.ts` (69) | Wasm loading and Rust render-actor bootstrap | Healthy domain-blind Worker shell |
| `mclone-render-compiler-shared.ts` (32) | asset fetch and SAB predicates | Healthy browser mechanics; future resource-role decisions must stay Rust-owned |
| `mclone-render-compiler-abi.d.ts` (16) | locked external SAB declarations | Healthy mechanical ABI declaration |
| `mclone-runner-shared-abi.d.ts` (7) | locked runner/job SAB declarations | Healthy mechanical ABI declaration |
| `mclone-thread-smoke-worker.ts` (47) | non-production shared-Wasm-memory capability proof | Appropriate isolated test Worker |

Two important JavaScript files sit outside that TypeScript inventory:

- `www/mclone-web-smoke.js` is a separate 1,334-line smoke page. Its semantic
  knowledge is acceptable because it is explicitly a test client, though it
  should use the future observer rather than define production interfaces.
- `scripts/browser-smoke.mjs` is the Playwright integration runner. Semantic
  assertions belong there when browser integration is what the test covers;
  it should stop reaching through a production-global semantic mirror.

### Browser Workers, Networking, And Persistence Audit

The Worker-side architecture is already the model to follow:

- the render Worker hosts a Rust render actor; TypeScript loads Wasm and moves
  opaque/SAB data;
- server-job Workers host Rust actors; TypeScript services external SAB or
  transferred frames without selecting worldgen/light meaning;
- the integrated-server Worker hosts the Rust server actor and Rust record
  continuation; TypeScript owns timers, worker calls, writer leases, and
  IndexedDB request execution;
- the remote WebSocket Worker hosts Rust protocol/lifecycle state and executes
  only generic open/send/post/close actions; and
- the catalog executor receives stable store/index/action descriptions from a
  Rust continuation.

The record executor necessarily maps stable namespaces to physical IndexedDB
stores and keys. That is platform storage addressing, not gameplay policy.
Likewise, `mclone-web-world-catalog.ts` owns database/store/index creation and
transactions.

One accepted exception remains: the legacy pre-dimension upgrade cursor
assigns old records to `minecraft:overworld` (it runs whenever the old
pre-dimension stores still exist during the version-6 open, not on an
explicit stored-version check). That is semantic migration code in TypeScript. It is
quarantined by `web_scene_async_boundary_lock` and is outside this host-input
cleanup. Remove or replace it only through a separate data-compatibility
decision; do not create runtime migration machinery here.

### Preferences Audit

`mclone-web-settings.ts` currently owns browser-local defaults and clamping for
touch look sensitivity, parses the `auto/on/off` touch-control mode, persists
named settings in localStorage, and formats semantic interaction outcomes.
`WebFrameDriver::applyNativeUiReport` also interprets setting fields and decides
when to persist those values. Tactical 205 removed unrelated semantic report
projection, leaving this exact seam for Tactical 206.

The desired split is:

- shared Rust owns preference identity, types, defaults, ranges, validation,
  and how settings affect input/UI/game behavior;
- TypeScript may implement a generic browser preference store and report
  storage availability/failure; and
- platform-specific preferences may exist, but their meaning still belongs to
  a typed Rust owner rather than ad hoc TypeScript constants.

`formatInteractionStatus` is unused and is not platform storage machinery.
Product UI text belongs in shared UI/localization; smoke-only formatting
belongs in test code.

### Output And Diagnostics Audit

The rendering output boundary is broadly healthy: shared scene/render crates
assemble world, actor, effect, and UI rendering while each platform owns its
surface or swapchain. Audio also has a shared `AudioOutputCapability`; the web
capability is honestly absent today.

Before Tactical 205, the browser diagnostic boundary was not healthy.
`WebSceneHost::report` authored a very large JavaScript object containing
camera/game state, player statistics, world time, hotbar state, session state,
UI state, startup state, actor/chunk counts, mailbox/backpressure metrics,
embedded-world internals, warm-world state, asset replacement state,
render/compiler state, and numerous smoke receipts. Production
`mclone-web-app.ts` copied much of that into global `__mcloneWebApp.state` and
exposed methods such as:

- `setInputKey`, `interactBlock`, `blockStateAt`, and `adjustCameraSpeed`;
- direct title/pause/help/UI pointer calls;
- embedded-world framing and lobby-smoke entry points;
- one-frame/proof rendering and resource rebuild hooks; and
- background-save and shutdown smoke helpers.

Playwright then reads semantic fields such as movement mode, collision mode,
Jump and placement statistics, world time, selected hotbar slot, block state,
actors, embedded preview identities, and internal queue receipts.

Tests are allowed to understand the game. Tactical 205 stopped using the
production adapter and its ordinary state as that test observer. The explicit
`mclone-web-smoke-observer.ts` module is installed only by
`smokeObserver=1`, consumes Rust-authored reports, and exposes the retained
commands and compatibility aliases to Playwright. Ordinary pages never define
the global.

The target is:

```text
production browser adapter
  minimal lifecycle/operational state only
  no semantic report projection
  no smoke command registry

test-only Rust diagnostic observer
  read-only typed snapshot/receipts
  explicit test commands only where real input cannot exercise the contract
        |
        v
thin test-only browser exposure
        |
        v
Playwright assertions and screenshots
```

Real DOM input remains preferred in browser adapter smokes, with shared Rust
unit/offscreen tests proving action meaning and internal state. Retained
test-only direct semantic commands are explicitly below/above the physical
input boundary and stay out of production behavior.

`www/mclone-web-smoke.js` remains a separate smoke-page harness. The ordinary
application now follows the same separation: its semantic smoke surface is in
the explicitly requested observer module, not embedded in production state.

## Desired Shared Contract Family

The exact types are an implementing-tactical decision, but the ownership shape
should remain recognizable.

### 1. Neutral physical input

`mclone-input` should own or expose a neutral event vocabulary sufficient for
flat interactive hosts, conceptually:

```rust
enum PhysicalInputEvent {
    Key { key: KeyboardKey, pressed: bool, repeat: bool },
    PointerButton { button: PointerButton, pressed: bool },
    PointerMotion { delta: Vec2 },
    PointerPosition { position: Vec2 },
    Wheel { direction_or_delta: /* neutral value */ },
    TouchContact { id: u64, phase: /* neutral phase */, position: Vec2 },
    FocusChanged(bool),
}
```

This sketch is not a demand for one enum if smaller typed entry points are
clearer or faster. It is a demand that the event identify physical controls and
facts, not Jump, Attack, Use, hotbar, or a particular UI screen.

Browser code-string parsing belongs in browser Rust or a shared stable parser.
Winit key conversion belongs in a small reusable winit adapter. Native should
not serialize key strings merely to imitate the browser.

### 2. Shared binding and held-state resolution

The existing `KeyboardMouseInputAdapter`, `TouchInputAdapter`, and gamepad/XR
types should be reused or reorganized rather than reimplemented. One binding
change must affect every applicable flat platform. Rust owns repeat,
press/release, held movement, dead zones, sensitivity, hotbar selection, and
action production.

Touch layout and visual overlay require special care. DOM pointer capture and
CSS coordinates are browser mechanics, but assigning regions/buttons to game
actions and rendering game controls are input/UI semantics. The preferred
direction is a shared Rust touch-control model consumed by both flat Android
and web, with platform adapters supplying contacts and viewport facts.

### 3. Shared context and action routing

A shared interactive scene/host router should:

- ask the shared UI/input context which consumer is active;
- route key, pointer, touch, and gamepad events accordingly;
- apply menu/help/debug/gameplay/camera/hotbar/interaction behavior once;
- clear held/transient state on focus loss or context transition;
- advance held movement from the presentation frame time;
- invoke existing `McloneSceneHost` methods and client-experience reducers; and
- return a mechanical outcome.

The router belongs with the shared state it needs. Neutral controls and
binding resolution belong in `mclone-input`; scene/UI/game routing belongs in
`mclone-scene`; broader session/operation effects remain in
`mclone-app-runtime`. Do not create a new `mclone-platform` policy crate that
merely forwards calls among the real owners.

### 4. Mechanical event outcome

The platform-facing result may include only facts the host needs to operate,
for example:

```text
handled / prevent-default
redraw useful
desired pointer capture or cursor mode
clear platform transient gesture state
exit or close requested
typed asynchronous operation became available
```

It must not return “Jump handled,” “pause menu opened,” or “block placed” for
ordinary execution. Rust tests and diagnostic observers may inspect those
semantic outcomes separately.

Not every platform needs every field. Browser handled/prevent-default and
native redraw/cursor handling may be separate outcome types sharing only the
logical decisions they actually have in common.

### 5. Frame and presentation

The browser frame boundary should stop receiving a semantic input snapshot
constructed in TypeScript. Rust input state should already contain key/button/
touch state when the frame begins. The presentation driver supplies time,
surface size/target, visibility, and platform render results as appropriate.

Native winit, browser rAF, Android lifecycle, OpenXR frame sequencing, and
offscreen capture remain specialized. `McloneSceneHost` and render-session
contracts continue to own frame admission, scene synchronization, and render
meaning.

### 6. Environment capabilities and bootstrap

Platform assembly should initialize available services and attach them to one
Rust-authored startup/host configuration. It is acceptable for TypeScript to
create every browser primitive up front or lazily. It is not acceptable for it
to choose engine/session variants by interpreting the startup domain.

Capabilities should be explicit facts such as audio available, pointer lock
supported, persistent storage available, clipboard available, or shared-memory
transport supported. Shared code responds to capabilities; it does not branch
on “web” merely to select different product semantics.

### 7. Asynchronous operations

Keep the actor/mailbox and typed platform-operation direction from
[`cross-platform-operation-execution.md`](cross-platform-operation-execution.md).
Input events do not need to become actors. Storage, sockets, workers, catalog,
and coarse asynchronous work retain specialized request/completion contracts.

The shared host boundary assembles these capabilities; it does not fold them
into the input router or one universal executor.

### 8. Diagnostic observer

Define a shared diagnostics contract before adding more product-global report
fields. It should distinguish:

- production operational health needed by the platform shell;
- in-product diagnostics rendered by shared Rust UI;
- support/telemetry snapshots with an explicit schema; and
- test-only semantic receipts and commands.

The browser smoke observer should be a clearly named separate module or build
surface. Its snapshot should be Rust-authored. The in-page JavaScript may expose
that snapshot mechanically; semantic assertions live in the Playwright runner
or Rust tests. Test-only commands should not be installed on the ordinary
production global.

## TypeScript Boundary In Detail

### TypeScript may own

- dynamic import and Wasm initialization;
- canvas lookup, focus, sizing, device-pixel conversion, and browser surface
  plumbing;
- DOM listener installation/removal and listener option selection;
- raw keyboard, pointer, touch, wheel, focus, visibility, and resize capture;
- `preventDefault`, pointer capture, pointer lock, fullscreen, soft-keyboard,
  clipboard, and browser permission mechanics;
- rAF/timer/yield scheduling and non-overlap/busy guards required by JS/Wasm
  borrowing;
- Worker construction/termination, `postMessage`, transfer lists, SAB views,
  atomics, and wakeups;
- IndexedDB/Web Lock/localStorage API calls through generic executors;
- WebSocket construction and browser callbacks;
- URL resolution, fetch, cache/version mechanics, and byte delivery for
  Rust-authored resource requests;
- browser capability discovery; and
- last-resort errors before Wasm/Rust can exist.

### Production TypeScript must not own

- physical-key-to-game-action mappings;
- held gameplay action state or `FlatInputFrame` assembly;
- input contexts or UI-versus-gameplay routing;
- menu/help/debug action selection;
- hotbar size/selection rules;
- block Attack/Use/break/place decisions;
- touch joystick game mapping, game-button identities, or semantic overlay
  state;
- camera movement/speed policy;
- preference defaults, ranges, validation, or engine application;
- local/remote/session/world/lobby/asset meaning;
- readiness, retry, revision, stale-result, or completion decisions;
- semantic success/failure construction;
- broad mirrors of player/world/session/UI/render state; or
- smoke-only semantic command registries and receipt accumulation.

Seeing a physical name such as `Space`, `pointerdown`, an opaque namespace, or
a browser store name is not itself a violation. The violation is assigning or
depending on engine meaning.

## Healthy Asymmetry That Must Survive

Do not “clean up” these differences merely for visual symmetry:

- browser rAF versus winit redraw scheduling versus OpenXR frame sequencing;
- browser promise/callback completion versus native direct calls or worker
  channels;
- IndexedDB transactions versus SQLite calls;
- browser private Wasm heaps and external SAB mailboxes versus native Rust
  moves/shared immutable allocations;
- CSS pixels/browser gesture constraints versus native physical window
  coordinates and cursor grab;
- XR hand poses, tracked validity, views, and swapchains versus flat pointer
  input; and
- offscreen semantic scripts versus live physical event adapters.

The goal is one semantic owner and clean capability boundaries, not identical
source code or transports.

## Current-To-Desired Gap Ledger

| Gap | Current owner(s) | Desired owner | Required deletion/adoption evidence |
|---|---|---|---|
| Browser key/action map | **Closed in Tactical 204:** raw TypeScript forwarding into `mclone-input` bindings | shared bindings/resolver | Source lock rejects action-name tables and key-to-game switches in production TS |
| Browser held input/frame assembly | **Closed in Tactical 204:** `WebSceneHost` router state | shared Rust input/router | `renderFrame` receives time only |
| Browser mouse Attack/Use | **Closed in Tactical 204:** raw click/button forwarding | shared binding/context router | No ordinary TypeScript break/place strings |
| Browser Escape/F1/debug/movement shortcuts | **Closed in Tactical 204:** raw key forwarding | shared input/UI/client-experience routing | No direct semantic UI/debug calls from DOM key handlers |
| Browser wheel camera-speed policy | **Closed in Tactical 204:** raw wheel forwarding | shared binding/runtime shortcut policy | Shared resolver owns meaning |
| Browser touch game model | **Closed in Tactical 204:** raw contact forwarding plus browser mechanics | shared input/UI model used with Android | No action names, control roles, or dead-zone policy in production TS |
| Desktop final `FlatInputFrame` dispatch | **Closed in Tactical 203:** `MonoInteractiveInputRouter` | shared interactive router | Source lock rejects the deleted app-local final-dispatch methods |
| Android final `FlatInputFrame` dispatch | **Closed in Tactical 203:** `MonoInteractiveInputRouter` | same shared interactive router | Source lock rejects the deleted app-local final-dispatch method |
| Duplicated winit normalization | desktop and flat Android apps | small reusable winit adapter | Same key/button/focus conversion used by both without a winit dependency in engine crates |
| UI active/pointer routing | **Closed for ordinary flat input in Tacticals 203-204:** shared router plus mechanical disposition | shared scene router plus mechanical outcome | Platform code no longer names the target UI action/screen |
| Pointer-lock policy leaks | desktop/web semantic branches | shared desired capture only where context-dependent; local mechanics otherwise | TS/native API code receives neutral desired state, or documents invariant autonomous capture |
| Browser preference semantics | `mclone-web-settings.ts` plus report-action branches | shared input/preferences/settings owner | TS storage executor treats typed/opaque values mechanically |
| Initial web host selection | TS startup branch | browser Rust consuming shared startup configuration | One mechanical browser bootstrap entry, all capability URLs supplied without TS choosing session meaning |
| Asset pack role assembly | TS constants/argument order | shared Rust asset plan | TS fetches Rust-requested resources without authored/reference/fallback selection logic |
| Browser startup/status projection | TS clamps/mirrors engine settings, selects debug visibility, and interprets post-Wasm status phases | shared startup/settings/UI owners; generic pre-Wasm browser status only | TS reports capabilities and shows generic bootstrap/fatal state without reconstructing engine configuration or phases |
| Semantic Wasm method surface | **Reduced in Tactical 205:** unused semantic exports deleted and retained commands are observer-only; startup validation remains broad | small event/frame/operation/capability ABI plus explicit diagnostic facade | Tactical 206 groups the remaining product surface and keeps observer commands separate |
| Browser Rust semantic dispatch | **Closed in Tacticals 204-205:** shared router for ordinary input, query-gated test facade for retained commands | shared router plus test-only diagnostic facade | Source lock rejects semantic per-action exports in the product adapter surface |
| Production semantic state mirror | **Closed in Tactical 205:** no broad copy; ordinary pages define no `__mcloneWebApp` | Rust state plus bounded operational projection | Product source lock rejects representative state fields and mirror helpers |
| Production smoke command hooks | **Closed in Tactical 205:** query-gated `mclone-web-smoke-observer.ts` owns them | test-only observer/harness | Ordinary production global excludes semantic smoke methods |
| Rich report bag | Rust still serializes a mixed report; product TS reads only operational fields while the opt-in observer consumes semantic fields | typed operational result plus explicit diagnostic snapshot | Tactical 206 closeout separates the always-on product projection if doing so does not duplicate report assembly |
| Legacy IndexedDB dimension migration | catalog TS upgrade callback | separate offline/data compatibility decision | Remains quarantined until explicitly replaced; not part of this tactical |

## Recommended Implementation Sequence

This should be implemented as a staged tactical, not a rewrite of every host at
once.

### Stage 0: Contract and enforcement baseline

- Freeze the current input/report/production-global inventory.
- Add focused source/contract locks for the named gaps above.
- Separate healthy Worker/storage/socket adapters from interactive-host debt so
  later line-count changes are not mistaken for architectural progress.
- Record native desktop, flat Android, web desktop/mobile, and offscreen input
  behavior before changing ownership, including the currently undocumented
  browser semantics: gameplay input suppression while a native UI is active
  (zeroed deltas plus default key state), the 4px click/drag threshold on
  break/place (movement greater than 4px cancels the click), the 800ms
  touch-to-synthetic-mouse suppression
  window, the `shift`-to-Sneak rename and arrow-turn versus A/D-strafe split,
  one-shot Attack/Use versus held Jump/Descend touch buttons, dead-zone
  renormalized analog impulses with pen-as-touch, wheel delta-mode scaling,
  and pointer-lock reacquisition on resume. Each recorded behavior must later
  land in the shared router, be documented as retained browser hygiene, or be
  explicitly rejected as accidental platform drift.

### Stage 1: Shared interactive input/router contract

- Reuse `mclone-input` physical controls, bindings, held state, touch, and
  capability types.
- Add the minimum neutral event ingestion and mechanical outcome types.
- Put UI/game/camera/hotbar/interaction routing in `mclone-scene` using existing
  `McloneSceneHost`, `HostEffects`, and client-experience reducers.
- Unit-test bindings separately from platform normalization and scene routing.

### Stage 2: Native-first extraction

- Make desktop winit drive the shared router.
- Remove the semantic key/action cascade and final flat-frame dispatch from
  `ChunkApp`, and the UI pointer and pause-menu/block-palette dispatch from
  `WinitFrameDriver`, where the shared router now owns them.
- Make flat Android use the same route, deleting its duplicate
  `apply_flat_frame` decisions.
- Extract only genuinely shared winit normalization; preserve platform event
  loop, surface, cursor, lifecycle, and redraw mechanics.

Native-first adoption keeps the shared contract typed and direct and prevents
the Wasm ABI from defining the engine architecture.

### Stage 3: Browser raw-input adoption

- Replace action-name setters with raw key/button/pointer/touch entry points.
- Move browser code parsing into browser Rust and reuse shared binding state.
- Stop constructing a semantic frame in TS; frame advancement consumes
  Rust-held input.
- Move UI/gameplay/touch mapping behind the same shared router.
- Delete the string-keyed break/place world-action ABI and per-action UI
  key/help/hotbar exports from `WebSceneHost` as the raw event path replaces
  them.
- Retain DOM listener, capture, pointer-lock, fullscreen, and rAF mechanics in
  TypeScript.

### Stage 4: Diagnostic and smoke isolation

- Define a Rust-authored diagnostic/test observer.
- Move semantic state projection and smoke-only commands out of production
  `mclone-web-app.ts`.
- Make browser input smokes use real DOM events.
- Move action semantics assertions to shared Rust tests and retain Playwright
  for browser mechanics, integration, and screenshots.
- Delete superseded report fields and direct semantic Wasm exports after all
  consumers move.

### Stage 5: Preferences and bootstrap cleanup

- Move browser preference schema/default/clamping/application into shared Rust.
- Give TypeScript a generic persistence mechanism where browser persistence is
  still desired.
- Consolidate initial web host construction behind a Rust-authored bootstrap
  decision while retaining browser capability setup and byte fetch mechanics.
- Review asset-resource requests so TypeScript does not assign logical pack
  roles.

### Stage 6: Closeout and parity validation

- Run shared input/scene/app-runtime tests.
- Run desktop and flat Android input/UI controls.
- Run headed browser desktop and mobile input/UI/persistence/session smokes and
  inspect screenshots.
- Run XR build/control gates if shared input or scene contracts affect XR.
- Run the Worker ownership, scene-host adoption, generated-bindgen, and
  TypeScript gates with new host-boundary locks.
- Refresh this topic's audit and the platform-parity matrix, and verify current
  summary figures in adjacent docs against gate output while preserving
  explicitly historical per-slice figures.

## Acceptance Criteria

The work is complete only when all of these are true:

1. Changing the default binding for Jump, Attack, Use, menu, help, camera view,
   or hotbar selection requires one shared Rust change.
2. Production TypeScript contains no gameplay action-name input state and no
   key/button/touch mapping to game actions.
3. Browser frame advancement does not accept a TypeScript-authored semantic
   input frame.
4. Desktop, flat Android, and browser flat input pass through one shared
   binding/context/action router.
5. Desktop and flat Android do not retain parallel final `FlatInputFrame`
   dispatch matches.
6. XR continues to use shared `XrControllerSnapshot` and scene locomotion/UI
   semantics without being flattened into a lossy browser-shaped interface.
7. Platform adapters retain only raw conversion, lifecycle, resource,
   presentation, permission, and API mechanics.
8. TypeScript remains free to initialize browser capabilities autonomously
   where no engine decision exists.
9. Context-dependent pointer/cursor behavior crosses a neutral disposition;
   invariant browser-local behavior is documented and stays local.
10. Production `mclone-web-app.ts` no longer maintains a broad semantic mirror
    or semantic smoke command registry.
11. Semantic diagnostics used by browser tests come from a clearly separate
    Rust-authored observer; product code does not depend on them.
12. Real DOM event smokes prove the browser raw-event adapter, while shared Rust
    tests prove action meaning.
13. Existing Worker, socket, render, catalog, and persistence actor ownership
    remains intact.
14. Native gains no browser serialization, promises, SAB-shaped calls, or
    unjustified hot-path allocation.
15. Rendered desktop, mobile browser, and affected native/device output is
    captured and inspected under the platform validation policy.
16. Web-only Rust no longer exposes a string-keyed game-action ABI or
    per-action UI entry points; browser raw events cross one typed neutral
    boundary.

## Validation Model

Validation should be layered so each test proves one owner:

| Layer | Proof |
|---|---|
| Shared binding tests | neutral `KeyboardKey::Space` resolves to shared Jump intent; rebinding changes one table |
| Platform normalization tests | winit `KeyCode::Space` and browser `KeyboardEvent.code == "Space"` normalize to the same neutral key without asserting Jump in the leaf test |
| Shared router tests | identical neutral event sequences produce identical UI/game/camera/action effects and mechanical dispositions |
| Native integration | real winit events exercise desktop and flat Android routes; focus/cursor/redraw behavior remains platform-correct |
| Browser integration | real DOM keyboard/pointer/touch events exercise raw forwarding, prevent-default, pointer capture/lock, rAF, and Wasm routing |
| XR controls | OpenXR snapshots continue through shared scene locomotion/menu/interaction paths |
| Offscreen semantic tests | direct intent/action scripts prove engine semantics independently of a live input API |
| Diagnostic boundary locks | production TS has no forbidden action map/report mirror/smoke commands; test observer is isolated |
| Pixels | headed browser and affected native/device screenshots are captured and inspected at drawable milestones |

Do not test `Space -> Jump` independently in every platform adapter. Test each
platform's physical normalization once, then test the shared binding once. That
is how the suite demonstrates reuse instead of merely checking duplicated
implementations for temporary agreement.

## Open Design Questions For The Tactical

These are implementation questions within the accepted direction, not reasons
to reopen semantic ownership:

1. Should the shared flat-event API be one enum, a few typed methods, or an
   `InteractiveInputState` facade?
2. Should desktop/Android winit normalization live in a small new adapter crate
   or an existing host-neutral integration module?
3. Should browser DOM listeners remain in TypeScript as raw forwarders or move
   to web Rust through `web_sys`? The ownership direction permits either; keep
   TypeScript if it is the simpler browser-mechanics layer.
4. How much pointer click/drag recognition is a generic platform gesture versus
   shared input policy? In either case, it must not select Attack/Use in TS.
5. Should shared touch layout/hit testing live in `mclone-input`, `mclone-ui`,
   or a narrow collaboration between them?
6. Which preference backend contract is sufficient for typed shared settings
   without building a general settings database prematurely?
7. What is the minimal production web operational state that support and page
   bootstrap genuinely require after the semantic mirror is removed?
8. Should smoke observer code be compile-feature-gated, query-gated but
   separately loaded, or a separate Wasm/test page? It must be absent from the
   ordinary production interface either way.
9. Which current `WebSceneHost` exports are true product operations, which are
   raw-adapter entry points, and which exist only for smoke?
10. Can initial local/remote construction become one browser-Rust bootstrap
    call without holding an exported mutable Wasm borrow across unrelated
    browser work?

## Stop Conditions

Stop for renewed review if:

- convergence would require flattening XR poses/tracking into a lossy flat
  input contract;
- a proposed universal adapter would force native through serialized or async
  browser mechanics;
- moving autonomous initialization into Rust adds round trips without a real
  Rust-owned decision;
- a browser API requires TypeScript to know domain meaning rather than merely
  execute a mechanical request;
- the shared router would become a new duplicate scene/client policy owner;
- preference cleanup would introduce runtime database migration semantics;
- removing production report fields reveals a real product UI or external
  support consumer that has not been modeled; or
- the change would weaken existing actor/mailbox ownership for Workers,
  sockets, persistence, render compilation, or catalog operations.

Routine Rust/TypeScript refactoring, generated binding changes, and browser API
mechanics are not stop conditions.

## Non-Goals

- zero TypeScript or moving DOM/IndexedDB/WebSocket/Worker APIs wholesale into
  Rust;
- one universal platform trait, Worker, executor, mailbox ABI, event loop, or
  byte representation;
- identical native/browser/XR control flow;
- routing every local browser invariant through a Rust effect request;
- redesigning game bindings, touch UI appearance, or interaction behavior as
  part of the ownership move;
- implementing a full end-user rebinding UI in the first slice;
- adding web audio, clipboard, IME, chat, or other missing product features;
- changing world database layout or the accepted legacy IndexedDB migration;
- replacing existing specialized actor/mailbox systems; or
- removing semantic APIs from explicit tests and offscreen harnesses.

## Code Map

Shared input and routing foundations:

- `native/crates/mclone-input/src/lib.rs`
- `native/crates/mclone-scene/src/mono.rs`
- `native/crates/mclone-scene/src/locomotion.rs`
- `native/crates/mclone-scene/src/host_effects.rs`
- `native/crates/mclone-scene/src/ui_panels.rs`
- `native/crates/mclone-app-runtime/src/client_experience.rs`
- `native/crates/mclone-app-runtime/src/platform_operation.rs`

Desktop, Android, and XR adapters:

- `native/apps/mclone-native-client/src/app.rs`
- `native/apps/mclone-native-client/src/winit_frame_driver.rs`
- `native/apps/mclone-native-client/src/offscreen_scene_host.rs`
- `native/apps/mclone-android-client/src/surface_driver.rs`
- `native/crates/mclone-xr-host/src/actions.rs`
- `native/crates/mclone-xr-host/src/frame_driver.rs`
- `native/apps/mclone-native-client/src/desktop_xr.rs`
- `native/apps/mclone-native-client/src/xr_emulation.rs`
- `native/apps/mclone-android-xr-client/src/lib.rs`

Browser interactive rim and diagnostics:

- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-web-input.ts`
- `native/apps/mclone-web-client/www/mclone-web-touch.ts`
- `native/apps/mclone-web-client/www/mclone-web-settings.ts`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`
- `native/apps/mclone-web-client/www/mclone-web-smoke.js`

Healthy browser-mechanics precedents to preserve:

- `native/apps/mclone-web-client/www/mclone-worker-transport.ts`
- `native/apps/mclone-web-client/www/mclone-remote-websocket-worker.ts`
- `native/apps/mclone-web-client/www/mclone-integrated-server-worker.ts`
- `native/apps/mclone-web-client/www/mclone-server-job-worker.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-worker.ts`
- `native/apps/mclone-web-client/www/mclone-web-persistence-executor.ts`
- `native/apps/mclone-web-client/www/mclone-web-world-catalog.ts`

Current enforcement:

- `scripts/check-web-worker-ownership.mjs`
- `scripts/check-web-scene-host-adoption.mjs`
- `native/apps/mclone-web-client/tests/web_scene_async_boundary_lock.rs`
- `native/apps/mclone-web-client/tests/scenario_parity_ownership_lock.rs`
  (Tactical 178's lobby-debt lock — adjacent enforcement, not a
  host-boundary lock)
- `native/apps/mclone-native-client/src/app/tests/input_adapter.rs`

## Related Documentation

- [`client-experience-architecture.md`](../client-experience-architecture.md)
  owns the broader one-client-experience rulebook.
- [`platform-parity.md`](platform-parity.md) owns the feature and shared-contract
  parity matrices and already calls for finishing the shared input-intent
  layer.
- [`cross-platform-operation-execution.md`](cross-platform-operation-execution.md)
  owns actor/mailbox and asynchronous platform-effect execution.
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md) records the
  completed shared browser scene-host cutover.
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md) owns
  Worker-resident Rust and domain-blind TypeScript topology.
- [`../tactical/197-domain-blind-web-worker-broker.md`](../tactical/197-domain-blind-web-worker-broker.md),
  [`../tactical/198-opaque-websocket-and-indexeddb-adapters.md`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md),
  and [`../tactical/202-web-scene-async-boundary-cleanup.md`](../tactical/202-web-scene-async-boundary-cleanup.md)
  are the immediate browser ownership history.

## Recommended Direction

Build on the structures that already work. Use XR host-to-snapshot-to-scene,
Worker-resident Rust actors, the WebSocket action executor, the persistence
record executor, `mclone-input`, `McloneSceneHost`, and `HostEffects` as the
patterns. Do not move the TypeScript input/report surface wholesale into
web-only Rust.

The shared interactive router is now proven on native typed paths and adopted
by flat Android and browser raw events. The diagnostic observer is isolated.
Active Tactical 206 should now move preference/bootstrap policy behind the
smaller production adapter and separate the always-on operational frame result
from the opt-in rich snapshot. It must preserve autonomous platform
initialization and healthy physical asymmetry.

The outcome is not “TypeScript contains fewer game words” in isolation. The
outcome is that every applicable host drives one shared Rust behavior path,
while each platform remains free to be a good native citizen of its own APIs.
