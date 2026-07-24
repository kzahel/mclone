# Touchscreen Input

Topic: `touchscreen-input`

Status: active implementation as of 2026-07-24. The shared touch gameplay
adapter, touch HUD, and Android/browser consumers already exist. Native desktop
currently observes touch capability but discards every
`winit::WindowEvent::Touch` before it reaches menu or gameplay semantics. The
first physical Steam Deck report is that tapping Quit on the title screen does
not activate it. Automated host validation and rendered-output acceptance will
land in this series; physical Deck acceptance remains pending afterward.

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

## Implementation And Validation Plan

1. Route first-contact native desktop touch through shared UI pointer
   down/move/up semantics, including non-activating cancellation and lifecycle
   clearing.
2. Add the shared touch adapter to the desktop host, compose held movement,
   route look and one-shot frames, and render the existing touch overlay.
3. Expose mode and sensitivity settings on capable desktop hosts and persist
   changes through the shared input-preference codec.
4. Add focused tests for capability discovery, SteamOS startup projection,
   contact ownership/cancellation, visibility policy, coordinate conversion,
   preference preservation, and source switching.
5. Run native-client, input, UI, scene, and app-runtime tests plus the affected
   workspace/platform boundary gates.
6. Capture and inspect the first desktop frame that renders the touch overlay.
7. Record physical Gaming Mode acceptance on the Steam Deck:
   - every title/menu button activates exactly once;
   - press-drag-off does not activate;
   - a second contact cannot steal the active UI press;
   - controller, trackpad/mouse, and touch switch cleanly in `Auto`;
   - in-world move/look/jump/attack/use/hotbar/pause controls work;
   - `Off` hides and disables gameplay controls but menus still work; and
   - focus loss and suspend/resume leave no stuck contact or action.

## Current Gaps

- Native desktop implementation and automated validation are in progress.
- Direct touch-drag scrolling and touch-specific larger menu geometry have not
  yet been accepted as requirements; physical Deck use should determine
  whether they need a follow-up.
- Published-App-ID behavior under both Steam touchscreen configuration modes
  remains future distribution acceptance.
- Physical Steam Deck evidence is pending the user's manual pass after this
  implementation series lands.
