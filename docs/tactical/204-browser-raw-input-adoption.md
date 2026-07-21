# Tactical 204: Browser Raw Input Adoption

Status: active 2026-07-21. The landed native router and current browser
keyboard, pointer, touch, frame, and smoke consumers have been inventoried;
implementation starts with ownership locks and the raw Wasm boundary.

Topic: `platform-host-boundary`

Related topic:

- [`../topics/platform-host-boundary.md`](../topics/platform-host-boundary.md)

Predecessor:

- [`203-shared-interactive-router-native-adoption.md`](203-shared-interactive-router-native-adoption.md)

## Goal

Make the ordinary browser path a thin raw-input adapter over the same
`MonoInteractiveInputRouter` used by desktop and flat Android. TypeScript will
retain DOM listener, pointer-capture/lock, coordinate, synthetic-event
suppression, rAF, and Wasm-borrow mechanics, but it will stop mapping keys,
buttons, wheel, or touch regions to gameplay/UI actions and stop assembling a
semantic input frame for every render call.

This tactical changes the production input path. It does not yet perform the
full diagnostic/global-state cleanup or the preference/bootstrap cleanup;
those are the next two bounded tacticals in the series.

## Current Shape

`mclone-web-input.ts` currently maps DOM events directly to movement action
names, hotbar slots, pause/help/debug behavior, string-keyed block break/place,
and camera-speed changes. `WebFrameDriver` stores those semantic key booleans,
touch booleans, analog movement, and mouse deltas, then passes fourteen
game-shaped arguments to `WebSceneHost::renderFrame`.

`mclone-web-touch.ts` separately owns joystick geometry/dead-zone policy,
left/right gesture roles, Jump/Attack/Use/Descend buttons, menu behavior, and
the semantic touch-overlay report. This duplicates the shared
`TouchInputAdapter` and the touch hit rectangles already used by flat Android.

`WebSceneHost` reconstructs a `FlatInputFrame`, calls the scene host directly,
and exposes parallel string/per-action entry points for block interaction,
hotbar, UI keys, and UI pointers. `MonoInteractiveInputRouter` is not yet a
field of the browser host.

Browser smokes also call some of those semantic production globals directly.
This tactical must move ordinary browser operation away from them and record
every remaining smoke-only consumer. Tactical 205 will put those consumers
behind a Rust-authored diagnostic/test observer and remove the production
command registry.

## Fixed Contracts

1. `WebSceneHost` owns one `MonoInteractiveInputRouter`; browser input does not
   add a second web-only binding or scene-policy owner.
2. TypeScript forwards physical browser facts: code/key, pressed/repeat,
   browser button, pointer position/motion, wheel delta/mode, touch/pen contact
   identity/phase/position, focus, and capability facts.
3. Browser Rust normalizes DOM encodings into shared `KeyboardKey`,
   `PointerButton`, wheel, GUI point, and touch values. Native does not adopt
   strings, JavaScript objects, promises, or browser event shapes.
4. Rust owns keyboard/mouse held state. `renderFrame` receives time only for
   input purposes and advances the router's held frame plus the shared touch
   adapter's held frame.
5. Touch control identity, bindings, dead zone, sensitivity application,
   joystick state, and rendered overlay are shared Rust policy. TypeScript may
   retain pointer capture and the 800ms synthetic-mouse suppression rule.
6. The shared `mclone-ui` touch rectangles become the one hit-test model for
   flat Android and browser. Web CSS/client coordinates are converted to the
   renderer's GUI space before hit testing.
7. The browser's 4px mouse click-versus-drag rule may remain TypeScript-owned
   as neutral browser gesture hygiene. It reports a physical click/button; it
   does not select Attack, Use, break, or place.
8. UI-versus-world routing occurs behind `MonoInteractiveInputRouter`.
   TypeScript does not branch on `uiActive` to choose a semantic method.
9. Mechanical event results may report handled/prevent-default, clear local
   gesture state, pointer-capture desire, and scene/redraw change. They do not
   report which game action occurred.
10. Browser runtime/debug shortcuts may be classified separately from normal
    bindings, but their physical mapping and execution must move out of
    TypeScript. Tactical 205 may further isolate them with diagnostics.
11. The rAF loop, canvas sizing, visibility observation, Worker/storage/socket
    adapters, catalog/session operations, and presentation remain unchanged
    except where their call signature must stop carrying semantic input.
12. Existing direct semantic smoke commands are not evidence of a valid
    production adapter. Any temporary survivor must be named and have no
    ordinary input caller, then be removed or isolated in Tactical 205.

## Implementation Slices

### Slice 0: Locks and browser behavior inventory

- Add a source lock for the current TypeScript action table, semantic held
  state, fourteen-argument frame call, string interaction ABI, direct UI entry
  points, and touch action/layout table.
- Pin behaviors to preserve or deliberately converge: UI input suppression,
  4px click/drag threshold, 800ms synthetic-mouse suppression, pointer-lock
  reacquisition, pen-as-touch, wheel delta-mode handling, touch held versus
  one-shot controls, and focus/visibility clearing.
- Classify current smoke-only direct commands separately from production input
  so the later observer tactical has an exact deletion ledger.

### Slice 1: Shared touch hit testing

- Move the Android-local `touch_control_at` selection over existing shared UI
  rectangles into a shared Rust owner.
- Keep `TouchInputAdapter` as the binding/held/dead-zone owner and use its
  overlay state for both platforms.
- Adopt the shared hit-test helper in flat Android without changing its event
  loop or rendered behavior.

### Slice 2: Raw WebSceneHost input boundary

- Add `MonoInteractiveInputRouter`, `TouchInputAdapter`, and any minimal
  browser touch-contact bookkeeping to `WebSceneHost`.
