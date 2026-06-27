# 098: Flat Input Capability Convergence

Status: active; Slices 1-2 shared input contract and desktop adapter
convergence are implemented and validated. Slice 3 Android touch
interaction/hotbar convergence is implemented; attached Android
keyboard/mouse routing remains. This splits the remaining flat Android
input/HUD parity work out of
[`090-flat-android-client-parity.md`](090-flat-android-client-parity.md) into a
shared flat-client capability contract for desktop, flat Android, and web.

## Purpose

Prevent new flat-client platform divergence by making input capability and user
preference explicit shared concepts.

The problem is not "desktop has keyboard/mouse" and "Android has touch." A
desktop machine may have a touchscreen. A flat Android device may have a USB or
Bluetooth keyboard, mouse, or gamepad. Web may expose touch, mouse, keyboard,
and gamepad through browser APIs. Those are capabilities, not platform
identities.

Flat clients should share gameplay-facing input semantics:

- move
- look
- jump
- sprint
- descend / sneak
- attack / break
- use / place
- select hotbar slot
- next / previous hotbar slot
- open menu / pause

Platform code should only collect raw events and lifecycle facts. Shared input
code should turn device capability state, bindings, presets, and user
preferences into flat gameplay/UI intents. Shared client/gameplay code should
apply those intents through the same movement, hotbar, block interaction, UI,
and runtime command paths on every flat platform.

XR may still diverge at the world-space presentation, controller pose/ray, and
comfort-policy layers, but XR controllers can still feed shared gameplay
intents where the semantics match.

## Current State

- Desktop flat has the most complete keyboard/mouse gameplay path, but that
  path is app-local in `mclone-native-client`.
- Desktop does not currently expose the touch-control path for touchscreen
  devices, even though `winit` can report touch events on desktop.
- Flat Android has shared menu UI, touch movement/look, and touch
  attack/use/hotbar routed through shared intents and the shared
  `ClientInteractionController`.
- Flat Android does not currently route attached keyboard/mouse/gamepad input
  into the same gameplay path desktop uses.
- Native web has separate browser keyboard/mouse/touch glue. It should converge
  on the same intent and preference contracts while keeping browser event,
  worker, and storage mechanics web-local.
- `mclone-ui` owns shared menu rendering and the current touch movement plus
  touch interaction overlay, but it does not yet own a shared flat gameplay
  HUD/hotbar across desktop, Android, and web.

## Target Shape

Add a shared input crate, tentatively `mclone-input`, with no dependency on
`winit`, Android activity types, browser `web-sys` types, OpenXR types, or app
crates.

The shared crate should own:

- flat input device/capability identifiers
- user input preferences and scheme presets
- binding tables for keyboard/mouse, touch, and gamepad
- "last active device" and capability-resolution policy
- per-frame flat gameplay/UI intents
- tests for preset and capability resolution

The shared crate should not own:

- OS/window/browser event loops
- cursor locking, Android lifecycle, DOM events, or OpenXR sessions
- actual rendering of UI/HUD geometry
- server/runtime command dispatch
- gameplay validation or world mutation

Recommended ownership:

- `mclone-input`
  - `InputDeviceKind`: keyboard, mouse, touch, gamepad, xr-controller
  - `FlatInputIntent`: gameplay/UI commands such as movement, look, attack,
    use, hotbar selection, and menu open
  - `FlatInputFrame`: accumulated analog/digital intents for one client tick
  - `InputPreferences`: persisted user choices
  - `InputSchemePreset`: auto, keyboard/mouse, touch, gamepad, custom
  - `TouchControlsMode`: auto, on, off
  - capability-resolution helpers that decide whether touch controls should be
    shown and which prompts/hints are preferred
- `mclone-ui`
  - shared flat HUD presentation: crosshair, hotbar/debug palette, selected
    slot state, touch buttons, and optional gamepad-friendly focus affordances
  - draw-only state for touch controls; no gameplay semantics
