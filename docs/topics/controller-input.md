# Controller Input

Topic: `controller-input`

Status: active implementation; shared source/assignment and semantic controller
session foundations implemented as of 2026-07-22. The existing capability,
binding, prompt, and first-pass gamepad adapter foundation is retained, but no
product target currently polls a real gamepad. This topic
owns the durable all-target controller direction across desktop flat, web,
flat Android, desktop XR, Android XR, offscreen/test hosts, Steam Deck, and a
future native Steam Input integration. Tactical
[`098`](../tactical/098-flat-input-capability-convergence.md) remains the
bounded execution record for the existing flat-input slices.
[`Tactical 215`](../tactical/215-preliminary-couch-readiness.md) owns the
bounded session-local source identity and scripted 1-4-source assignment proof;
it deliberately does not add a physical gamepad collector.
[`Tactical 216`](../tactical/216-controller-input-foundation.md) owns the active
unattended implementation series through shared semantics, UI, physical
collectors, XR convergence, preferences, and automated validation. Real-device
acceptance remains a separate recorded gate.

## Top-Level Decision

Build one shared semantic input pipeline with several thin physical-device
collectors.

Do not force ordinary gamepads, Steam Input, and tracked XR controllers into
one universal hardware type. They have genuinely different capabilities and
binding systems. They converge at a shared action-state boundary, while XR
retains poses, rays, hand identity, tracking validity, and comfort-specific
input beside those actions.

```text
gilrs / Web Gamepad API / Android controller API
                       |
                       v
            StandardGamepadSnapshot
                       |
                       v shared bindings
OpenXR / Steam Input -> PlayerActionFrame
                       |
                       v
              shared scene input router
                  /               \
                 v                 v
        flat camera/UI       XR locomotion/UI
                                      +
                         tracked poses/rays/comfort
```

The meaningful reuse begins immediately after platform collection. Platform
APIs may deliver callbacks, event queues, or polled snapshots, but none of
them may own dead zones, gameplay bindings, edge semantics, input contexts,
UI navigation, prompt selection, or game behavior.

## Goals

- One gameplay and UI meaning for controller input across every client target.
- Ordinary USB, Bluetooth, built-in, and virtual gamepads on desktop, web, and
  Android.
- Controller use in desktop XR and Android XR alongside, or instead of,
  tracked controllers where the action is meaningful without a tracked pose.
- Steam Deck support through the ordinary Linux gamepad path, with a later
  native Steam Input path for Steam Controller, trackpads, gyro, action sets,
  and exact glyph origins.
- Mixed keyboard, mouse, touch, gamepad, and tracked-controller input without
  encoding a capability as a platform identity.
- Deterministic scripted/offscreen input and conformance tests using the same
  canonical snapshots and action frames as product hosts.
- A future path for haptics, controller-specific prompts, accessibility
  devices, and 1-4-player local multiplayer without replacing the foundation.
  Participant, view, and couch-session policy lives in
  [`local-couch-multiplayer.md`](local-couch-multiplayer.md).

## Non-Goals

- A catch-all platform adapter trait spanning input, windowing, storage,
  rendering, and lifecycle.
- Making tracked controllers pretend to be ordinary gamepads.
- Putting `winit`, `web-sys`, Android, OpenXR, GilRs, or Steamworks types in
  `mclone-input`.
- Treating successful in-world movement as complete controller support while
  title, pause, options, world selection, and first-run UI remain inaccessible.
- Native Steam Input in the first gamepad slice.
- Local multiplayer in the first gamepad slice. Source identity and assignment
  must permit it later, but one local player remains the current product shape.
  The accepted downstream cardinality and assignment consumer are recorded in
  [`local-couch-multiplayer.md`](local-couch-multiplayer.md).

## Current State And Gaps

The current code has the right shared owner and the preliminary couch-safe
source seam, but not yet the complete semantic or physical-input contract:

- `mclone-input` owns `InputDeviceKind`, capability and last-active-device
  state, preferences, prompt resolution, `GamepadBindings`, radial dead zones,
  and `GamepadInputAdapter`.
- `mclone-input` now also owns session-local `InputSourceId`, neutral source
  descriptors, the full normalized `StandardGamepadSnapshot`, and a bounded
  scripted 1-4-seat assignment/reconnect reducer. Snapshot controls feed the
  existing shared bindings. This preliminary reducer deliberately stops before
  durable participants, profiles, or product joining.
- `ControllerInputSession` now owns Gameplay/Menu/TextEntry contexts,
  per-source semantic held/pressed/released actions, movement and look-rate
  axes, dead-zone/curve policy, trigger hysteresis, controller navigation
  repeat, lifecycle clearing, and active-source/layout arbitration. A bounded
  flat-frame projection applies look rate with `dt` while hosts migrate.
- No desktop, browser, or Android product adapter currently advertises a real
  gamepad or feeds that adapter. The repository explicitly records this in
  [`platforms.md`](../platforms.md) and Tactical 098.
- The legacy `GamepadInputAdapter` remains one controller at a time and still
  projects right-stick state directly into `FlatInputFrame`. The new semantic
  session fixes those constraints, but shipping hosts have not adopted it yet;
  participant-scoped routing also remains downstream couch work.
- `MonoInteractiveInputRouter` owns the shared keyboard/mouse resolver but
  receives touch as a supplemental frame and does not own gamepad state.
  Desktop, Android, and web separately retain capability/preference facts.
- `mclone-ui::GuiKey` currently exposes only Escape and F1. There is no shared
  directional navigation, confirm/back, focus traversal, or deterministic
  controller repeat policy.
- The current gamepad HUD renders literal Xbox-style `A/B/X/Y/LB/RB` labels.
  It does not resolve prompts from controller family, active binding, or an
  action origin.
- `XrControllerSnapshot` is correctly host-neutral, but it combines spatial
  tracking with physically named controls such as `a_pressed`, `b_pressed`,
  `y_pressed`, trigger, squeeze, and thumbstick. The OpenXR host similarly
  creates physically named actions and the scene later assigns their meaning.

These are foundation gaps, not reasons to add platform-local gameplay maps.

## Accepted Shared Contracts

The exact Rust names may adjust during implementation, but the information
and ownership boundaries are accepted.

### Source identity and description

`mclone-input` should define a session-local `InputSourceId` and an
`InputSourceDescriptor`. A descriptor should report only neutral facts needed
by shared policy and presentation:

- source class: keyboard/mouse, touch, gamepad, tracked controller, Steam
  Input, scripted/test;
- controller layout family: Xbox-like, PlayStation-like, Nintendo-like,
  Steam/Deck-like, generic, or unknown;
- available axes, buttons, touch surfaces, motion sensors, and haptic channels;
- connected/available state and a non-persisted display label where safe;
- optional backend action-origin/glyph capability.

Platform-local device handles, browser indices, Android device IDs, GilRs IDs,
OpenXR paths, and Steam Input handles stay behind their collectors. Shared IDs
are never persisted as durable device identity.

### Standard gamepad snapshot

Platform collectors for ordinary gamepads should emit one normalized
`StandardGamepadSnapshot` per connected source at the presentation/input
sample boundary. It should contain:

- left and right 2D sticks in `[-1, 1]`;
- left and right triggers in `[0, 1]`;
- south/east/west/north face buttons;
- left/right shoulders and stick clicks;
- D-pad directions;
- Start, Select/Back, and Guide/System where exposed;
- analog button values when available, plus normalized pressed state;
- monotonic sample information only when it has cross-platform meaning.

Snapshot input is the common denominator because the browser is naturally
polled, GilRs offers cached state plus events, Android delivers raw events,
and test hosts synthesize state. Platform event frequency and repeat behavior
must not leak into shared action semantics.