- Add small raw key, pointer button/click, pointer move/motion, wheel, touch,
  focus/clear entry points returning mechanical dispositions.
- Normalize browser code/key/button/phase labels in browser Rust and reject
  malformed values without assigning meaning in TypeScript.
- Route touch events and overlay updates through the shared touch adapter and
  shared touch hit test.
- Shrink `renderFrame` to time/presentation facts and advance Rust-held input.
- Update overview/warm-up/test callers to the new frame signature.

### Slice 3: Keyboard and mouse TypeScript cutover

- Replace action-name tables and setters with raw DOM event forwarding.
- Remove TypeScript hotbar, menu/help, movement-mode, debug-toggle,
  break/place, and wheel-policy branches from ordinary listeners.
- Retain pointer lock/capture, click/drag recognition, coordinate conversion,
  context-menu suppression, listener options, and generic handled/
  prevent-default behavior.
- Queue only raw browser facts if an exported mutable Wasm borrow is busy.
- Remove semantic input state and per-frame assembly from `WebFrameDriver`.

### Slice 4: Touch TypeScript cutover

- Replace TypeScript joystick/button/layout/action state with raw touch/pen
  contact forwarding and mechanical pointer capture.
- Preserve synthetic mouse suppression and canvas focus.
- Have browser Rust/shared UI produce the touch overlay and shared input state.
- Remove Jump/Attack/Use/Descend, movement-key, menu-action, and string
  interaction vocabulary from `mclone-web-touch.ts`.

### Slice 5: Smoke migration and closeout

- Make browser input smokes use real DOM keyboard, mouse, wheel, and pointer
  events wherever the physical adapter is the subject under test.
- Record any remaining direct semantic smoke command as Tactical 205 debt;
  ordinary app input must not call it.
- Run shared input/scene/web Rust tests, source locks, Wasm/generated-bindgen
  build, TypeScript typecheck, Worker ownership, and scene-host adoption gates.
- Run headed Wayland desktop and mobile browser input/UI smokes and inspect
  their screenshots.
- Run native scene/router, desktop compile, Android compile/source-lock, and XR
  controls affected by shared touch/UI changes.
- Update the master topic with the landed browser contract and open Tactical
  205 from the actual remaining diagnostics surface.

## Acceptance Criteria

1. Ordinary desktop, flat Android, and browser flat input use one
   `MonoInteractiveInputRouter` implementation.
2. `mclone-web-input.ts` contains no physical-key-to-game-action table and no
   hotbar/menu/help/debug/gameplay selection.
3. `mclone-web-touch.ts` contains no game-action names, button-role layout,
   joystick/dead-zone policy, or semantic overlay construction.
4. Production TypeScript stores no held gameplay keys or analog movement
   frame and passes no such values to `renderFrame`.
5. Browser Rust parses raw browser encodings but does not reimplement the
   shared binding/context/action switch.
6. Mouse buttons, wheel, UI keys/pointers, and touch one-shot/held behavior pass
   through the shared binding/router path.
7. Browser and flat Android use one shared touch hit-test/layout owner and one
   touch binding/dead-zone implementation.
8. DOM pointer/capture/lock, event suppression, rAF, canvas, and Wasm-borrow
   mechanics remain platform-owned.
9. Real DOM integration tests prove browser normalization and mechanics;
   shared Rust tests prove action meaning.
10. The source lock rejects reintroduction of TypeScript action maps,
    semantic frame arguments, or ordinary calls to direct action exports.
11. Rendered headed desktop and mobile browser captures are inspected.
12. Workers, persistence, catalog, sockets, startup/session coordination, and
    native typed hot paths retain their existing ownership.

## Expected Follow-Up

After this tactical closes, create Tactical 205 for the Rust-authored
diagnostic/test observer and removal of the production semantic report mirror
and smoke command registry. Then create the preference/bootstrap tactical from
the smaller production web host that remains. Do not combine those concerns
into this input cutover merely because they share `mclone-web-app.ts`.

## Stop Conditions

Stop for renewed review if:

- browser event handling requires TypeScript to know a game/UI action rather
  than forwarding a physical fact or executing a mechanical disposition;
- the raw ABI would require native to serialize input or adopt browser async
  machinery;
- the router would duplicate rather than call existing scene/client policy;
- shared touch hit testing cannot represent both Android and browser without
  encoding DOM/device-specific mechanics in `mclone-ui` or `mclone-input`;
- preserving an undocumented browser behavior conflicts materially with the
  accepted shared bindings and needs a product choice; or
- the cutover would weaken Worker, persistence, socket, catalog, or session
  actor ownership.

Routine bindgen regeneration, TypeScript call-site changes, smoke migration,
and headed-browser setup are not stop conditions.

## Non-Goals

- removing the broad production report/state mirror in this tactical;
- designing the final diagnostic observer or smoke build surface;
- moving preference schema/default/clamping or bootstrap/session selection;
- moving DOM listeners wholesale into `web_sys`;
- creating a universal platform event enum or adapter trait;
- changing IndexedDB, Worker, WebSocket, catalog, or persistence contracts;
- changing XR controller/pose input; or
- adding end-user rebinding UI.

## Code Map

Shared route and touch model:

- `native/crates/mclone-scene/src/interactive_input.rs`
- `native/crates/mclone-input/src/lib.rs`
- `native/crates/mclone-ui/src/lib.rs`

Browser Rust and TypeScript:

- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-web-input.ts`
- `native/apps/mclone-web-client/www/mclone-web-touch.ts`

Validation and later diagnostic consumers:

- `native/apps/mclone-web-client/tests/`
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`
- `native/apps/mclone-web-client/www/mclone-web-smoke.js`
- `scripts/check-web-worker-ownership.mjs`
- `scripts/check-thin-platform-adapters.mjs`