- `mclone-client`
  - `ClientInteractionController`, selected hotbar, carried-item sync, block
    raycast, and command construction
- app crates
  - raw event collection and platform lifecycle
  - adapter code from raw events into `mclone-input` updates
  - user preference persistence per platform
  - final runtime command dispatch through shared client/app-runtime APIs

## Capability And Preference Rules

The default scheme is `Auto`.

`Auto` must be capability-based:

- If no keyboard/mouse/gamepad is detected and touch is present, show on-screen
  controls by default.
- If keyboard/mouse appears on Android, accept it without disabling touch.
- If touch appears on desktop, accept it and allow the touch HUD to become
  visible through `Auto` or a forced `Touch Controls: On` preference.
- If gamepad appears on any flat platform, accept it as another input source
  and let it drive movement/look/actions through the same intent layer.
- If multiple devices are active, compose them unless a later accessibility or
  debugging setting explicitly asks for exclusive input.

User preferences override auto policy:

- `Preferred Input: Auto` chooses prompts/HUD affordances from capability state
  and last active device.
- `Preferred Input: Keyboard/Mouse` prefers keyboard/mouse prompts and hides
  automatic touch controls, but still accepts touch/gamepad input.
- `Preferred Input: Touch` prefers touch prompts and shows touch controls when
  a touch surface exists, including on desktop touchscreens.
- `Preferred Input: Gamepad` prefers gamepad prompts and bindings when a gamepad
  exists, but does not disable keyboard/mouse/touch.
- `Touch Controls: Auto / On / Off` controls only the on-screen overlay. It
  should not disable touch menu pointing or touch gestures unless an explicit
  future setting says so.

The important invariant: **a platform never owns or forbids a capability.**
Platforms expose raw events; capability adapters turn those events into shared
intents; preferences decide presentation defaults.

## Implementation Slices

- [x] **Slice 1: shared input contract.**
  - Add `mclone-input` to the native workspace.
  - Define device kinds, input preferences, scheme presets, touch-control mode,
    flat input intents, and per-frame intent accumulation.
  - Add unit tests for auto/preset resolution, including touchscreen desktop,
    touch-only Android, Android with keyboard/mouse, and gamepad-present cases.
  - Do not move platform raw event types into the crate.

- [x] **Slice 2: desktop adapter convergence.**
  - Replace desktop app-local gameplay key/mouse dispatch with an adapter that
    emits shared flat input intents.
  - Keep cursor-lock and window focus policy in the desktop app.
  - Route desktop `WindowEvent::Touch` into the same touch capability path used
    by other flat clients.
  - Preserve current desktop keyboard/mouse behavior while changing ownership.

- [ ] **Slice 3: flat Android capability convergence.**
  - [x] Add `ClientInteractionController` to flat Android.
  - [x] Route Android touch attack/use/hotbar through shared intents and the same
    block-pick / carried-item / gameplay-command path as desktop.
  - [ ] Route attached Android keyboard/mouse events into the shared keyboard/mouse
    adapter when `winit` exposes them.
  - [x] Keep Android lifecycle, surface, property lookup, and APK validation in the
    Android app crate.

- [ ] **Slice 4: shared flat HUD and touch controls.**
  - Move crosshair, hotbar/debug palette, selected-slot display, and remaining
    cross-platform touch-control policy into shared `mclone-ui` presentation.
  - Let `mclone-ui` consume resolved overlay state from `mclone-input`; keep
    gameplay command semantics outside the UI crate.
  - Support `Touch Controls: Auto / On / Off` across desktop, Android, and web.

- [ ] **Slice 5: gamepad capability foundation.**
  - Add a platform-neutral gamepad intent mapping to `mclone-input`.
  - Evaluate native gamepad backend ownership separately for desktop/Android
    and browser Gamepad API ownership for web.
  - Wire a first flat gamepad mapping: left stick move, right stick look,
    face buttons jump/use/attack/menu, shoulders or d-pad hotbar.
  - Keep XR controller pose/ray logic outside this flat gamepad slice.

