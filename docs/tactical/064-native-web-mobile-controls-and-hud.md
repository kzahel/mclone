# 064: Native Web Mobile Controls and HUD

Status: active; mobile controls/HUD and analog movement slices landed.

## Purpose

Make the native web/WASM app usable on phones and tablets without reviving the
legacy TypeScript engine.

The immediate user-facing problems are:

- the native web app has keyboard/mouse input, but no mobile movement controls
- the runtime/debug HUD is always visible and takes too much screen space on
  small displays

This tactical ports the useful interaction shape from the legacy TypeScript
browser implementation into the native web adapter, while keeping runtime,
movement, renderer, and server ownership in the native Rust workspace.

## Reference Prior Art

Use the legacy TypeScript implementation only as reference prior art:

- `src/renderer/debug/debug-input.ts`
  - fixed-center virtual joystick
  - right-side touch drag for look
  - touch event capture and gesture ownership
  - `JOYSTICK_MAX_DISTANCE = 50`, `JOYSTICK_DEAD_ZONE = 10`
- `src/renderer/debug/touch-control-layout.ts`
  - responsive forward/back button sizing and lower-right placement
- `src/renderer/gui/touch-joystick-hud.ts`
  - translucent joystick base/thumb and active button visualization

Do not edit `src/**/*.ts` for this tactical. The implementation target is the
native web/WASM app under `native/apps/mclone-web-client/`.

## Starting Native Web State

At the start of this tactical, the browser app already had:

- continuous `requestAnimationFrame` ticking in
  `native/apps/mclone-web-client/www/mclone-web-app.js`
- keyboard and mouse-look input feeding `WebChunkRenderSession.advanceCameraFrame`
- physical-key browser input for layout-independent desktop movement
- walking/collision mode, no-clip mode, hotbar selection, target preview, and
  block interaction
- Playwright app-loop smoke coverage and screenshot capture

The initial gaps were:

- `mclone-web-app.js` stores movement as booleans only:
  `forward`, `backward`, `left`, `right`, `jump`, `descend`, `shift`, `sprint`
- `web_canvas.rs::advance_camera_frame(...)` accepts those booleans directly
- `mclone-render-session::EngineCameraInput` also accepts only booleans, even
  though `mclone-client::PlayerInput` internally has impulse fields that could
  support analog axes
- `app.html` renders the HUD and bottom status bar unconditionally
- there is no phone-sized validation path for touch movement or HUD behavior

## Target UX

On touch devices:

- the first touch on the left half of the canvas creates a floating movement
  joystick anchored at the touch start
- dragging the movement joystick produces forward/back/strafe intent
- the first touch on the right half of the canvas controls look by accumulating
  mouse-look style deltas
- touch controls suppress page scroll/zoom while interacting with the canvas
- jump, descend/sneak, and sprint are available as large thumb-friendly buttons
- the control overlay is visible only on touch-capable devices or after the
  first touch interaction
- desktop keyboard/mouse behavior remains unchanged

For the HUD:

- the detailed runtime stats live behind a compact hamburger/debug button
- the HUD is collapsed by default on mobile
- a minimal status signal remains visible for boot/failure state
- errors should either auto-open the panel or show an obvious failure badge
- the panel must not block joystick/look areas when collapsed

## Architecture

Keep the first mobile input slice in the browser adapter:

```text
DOM/touch events
  -> mclone-web-app.js TouchControls
  -> WebChunkApp frame input
  -> WebChunkRenderSession.advanceCameraFrame(...)
  -> EngineCameraController / LocalPlayerController
```

The web adapter may own DOM gesture capture, CSS overlay controls, and
mobile-specific layout. It must not move gameplay movement rules into
JavaScript. JavaScript should only produce input intent.

Longer term, analog joystick axes should be represented in the shared native
engine input type:

```text
EngineCameraInput
  dt_seconds
  mouse_delta_x / mouse_delta_y
  left_impulse / forward_impulse
  jump / descend / shift / sprint
  compatibility key booleans or a key-to-axis helper
```

