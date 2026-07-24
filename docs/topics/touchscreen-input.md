# Touchscreen Input

Topic: `touchscreen-input`

Status: automated implementation and host validation complete as of
2026-07-24; physical Steam Deck acceptance remains pending. The first Deck
report was that tapping Quit on the title screen did not activate it. Native
desktop now routes standard `winit` touch events through the shared menu
contact lifecycle and the existing `mclone-input` gameplay adapter, exposes
and persists touch settings, and renders the shared touch HUD. Android and
browser hosts use the same contact and `Auto`/`On`/`Off` policy.

## Scope

This topic owns direct touchscreen behavior across native desktop, Steam Deck,
flat Android, and browser clients:

- direct menu button, slider, and other pointer-like activation;
- multi-contact lifecycle and cancellation;
- in-world movement, look, interaction, hotbar, and pause controls;
- touch-control visibility, settings, prompts, and input-source arbitration;
- platform collection differences that affect those shared semantics; and
- physical touchscreen acceptance.

Ordinary controller and future native Steam Input behavior remain owned by
[`controller-input.md`](controller-input.md). Loss-aware source observations
and the fixed movement-command timeline remain owned by
[`input-observation-timeline.md`](input-observation-timeline.md). Steam Deck
deployment and general handheld acceptance remain owned by
[`steam-deck-test-bed.md`](steam-deck-test-bed.md).

## Reported Defect And Root Cause

On the Steam Deck title screen, tapping Quit does not exit. This is not a Quit
action or shared UI hit-test defect:

- native desktop mouse input sends primary down/up through
  `MonoInteractiveInputRouter`;
- the shared UI captures the pressed widget and activates it only when release
  occurs over that same widget; and
- the title layout binds Quit to `GameUiAction::Quit`.

The native desktop `WindowEvent::Touch` branch only calls
`note_touch_activity()` and schedules a redraw. It does not route the contact
position or phase. The UI therefore never receives the down/up pair required
to activate any title button.

The current Steam Deck presentation logs show a Gamescope X11/Xwayland window.
This matters because winit 0.30.13's X11 backend deliberately suppresses
`XIPointerEmulated` mouse-button events when it delivers native multitouch
events, avoiding duplicate mouse and touch input. It still emits cursor
movement for the first active contact. The resulting symptom can therefore be
a moving cursor or hover highlight without a click.

Valve documents mouse-click emulation as the default Steam Deck touchscreen
mode and offers a Steamworks **Touch API Pass-through** setting for native
multitouch. Application correctness must not depend on that configuration:
Devkit shortcuts, published Steam App IDs, X11, Wayland, Windows, browsers, and
non-Steam touchscreen PCs may expose different event projections.

References:

- [Steam Deck FAQ: input and touchscreen modes](https://partner.steamgames.com/doc/steamhardware/steamdeck/faq?l=english#input)
- [winit `WindowEvent` touch contract](https://docs.rs/winit/0.30.13/winit/event/enum.WindowEvent.html)
- [winit 0.30.13 X11 event processing](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/x11/event_processor.rs)

## Accepted Product Contract

### Menu touch is unconditional

When a shared flat menu is active, the first eligible contact behaves as a
direct primary pointer:

- `Started` sends pointer down at that contact's position;
- `Moved` updates hover, slider drag, and captured-widget state;
- `Ended` sends pointer up at the final position;
- `Cancelled` releases capture without activating a widget; and
- unrelated simultaneous contacts do not steal or duplicate the UI click.

Touch-control visibility preferences apply to the in-world overlay, not to
direct menu tapping. `Touch Controls: Off` must never make menus inaccessible.
Direct touch-drag scrolling is separate future UI behavior; existing buttons,
sliders, controller focus scrolling, and mouse/trackpad wheel scrolling keep
their current contracts.

### Gameplay touch reuses the shared adapter

Native desktop must use `mclone_input::TouchInputAdapter`, the same host-neutral
movement/look/action reducer used by flat Android and browser Rust. Platform
code converts native physical coordinates into GUI space, identifies the
shared `TouchControl`, forwards contact lifecycle, and executes host effects.
It must not duplicate touch bindings or gameplay meanings in the desktop app.

Held touch movement is composed beside keyboard/mouse and controller state at
the scene-owned movement boundary. Touch look remains a pointer-like delta.
Focus loss, cancellation, quit-to-title, and other transient-input clears must
drop all active contacts and UI capture.

### Visibility and settings are capability-led

The shared `Auto`, `On`, and `Off` modes retain their existing meanings:

- `Auto` shows in-world controls when touch was the last active flat source;
- `On` keeps the overlay available on a touch-capable host; and
- `Off` disables and hides in-world touch controls while preserving menus.

Native desktop discovers touch dynamically on its first event. The explicit
SteamOS launch profile may advertise touch at startup so a Deck exposes the
Touch Controls and Touch Look settings before the first tap. Other desktop
hosts must not be classified by operating-system name or assume that every
machine has a touchscreen.

The versioned shared input-preference document already owns touch mode and look
sensitivity. Desktop setting changes must persist through the same native
storage path as Android and must preserve all controller preference fields.

### Avoid duplicate emulation

Touch events are authoritative when the platform provides them. Do not
synthesize a second click from nearby mouse events inside shared UI code.
Platform validation must cover stacks that emit native touch, mouse emulation,
or both. Any deduplication required by a backend belongs in its collector or
native event adapter, not in gameplay or widgets.

## Steam API Boundary

No Steamworks API is required for direct touchscreen support. Winit's standard
OS touch events are sufficient.

Steam Input is a documented, proprietary Steamworks partner API rather than a
private or reverse-engineered interface. It remains useful later for action
sets, trackpads, gyro, rear buttons, user rebinding, and exact action-origin
glyphs. That optional backend should emit the existing shared semantic action
frame and de-duplicate its virtual gamepad. It is not the owner of touchscreen
menu or gameplay behavior.

## Implemented Shape

- `mclone-input` owns first-contact UI tracking and cancellation. Desktop,
  Android, and browser adapters translate their platform phase enum into that
  neutral contract.
- `MonoInteractiveInputRouter` has a touch-primary-button route that cannot
  request mouse capture. Ordinary mouse clicks retain their existing capture
  behavior.
- Native desktop translates touch coordinates into shared GUI space, routes
  menu contacts unconditionally, and sends in-world contacts through
  `TouchInputAdapter`. Held movement composes with keyboard/mouse and gamepad
  state at the existing scene boundary; touch look and action frames use the
  existing shared routes.
- The SteamOS presentation profile advertises touch at startup. Generic
  desktop profiles discover it only after a real event. This makes touch
  settings visible on Deck before the first tap without classifying every
  Linux or Windows desktop as touch-capable.
- Native desktop renders the existing `TouchOverlay`, reads mode and
  sensitivity from the versioned native preference document, and writes only
  those touch fields while preserving the controller profile.
- Every flat host now admits gameplay touch only while shared capability
  resolution says controls are visible. Direct menu touch remains available
  in `Off`. In `Auto`, a later keyboard, mouse, or controller input hides and
  stops the touch HUD until touch becomes active again.

## Automated Evidence

The series is commits `cdcc649f`, `1ecb78c0`, `048ad9db`, `7b83b076`, and
`3015c017` under `Topic: touchscreen-input`.

Validation completed on 2026-07-24:

- `mclone-input`, `mclone-ui`, `mclone-app-runtime`, `mclone-scene`, and
  `mclone-native-client` tests passed, including contact ownership,
  non-activating cancellation, SteamOS startup capability, coordinate mapping,
  and controller-preference preservation.
- Native desktop compiled on the host and `mclone-web-client` compiled for
  `wasm32-unknown-unknown`.
- `pnpm native:android:apk:avd` built ARM64 and x86_64 libraries plus the APK.
  `pnpm native:android:avd-touch-smoke -- --skip-build` then installed it,
  injected a touch swipe, and passed.
- After `pnpm host:check` selected headed Wayland,
  `pnpm native:web:mobile-smoke` passed movement, look, jump, attack, use,
  touch-opened pause UI, touch-look slider persistence, and the `Auto` switch
  to keyboard after Escape.
- The Android touch frame and headed-browser world, active joystick, pause
  menu, and touch-options captures under `/tmp` were inspected. They show the
  shared touch controls and touch settings over live rendered terrain.

The current Linux host has no desktop touchscreen, so it cannot produce a real
`winit` desktop-touch capture. The two other consumers validate the same touch
adapter, UI renderer, and scene routes; the Deck pass below remains the
authoritative validation of the desktop platform collector and Gamescope event
projection.

## Physical Steam Deck Acceptance

Record a Gaming Mode pass on the Steam Deck:

- every title/menu button activates exactly once;
- press-drag-off does not activate;
- a second contact cannot steal the active UI press;
- controller, trackpad/mouse, and touch switch cleanly in `Auto`;
- in-world move/look/jump/attack/use/hotbar/pause controls work;
- `Off` hides and disables gameplay controls but menus still work; and
- focus loss and suspend/resume leave no stuck contact or action.

## Current Gaps

- Direct touch-drag scrolling and touch-specific larger menu geometry have not
  yet been accepted as requirements; physical Deck use should determine
  whether they need a follow-up.
- Published-App-ID behavior under both Steam touchscreen configuration modes
  remains future distribution acceptance.
- Physical Steam Deck evidence, including the original title-screen Quit
  reproduction, is pending the user's manual pass.