Unknown or non-standard controls may be retained in a bounded extension for
diagnostics and future binding support. A backend must not guess that an
unrecognized raw index is a standard control.

### Semantic action frame

All device families converge on a `PlayerActionFrame` or equivalent containing:

- continuous movement axis;
- continuous look/turn rate;
- pointer-like look delta for mouse and touch;
- held actions;
- newly pressed edges;
- newly released edges;
- the source responsible for meaningful recent activity.

Initial shared actions include:

- move and look/turn;
- jump, sprint, sneak, and descend;
- attack/break and use/place;
- select hotbar slot and previous/next hotbar slot;
- pause/menu, block palette/inventory, help, and camera-view toggle;
- UI navigate, confirm, back, and previous/next page or tab.

XR-only teleport, hand-push, thruster, grip-pose, and comfort facts remain in a
typed XR extension. Future vehicle, inventory, chat, or accessibility actions
extend the semantic vocabulary rather than adding platform-local shortcuts.

### Input contexts

Bindings resolve under an explicit shared `InputContext`. The initial contexts
are:

- `Gameplay`;
- `Menu`;
- `TextEntry`.

The scene owns context selection because it owns the active UI and gameplay
state. A controller's south button may mean Jump in gameplay and Confirm in a
menu without the platform collector knowing either word. Context changes also
provide the exact seam required by OpenXR action sets and future Steam Input
action sets.

### Stateful resolver and mixer

A shared `InputSession`, `ControllerInputState`, or similarly focused owner
should retain per-source previous snapshots and implement:

- radial stick dead zones and response curves;
- independent look and movement tuning;
- trigger press/release hysteresis;
- digital pressed/released edge generation;
- controller navigation repeat delay/rate;
- significant-activity detection after dead-zone filtering;
- capability and last-active-device tracking;
- disconnect, focus-loss, pause, and visibility-loss clearing;
- source arbitration and mixed-device composition.

Mouse/touch deltas and controller look rates remain distinct through the
action frame. The scene applies look rate using `dt`; this makes controller
turning invariant across 60, 72, 90, 120, and uncapped presentation rates.

Keyboard, mouse, touch, and the active gamepad may compose. Until local
multiplayer exists, continuous analog input from multiple gamepads uses the
most recently meaningfully active gamepad rather than summing unrelated
controllers. Digital edges may be accepted from any connected assigned
gamepad. Future player assignment extends this policy using `InputSourceId`
without changing snapshots or bindings.

The accepted couch path assigns one `InputSourceId` to at most one
session-local participant. An unassigned gamepad may contribute a deliberate
join edge, but backend device IDs are never persisted as player identity.
Keyboard/mouse normally form one composite participant source; controller-only
player-one boot and independent per-participant input contexts/prompts are
required. Exact join, reconnect, guest/profile, and participant policy belongs
to the couch topic rather than platform collectors.

Axis noise below the configured dead zone is not activity and must not switch
the active prompt. Disconnect or lifecycle loss clears held state so a missing
release cannot leave movement or an action stuck.

## Provisional Default Gamepad Bindings

Defaults should use position-neutral control names, never Xbox labels:

| Control | Gameplay action | Menu action |
|---|---|---|
| Left stick | Move | Navigate |
| Right stick | Look / turn | Optional scroll or no action |
| Right trigger | Attack / break | — |
| Left trigger | Use / place | — |
| South face | Jump | Confirm |
| East face | Sneak / descend | Back |
| Left-stick click | Sprint | — |
| Left/right shoulder | Previous/next hotbar | Previous/next tab where applicable |
| D-pad | Hotbar or future quick actions | Navigate |
| Start | Pause / menu | Resume or context-owned menu action |
| Select/Back | Block palette/inventory | Back or secondary action |

The exact inventory and D-pad defaults may evolve with the inventory surface.
Binding persistence stores semantic control/action names and schema version,
not backend numeric codes or a device instance ID.