That keeps desktop keyboard, native web touch, and future gamepad input on the
same renderer/session boundary.

## Implementation Slices

### 1. Collapsible Native Web HUD

Status: landed first pass.

Scope:

1. Add a compact top-left button in `app.html`.
2. Move the existing `.hud` into a collapsible panel.
3. Collapse by default on small viewports and touch-capable devices.
4. Keep the bottom status area minimal or fold it into the same panel, while
   preserving visible boot/error feedback.
5. Add keyboard/mouse accessibility basics:
   - real `button`
   - `aria-expanded`
   - visible focus state
   - Escape closes the panel

Acceptance:

- desktop view still allows inspecting runtime stats
- mobile view starts with the stats hidden
- failure state is still visible without hunting through hidden UI
- app-loop smoke still finds `#status` or an equivalent status element

### 2. Digital Virtual Joystick

Status: landed first pass.

Scope:

1. Add a `TouchControls` helper in `mclone-web-app.js`.
2. Track active touch/pointer IDs separately for:
   - movement joystick
   - look drag
   - action buttons
3. Reuse the legacy constants as starting values:
   - max throw: `50px`
   - dead zone: `10px`
4. Convert joystick axes to the existing boolean keys:
   - up -> `forward`
   - down -> `backward`
   - left -> `left`
   - right -> `right`
5. Convert right-side drag into `queueMouseDelta(...)`.
6. Add CSS-rendered joystick base/thumb and large action buttons.
7. Clear touch input on `touchcancel`, lost pointer capture, blur, and when the
   HUD/menu is actively consuming input.

Acceptance:

- phone viewport can walk with the left thumb and look with the right thumb
- keyboard/mouse path remains unchanged
- movement stops immediately when the controlling touch ends or is cancelled
- no page scrolling/selection occurs during canvas touch controls

### 3. Touch Buttons

Status: landed first pass for jump, descend, and sprint hold buttons.

Scope:

1. Add thumb buttons for:
   - jump
   - descend/sneak
   - sprint hold or sprint toggle
2. Decide sprint behavior after a real-phone feel check:
   - hold is simpler and mirrors keyboard state
   - toggle is easier on small screens but needs clear visual state
3. Keep break/place out of this slice unless needed for immediate phone testing;
   those need more careful accidental-tap behavior.

Acceptance:

- walking mode can jump
- no-clip mode can move vertically
- sprint state is visible if it toggles

### 4. Analog Movement Input

Status: landed.

Scope:

1. Extend `mclone-render-session::EngineCameraInput` with movement impulses.
2. Keep key booleans or add a helper that turns booleans into impulses for
   desktop compatibility.
3. Add a `PlayerInput` path that accepts explicit `left_impulse` and
   `forward_impulse` without losing Java-shaped key state where it matters
   (`jump`, `shift`, sprint gating).
4. Extend `web_canvas.rs::advance_camera_frame(...)` or add a new bindgen
   method that accepts axes.
5. Feed raw joystick axes from JS instead of thresholded booleans.

Acceptance:

- diagonal joystick movement has smooth analog magnitude
- walking acceleration and no-clip movement still normalize/limit correctly
- existing native movement tests remain green
- desktop keyboard input produces the same behavior as before

### 5. Mobile Smoke and Visual Validation

Status: landed first pass.

Scope:

1. Add a Playwright mobile app-loop lane or an option on
   `native/apps/mclone-web-client/scripts/browser-smoke.mjs`.
2. Use a phone-sized viewport.
3. Dispatch synthetic touch/pointer gestures:
   - left joystick drag moves camera/chunk view
   - right drag changes yaw/pitch
   - hamburger opens/closes stats
4. Save mobile screenshots under `/tmp`, for example:
   - `/tmp/mclone-native-web-mobile-app.png`
   - `/tmp/mclone-native-web-mobile-canvas.png`
5. Run a real device check when practical, because browser touch synthesis does
   not catch every mobile Safari/Chrome viewport and pointer-capture quirk.

Acceptance:

- automated smoke proves touch movement and look update state
- screenshots show terrain, crosshair, collapsed HUD, and usable touch controls
- no screenshot artifacts are written into the repo

## Non-Goals

- Do not revive or edit the legacy TypeScript browser engine.
- Do not implement a full mobile UI or inventory screen.
- Do not add gamepad support in this tactical, though the analog input shape
  should make that easier later.
- Do not move DOM/touch handling into shared Rust crates.
- Do not block the first usable mobile slice on analog input; digital joystick
  is acceptable as the bring-up step.
- Do not add Android, Gradle, or OpenXR scaffolding.

## Validation

Run after each relevant slice:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
```

For rendered-output validation, inspect the saved `/tmp` screenshots before
considering a slice complete.

Run broader native checks if the analog slice touches shared Rust movement or
render-session code:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
pnpm native:movement:smoke
pnpm native:web:app-smoke
```

## Open Questions

- Should sprint be a hold button, a toggle, or both behind a small mode switch?
- Should the left joystick be floating at touch start, fixed at bottom-left, or
  configurable after a real phone test?
- Should break/place become mobile buttons in this tactical or wait until basic
  movement/look is proven?
- Does the current dead-zone and response curve feel right on actual mobile
  Safari/Chrome, or should the curve become configurable?

## First Slice Landed - 2026-06-22

Implemented the first usable native web mobile-control pass:

- `app.html`
  - debug/runtime HUD is behind a hamburger-style button
  - HUD defaults collapsed on touch/coarse-pointer/mobile viewports
  - ready status is hidden, while boot/error status remains available
  - full-height mobile viewport sizing keeps the WebGPU canvas under controls
  - touch overlay contains CSS-rendered jump, sprint, and descend buttons
- `mclone-web-app.js`
  - keyboard and touch inputs are tracked separately and ORed per frame
  - left-side touch creates a floating digital movement joystick
  - right-side touch drag feeds the existing mouse-look delta path
  - jump/descend/sprint touch buttons feed existing input booleans
  - touch-generated compatibility mouse events are ignored to avoid accidental
    pointer-lock attempts
  - runtime state exposes HUD/touch diagnostics for smoke validation
- `browser-smoke.mjs`
  - added `--mobile-app-loop`
  - mobile smoke uses a phone viewport with touch enabled
  - probes joystick walking, touch-look yaw/pitch change, touch button state,
    and HUD open/close behavior
  - saves `/tmp/mclone-native-web-mobile-app.png` and
    `/tmp/mclone-native-web-mobile-app-canvas.png`
- `package.json`
  - added `pnpm native:web:mobile-smoke`

Validated with:

```text
node --check native/apps/mclone-web-client/www/mclone-web-app.js
node --check native/apps/mclone-web-client/scripts/browser-smoke.mjs
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
```

Screenshots inspected:

- `/tmp/mclone-native-web-app-canvas.png`
- `/tmp/mclone-native-web-mobile-app.png`
- `/tmp/mclone-native-web-mobile-app-canvas.png`

## Analog Slice Landed - 2026-06-22

Implemented smooth joystick axes through the native movement boundary:

- `mclone-client::PlayerInput`
  - added an explicit movement-impulse override path
  - keyboard-derived impulses remain the default path
  - jump, shift, descend, and sprint gating continue to use key state
- `mclone-render-session::EngineCameraInput`
  - added optional `EngineCameraMovementImpulse`
  - walking and no-clip movement both consume the same impulse override
- `web_canvas.rs`
  - extended `advanceCameraFrame(...)` with active/left/forward impulse args
- `mclone-web-app.js`
  - touch joystick now feeds dead-zone-adjusted analog axes
  - thresholded touch-key booleans remain available for diagnostics and
    compatibility state
- `browser-smoke.mjs`
  - mobile smoke now asserts fractional diagonal joystick axes while a touch is
    active and verifies axes clear after release

Next likely step: real-phone feel validation and interaction tuning. In
particular, check whether sprint should remain hold-only, whether the joystick
response curve/dead zone feels right, and whether break/place need explicit
mobile buttons before more mobile UI work.