- [ ] **Slice 6: web adapter convergence.**
  - Rework native-web keyboard/mouse/touch/gamepad glue to emit the same shared
    intent and preference facts.
  - Keep browser worker, canvas, localStorage, pointer-lock, and Gamepad API
    polling web-local.
  - Preserve existing web smoke coverage while reducing browser-only gameplay
    input logic.

- [ ] **Slice 7: preference persistence and options UI.**
  - Add shared menu/options controls for preferred input scheme and touch
    controls mode.
  - Persist preferences through platform-local storage:
    - desktop config path
    - Android app storage or startup property bridge until persistent settings
      are in place
    - web localStorage through existing browser settings glue
  - Keep defaults deterministic for automated smokes.

## Validation

Shared code gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-input -p mclone-ui -p mclone-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target x86_64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
pnpm native:web:smoke
git diff --check
```

Rendered/device validation after HUD or touch changes:

```bash
pnpm native:android:avd-touch-smoke
pnpm native:android:quest-flat
pnpm native:web:smoke
```

Manual/device validation to record before closing this tactical:

- Desktop touchscreen: touch menu, touch look/move, attack/use, and hotbar
  selection.
- Desktop keyboard/mouse: existing behavior unchanged.
- Desktop gamepad: first gamepad mapping drives move/look/actions.
- Flat Android touch-only: on-screen controls visible by default in `Auto`.
- Flat Android with keyboard/mouse: keyboard/mouse controls work without
  disabling touch.
- Flat Android with gamepad: gamepad controls work without disabling touch.
- Web touch and keyboard/mouse: shared intents preserve current behavior.

## Guardrails

- Do not encode `desktop == keyboard/mouse` or `Android == touch`.
- Do not add Android-only or desktop-only gameplay input semantics.
- Do not let touch controls own gameplay behavior; touch controls emit shared
  intents.
- Do not let `mclone-input` depend on `winit`, Android, browser, OpenXR, app
  crates, renderer crates, or server crates.
- Do not put visible DOM HUD/menu UI back into the web app.
- Do not make input schemes exclusive by default; mixed input should work.
- Do not hide touch controls permanently just because keyboard/mouse or gamepad
  appeared. User preference must be able to force them on.
- Do not route XR controller pose/ray/world-space menu ownership through the
  flat input layer. Only shared gameplay-equivalent intents should cross that
  boundary.

## Status Log

- Created: captured the flat-client input capability model so future Android,
  desktop, web, touch, keyboard/mouse, and gamepad work converges instead of
  adding more platform-specific gameplay adapters.
- Slice 1 landed: added `mclone-input` with platform-neutral capability state,
  serializable preferences, resolved prompt/touch-overlay policy, keyboard/mouse,
  touch, and gamepad binding tables, per-frame flat input intent accumulation,
  and unit tests for touch-only, desktop touchscreen, Android keyboard/mouse
  plus touch, explicit preference, gamepad, and hotbar/frame behavior.
- Slice 2 landed: desktop flat now owns a small `mclone-native-client` raw-event
  adapter that converts `winit` keyboard/mouse/touch events into `mclone-input`
  capability state and `FlatInputFrame` intents before applying movement, look,
  hotbar, menu, attack, and use through existing shared camera/interaction
  paths. Cursor lock, focus, debug shortcuts, and UI pointer handling remain in
  the desktop app. Desktop `WindowEvent::Touch` now records touch capability
  state for future shared touch HUD work; it does not yet add desktop touch
  gameplay controls.
- Slice 3 Android touch sub-slice landed: flat Android now owns a
  `ClientInteractionController`, records touch capability through
  `mclone-input`, converts held touch movement into `FlatInputFrame`, and routes
  touch attack/use/hotbar selection through shared intents, block picking,
  carried-item sync, and gameplay command dispatch. `mclone-ui` can now draw the
  native touch attack/use buttons and nine-slot touch hotbar. Attached Android
  keyboard/mouse routing remains the open Slice 3 item.