## Ownership

### `mclone-input`

Owns:

- source IDs, descriptors, layouts, and capabilities;
- standard gamepad snapshots;
- binding tables, contexts, dead zones, curves, trigger thresholds, and
  controller sensitivity;
- action held/edge state, input mixing, active-source policy, and prompt tokens;
- deterministic tests and scripted input fixtures;
- future neutral haptic requests and targets.

It must not depend on `winit`, Android activity types, `web-sys`, OpenXR,
GilRs, Steamworks, rendering, scene, or app crates.

### `mclone-scene`

Owns:

- the shared input session used by product hosts;
- current gameplay/menu/text context;
- routing semantic actions to UI, camera, locomotion, hotbar, interaction, and
  client commands;
- flat versus XR application of look/turn policy;
- selecting a head-gaze or tracked-controller interaction ray;
- merging ordinary action state with XR-specific tracked input;
- publication cadence after presentation-rate input application.

`MonoInteractiveInputRouter` should evolve or be composed into this owner. The
goal is one logical router, not necessarily one giant Rust type or a breaking
rename in the first slice.

### `mclone-ui`

Owns:

- directional focus navigation and traversal order;
- confirm/back/page action handling;
- focused widget rendering;
- prompt rendering from semantic action and controller-layout/glyph facts;
- controller-accessible title, world, pause, options, help, and confirmation
  screens.

It does not poll devices or decide gameplay bindings.

### Platform adapters

Own only:

- device/API construction and shutdown;
- hotplug callbacks or enumeration;
- OS/browser/Android event receipt or polling;
- raw-to-standard control normalization;
- platform focus, pause, visibility, and permission facts;
- executing haptic requests through the selected backend;
- device validation and packaging.

## Platform Backend Plan

| Target | Physical collector | Shared destination |
|---|---|---|
| Desktop flat | GilRs | `StandardGamepadSnapshot` |
| Desktop XR | Same GilRs collector beside OpenXR | ordinary snapshot plus tracked XR input |
| Web/WASM | Browser Gamepad API near `requestAnimationFrame` | `StandardGamepadSnapshot` |
| Flat Android | Android raw controller collector in `mclone-android-platform` | `StandardGamepadSnapshot` |
| Android XR | Same Android collector beside OpenXR | ordinary snapshot plus tracked XR input |
| Offscreen/test | scripted snapshots or semantic frames | exact shared resolver/router |

### Desktop and Steam Deck

GilRs is the preferred ordinary desktop backend. Its current documented
support covers Linux/BSD, Windows, macOS, and Wasm, including hotplugging,
unified controller layout, SDL-compatible mappings, and mappings supplied by
Steam through `SDL_GAMECONTROLLERCONFIG`. It explicitly does not support
Android.

Desktop flat and desktop XR are in the same native app, so one app-local or
small platform-input collector can serve both without adding GilRs to
`mclone-input`. Steam Deck is the ordinary Linux path, not a separate engine
target.

### Web

The browser collector should use `navigator.getGamepads()` and sample as close
as possible to the existing animation-frame input boundary. The W3C standard
layout defines four stick axes and seventeen canonical buttons when the
browser reports `mapping == "standard"`.

Direct `web-sys` polling is the initial recommendation because it is small,
fits the existing browser host, and keeps browser lifecycle facts explicit.
GilRs also has a Wasm backend and may be adopted after a bounded spike if it
reduces total code and preserves the host-boundary source locks. It cannot be
the universal solution because Android remains unsupported.

The TypeScript product adapter may install connection listeners and forward
mechanical browser facts, but it must not contain action names, button maps,
dead zones, or gameplay decisions. Browser Rust produces the canonical
snapshot and feeds the shared resolver before each frame.

### Android

The preferred production-quality Android collector is a thin Rust/FFI wrapper
around Android's Game Controller Library (Paddleboat), owned by
`mclone-android-platform`. Paddleboat supplies connection callbacks,
standardized dual-stick controller data, layout information, mapping data,
haptics, motion sensors, and battery facts. It supports both `NativeActivity`
and `GameActivity`, but requires raw `AInputEvent` or `GameActivity` events to
be forwarded before its per-frame update/read step.

Android XR already polls `android_activity` directly and can forward raw input
to this collector. Flat Android currently lets `winit` own the Android input
queue. Its implementation therefore needs a narrow raw-controller event seam,
preferably upstreamable to `winit`, so joystick motion is identified by input
source and forwarded instead of being interpreted as touchscreen motion. Do
not replace the established Android surface/lifecycle host solely to obtain
gamepads.

If Paddleboat packaging and C++ FFI are disproportionate for the first proof,
a pure-Rust `android_activity` normalizer may temporarily map Android's
documented standard axes and key codes behind the same
`StandardGamepadSnapshot`. The shared contract and validation bar do not
change, and the adapter must still inspect `SOURCE_GAMEPAD`, `SOURCE_DPAD`,
and `SOURCE_JOYSTICK` before assigning meaning.

## XR Convergence

OpenXR and ordinary gamepads should share semantic actions while preserving a
separate tracked-data channel.

The target neutral shape is conceptually:

```rust
struct XrInputFrame {
    actions: PlayerActionFrame,
    tracked: Vec<TrackedControllerState>,
    xr_specific: XrSpecificInput,
}
```

`TrackedControllerState` retains:

- hand identity;
- aim and grip positions/orientations;
- tracking validity and activity;
- pointer/ray facts and any later hand-tracking extension.

`XrSpecificInput` retains teleport, hand-push, thruster, and comfort-specific
state that has no ordinary flat-gamepad meaning.

`mclone-xr-host::OpenXrControllerActions` should migrate from physically named
application actions such as `right_a_click` and `left_y_click` toward semantic
actions such as Jump, Sprint, Move, Turn, Attack, Use, Menu, and UI Select.
OpenXR suggested interaction-profile bindings then decide which physical
control supplies each action. This follows OpenXR's own action model and makes
the common action frame the natural boundary rather than an emulated gamepad.

Desktop and Android XR continue to call the one shared scene frame path. The
OpenXR host still owns action synchronization and space location; the scene
still owns locomotion, comfort, UI, and gameplay meaning.

An ordinary gamepad used in XR supplies actions but no tracked pose. Movement,
turning, jump, sprint, pause, and focused menu navigation work normally. Attack
and use fall back to a scene-owned head-gaze/crosshair ray. When a tracked ray
is available, the scene continues to prefer the relevant tracked interaction
source. Hand-push, thruster, and tracked teleport remain unavailable to an
ordinary gamepad unless they receive a deliberate non-spatial alternative.

## Steam Input Direction

Basic Steam Deck and Steam Controller compatibility may arrive through
Steam's legacy/virtual gamepad output and the ordinary GilRs path. This is not
the final native Steam Controller experience.

A later optional `SteamInputSource` should consume Steam Input's digital and
analog actions directly and emit `PlayerActionFrame`. It must not translate
Steam actions back into `GamepadControl`, because Steam Input is already an
action-based API and supports user bindings, action sets, layers, trackpads,
gyro, and action-origin glyphs.

The shared `InputContext` drives Steam action-set selection such as Gameplay
and Menu. Steam-provided action origins feed prompt/glyph projection. The
backend remains optional so non-Steam builds and all other platforms retain
the ordinary gamepad path.

When native Steam Input is active, source arbitration must suppress or
de-duplicate the corresponding virtual Xbox/GilRs device so one physical
press cannot arrive twice.

## UI And Prompt Requirements

Full controller support requires every non-text product flow to be navigable
without a pointer:

- title and scenario entry;
- world list, create, delete confirmation, and open;
- pause and resume;
- options categories, sliders, toggles, and back navigation;
- help and block palette/inventory;
- connection failure and recovery UI;
- XR world panels through either ray input or focus navigation.

`mclone-ui` already has focused-widget visual state, but it needs an explicit
navigation model. Prefer stable per-screen focus order with spatial movement
only where a grid requires it. Analog-stick navigation uses threshold and
hysteresis plus shared initial-delay/repeat timing; raw platform key repeat is
not authoritative.

Prompt rendering consumes an action plus an origin/layout token. Examples:

```text
Attack -> RT / R2 / ZR / Steam action glyph
Jump   -> A / Cross / B / Steam action glyph
Back   -> B / Circle / A / Steam action glyph
```

The backend or user override selects layout family. The UI never assumes that
the south button is labeled `A`.

## Haptics And Advanced Capabilities

Haptics are not required for the first input slice, but the source descriptor
should record output capabilities and the shared boundary should reserve a
neutral request such as:

```text
HapticRequest { target, low_frequency, high_frequency, duration }
```

Scene/gameplay code decides when and why feedback occurs. Platform collectors
execute it through GilRs, Paddleboat, OpenXR, or Steam Input. A request may
target the active gamepad, a specific source, or an XR hand. Capability absence
is a normal no-op, not a gameplay fork.

Gyro, controller touchpads, adaptive triggers, lights, and battery display are
extensions of source capabilities. They must not complicate the initial
standard snapshot or make the shared gameplay path conditional on a vendor.

## Implementation Sequence

1. **Shared state contract.** Add source identity/description, a full standard
   snapshot, semantic held/pressed/released action state, input contexts, and
   a multi-source reducer. Separate look rate from pointer delta. Preserve
   compatibility entry points only where they keep the first slice bounded.
2. **Shared router ownership.** Put gamepad state and capability/activity
   policy into the shared scene input owner. Remove app-local semantic merging
   and make focus/lifecycle clearing exhaustive.
3. **Shared UI completion.** Add navigation, confirm/back/page actions, focus
   traversal, repeat policy, and action/layout-based prompts. Prove every
   non-text menu with scripted controller input.
4. **Synthetic and desktop proof.** Feed canonical snapshots through the
   offscreen/test host, then add the GilRs desktop collector for flat and XR
   modes. Validate ordinary desktop controllers and Steam Deck.
5. **Browser adoption.** Poll browser gamepads at the frame boundary, normalize
   the standard mapping in browser Rust, update real capability/activity, and
   retain TypeScript as mechanical glue.
6. **Android adoption.** Land the raw-controller input seam, one shared Android
   collector, and both flat-Android and Android-XR consumption. Validate wired
   and wireless real devices rather than relying only on AVD key injection.
7. **XR action convergence.** Convert OpenXR application actions to semantic
   action state plus tracked extensions, preserve existing tracked-controller
   behavior, and prove ordinary gamepad use inside both XR targets.
8. **Preferences and rebinding.** Persist controller sensitivity, inversion,
   dead zones, layout override, preferred input, and schema-versioned bindings
   through shared preference codecs and platform storage executors.
9. **Optional advanced backends.** Add Steam Input, haptics, exact glyph
   origins, gyro/touchpads, accessibility extensions, and durable
   source-to-profile/participant association beyond the preliminary seat
   reducer. Adopt the participant contract from
   [`local-couch-multiplayer.md`](local-couch-multiplayer.md) rather than
   introducing a controller-local player model.

Tactical 215 completed source identity/descriptor, canonical snapshots, and
bounded scripted assignment. Tactical 216 Slice 1 completed semantic
per-source action state, look-rate semantics, input contexts, controller
navigation repeat, lifecycle clearing, and controller-to-controller
arbitration. Shared scene adoption, cross-class mixing, and UI traversal remain
before a physical collector.

Do not wire a platform backend before Slices 1–3 provide the complete shared
gameplay and UI destination. Otherwise the first platform will accidentally
define product policy at its adapter rim.

## Validation Contract

### Shared unit and contract tests

`mclone-input` tests must cover:

- canonical standard-layout normalization fixtures;
- radial dead zones, response curves, trigger hysteresis, and non-finite input;
- held, pressed, and released edges without platform repeat dependence;
- right-stick look invariance across representative frame rates;
- activity detection that ignores drift;
- disconnect, focus loss, pause, and visibility clearing;
- multiple connected gamepads and active-source selection;
- mixed keyboard/touch/gamepad composition;
- binding serialization, schema defaults, and controller-layout override;
- action-context changes without stuck or replayed inputs.

`mclone-scene` tests must feed equivalent semantic traces from keyboard,
gamepad, and XR sources and compare gameplay/camera/UI outcomes where their
capabilities overlap. Pose/ray/comfort behavior remains separately tested.

`mclone-ui` tests must exercise focus order, grids, disabled controls, sliders,
confirmation flows, repeat behavior, back navigation, and layout-sensitive
prompts on every screen.

### Platform gates and device evidence

- Desktop flat: real Xbox-layout, PlayStation-layout, and generic controller;
  hotplug, reconnect, focus loss, gameplay, menus, and prompts.
- Steam Deck: built-in controls through the Linux path, suspend/resume, menus,
  and prompt layout; Steam Controller basic path where available.
- Web: mapping conversion tests plus a real controller in supported browsers;
  connect/disconnect, visibility loss, gameplay, menus, and prompt switching.
- Flat Android: wired and Bluetooth controllers, gameplay and every menu,
  mixed controller/touch input, pause/resume, and no false touch events from
  joystick motion.
- Desktop XR and Android XR: tracked controllers unchanged, ordinary gamepad
  locomotion and UI navigation, head-gaze interaction fallback, and mixed
  tracked/gamepad source arbitration.
- Offscreen: deterministic scripted snapshot/action trace suitable for CI and
  future replay/input-source work.

Any prompt, focus, or HUD slice produces pixels and therefore requires a
screenshot captured outside the repository and inspected at the first drawable
milestone, following the repository rendered-output policy.

Android's published controller-quality checklist is a useful acceptance floor:
controller input must work in menus and first-run flow, both sticks must avoid
drift, triggers must not double-fire, held buttons must not repeat accidentally,
mixed touch/controller input should work, and multiple common controller
layouts require device testing.

## External Evidence

- [GilRs documentation](https://docs.rs/gilrs/latest/gilrs/) — current
  desktop/Wasm platform support, normalized layout, hotplugging, SDL mappings,
  Steam mapping environment support, and explicit lack of Android support.
- [W3C Gamepad specification](https://www.w3.org/TR/gamepad/) — standard
  mapping, canonical button/axis indices, connection model, and recommendation
  to sample near `requestAnimationFrame`.
- [Android controller input](https://developer.android.com/games/sdk/game-controller/controller-input)
  — source classification, key-versus-motion delivery, standard axes/buttons,
  D-pad variants, and repeat handling.
- [Android Game Controller Library](https://developer.android.com/games/sdk/game-controller)
  and [usage guide](https://developer.android.com/games/sdk/game-controller/controller)
  — Paddleboat mapping, connection, layout, haptics, raw-event forwarding, and
  per-frame state access.
- [Android controller testing](https://developer.android.com/games/sdk/game-controller/testing_controller)
  — full-menu, drift, trigger, repeat, mixed-input, layout, wired, and wireless
  validation expectations.
- [OpenXR 1.1 specification](https://registry.khronos.org/OpenXR/specs/1.1-khr/html/xrspec.html)
  — semantic actions, action sets, interaction profiles, suggested bindings,
  and tracked action spaces.
- [Steam Input API](https://partner.steamgames.com/doc/api/isteaminput?language=english)
  and [Steam Input overview](https://partner.steamgames.com/doc/features/steam_controller)
  — action-based controller support, action sets, analog/digital actions,
  Steam Controller and major controller families.

## Code And Documentation Map

- [`../../native/crates/mclone-input/src/lib.rs`](../../native/crates/mclone-input/src/lib.rs)
  — current capabilities, preferences, bindings, flat frames, gamepad adapter,
  and neutral XR snapshots.
- [`../../native/crates/mclone-input/src/controller_session.rs`](../../native/crates/mclone-input/src/controller_session.rs)
  — semantic contexts/actions, per-source reduction, look-rate projection,
  hysteresis, navigation repeat, lifecycle clearing, and arbitration.
- [`../../native/crates/mclone-scene/src/interactive_input.rs`](../../native/crates/mclone-scene/src/interactive_input.rs)
  — shared mono input/context/action router.
- [`../../native/crates/mclone-scene/src/locomotion.rs`](../../native/crates/mclone-scene/src/locomotion.rs)
  and [`../../native/crates/mclone-scene/src/ui_panels.rs`](../../native/crates/mclone-scene/src/ui_panels.rs)
  — current XR controller interpretation, locomotion, interactions, and
  world-panel pointer behavior.
- [`../../native/crates/mclone-xr-host/src/actions.rs`](../../native/crates/mclone-xr-host/src/actions.rs)
  — OpenXR action creation, profile bindings, polling, and neutral snapshots.
- [`../../native/crates/mclone-ui/src/lib.rs`](../../native/crates/mclone-ui/src/lib.rs)
  — current UI key vocabulary, focus visuals, HUD, and literal gamepad prompts.
- [`../../native/apps/mclone-native-client/src/winit_frame_driver.rs`](../../native/apps/mclone-native-client/src/winit_frame_driver.rs)
  — desktop flat input/cadence rim.
- [`../../native/apps/mclone-web-client/src/web_scene_host.rs`](../../native/apps/mclone-web-client/src/web_scene_host.rs)
  — browser Rust scene/input owner and animation-frame boundary.
- [`../../native/apps/mclone-android-client/src/surface_driver.rs`](../../native/apps/mclone-android-client/src/surface_driver.rs)
  — flat Android `winit` input and lifecycle rim.
- [`../../native/apps/mclone-android-xr-client/src/lib.rs`](../../native/apps/mclone-android-xr-client/src/lib.rs)
  — Android XR raw activity polling and OpenXR frame/input path.
- [`platform-host-boundary.md`](platform-host-boundary.md) — controlling
  shared/platform ownership rules for all interactive input.
- [`platform-parity.md`](platform-parity.md) and
  [`../platforms.md`](../platforms.md) — platform matrix, shared owner, and
  validation routing.
- [`local-couch-multiplayer.md`](local-couch-multiplayer.md) — accepted 1-4
  local-participant assignment, split/auxiliary view, helper, and mixed
  XR-plus-flat consumer of this input foundation.
- [`../tactical/098-flat-input-capability-convergence.md`](../tactical/098-flat-input-capability-convergence.md)
  — existing flat capability/gamepad foundation and bounded execution record.

## Definition Of Done

Controller input is complete only when:

1. all five product clients can consume the same semantic action contracts;
2. desktop, web, and Android poll real ordinary gamepad sources;
3. both XR clients preserve tracked-controller behavior and also accept an
   ordinary gamepad for the supported non-spatial actions;
4. every non-text product UI flow is navigable with a controller;
5. look, edges, hotplug, lifecycle clearing, mixed input, prompts, and layouts
   meet the shared validation contract;
6. no app, TypeScript module, Android adapter, OpenXR host, or Steam backend
   owns a second gameplay/UI action map; and
7. current platform and rendered-output sentinel gates pass, plus recorded
   real-device evidence for desktop, browser, Android, and XR.
