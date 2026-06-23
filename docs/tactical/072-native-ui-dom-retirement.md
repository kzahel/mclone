# 072: Native UI DOM Retirement

Status: proposed high-priority parent.

## Purpose

Move all player-facing native-web UI onto the shared native Rust/WebGPU UI path so desktop and browser cannot drift into separate menu systems again.

The target is not "host-native" OS widgets. It is the engine-native `mclone-ui` model rendered by `mclone-render::gui` into the game frame on every supported client shell: desktop now, native web/WASM now, and Android/XR later. Browser HTML/CSS/TypeScript should shrink to platform plumbing only.

## Problem

The current desktop client has a Rust UI stack:

- `native/apps/mclone-native-client/src/ui.rs` owns title, pause, options, widget hit testing, and `NativeUiAction`.
- `native/apps/mclone-native-client/src/app.rs` applies those actions to render options, frame pacing, render distance, mouse lock, and app exit.
- `mclone-render::gui::GuiRenderer` draws `mclone-ui::GuiDrawList` after the world pass.

The native-web client has a separate visible UI stack:

- `native/apps/mclone-web-client/www/app.html` owns menu, HUD, status, crosshair, and touch-control DOM.
- `native/apps/mclone-web-client/www/mclone-web-hud.ts` owns menu/settings state and the web-only look-sensitivity setting.
- `native/apps/mclone-web-client/www/mclone-web-app.ts` hardcodes render radius through `RADIUS_CHUNKS` and calls into Rust only for world/render/session operations.
- `native/apps/mclone-web-client/src/web_canvas.rs` renders sky, chunks, and actors, then presents without a GUI pass.

That split was useful for the quick mobile/HUD slice in `064`, but it is now a drift trap. Every new web button or CSS panel makes the native UI less authoritative.

## Desired End State

- `mclone-ui` is the single source of truth for player-facing screens, menus, HUD overlays, debug/status panes, and touch widgets where practical.
- `mclone-render::gui` renders the UI draw list in both desktop and web frame paths.
- Native desktop and native web route input into the same UI model before gameplay input.
- DOM remains only for:
  - one canvas
  - script/module loading
  - browser APIs: WebGPU canvas surface, pointer lock, focus, resize, keyboard/mouse/touch event capture, local storage, workers, clipboard/file/open-url adapters when explicitly needed
  - smoke/test globals
  - minimal fatal pre-WASM/pre-WebGPU failure fallback if Rust cannot render a frame
- CSS remains only for document/canvas sizing and browser behavior resets. It should not draw game UI.
- HTML should not contain game menu buttons, HUD text, status panels, crosshair elements, settings forms, or touch-control elements.
- TypeScript should not own menu labels, menu state, option semantics, HUD formatting, or gameplay-visible UI layout.
- Touch-specific affordances are allowed, but their visible controls should be native UI widgets. A browser hamburger/touch gesture may be a platform input shortcut only if it opens the native UI screen rather than rendering its own menu.

## Non-goals

- Do not touch the legacy TypeScript engine under `src/**`, `test/browser/**`, `playwright*`, or legacy tacticals.
- Do not introduce a web UI framework, retained DOM app, or `egui` for player-facing UI.
- Do not add Android/OpenXR scaffolding in this tactical.
- Do not rewrite gameplay, render streaming, workers, worldgen, lighting, or mesh logic except where a UI action needs an existing setting exposed.
- Do not chase complete vanilla GUI parity before deleting the duplicate web menu. The first target is one authoritative native UI path.

## Current Useful Pieces

Already available:

- `native/crates/mclone-ui`: GUI scale, draw list, font, button, checkbox, slider, cycle button.
- `native/crates/mclone-render/src/gui.rs`: `GuiRenderer` for solid/gradient rectangles, clipping, and alpha blend overlay rendering.
- `native/apps/mclone-native-client/src/ui.rs`: usable first screen/action model, but currently desktop-app-owned and tied to `winit::keyboard::KeyCode`.
- `native/apps/mclone-native-client/src/app.rs`: reference action application for render distance, section occlusion, fullbright, frame pacing, FPS cap, resume/start, and quit.
- `native/apps/mclone-web-client/src/web_canvas.rs`: owns the web `wgpu` surface and is the right place to add `GuiRenderer`.

## Architecture Direction

### Shared UI Model

Move the reusable UI model out of `mclone-native-client` and into `mclone-ui` or a small sibling module owned by `mclone-ui`.

Keep it platform-neutral:

```text
mclone-ui
  GuiScale
  GuiInputEvent / GuiKey / PointerButton
  GameUi
  GameScreen
  GameUiAction
  GameUiRenderState
  GameUiRuntimeToggles
  widgets...
```

`GameUiRenderState` should carry the facts needed to draw labels and options without importing app crates:

- render distance and limits
- section occlusion enabled
- fullbright/lighting mode
- frame pacing mode and FPS cap where supported
- debug pane visibility or debug overlay state
- touch settings such as look sensitivity when running on touch-capable shells

`GameUiAction` should describe intent, not perform platform work:

- start/resume/open pause/open options/back
- set render distance
- toggle section occlusion
- toggle fullbright
- cycle frame pacing/FPS cap where available
- toggle debug HUD/pane
- set touch look sensitivity
- quit/shutdown/back-to-title where supported

Desktop and web app adapters remain responsible for applying actions to their runtime/session/platform state.

### Web Frame Composition

Add the GUI pass to `WebChunkRenderSession`:

- add a `GuiRenderer` field
- add a shared `GameUi` field or accept UI draw/input through explicit exported calls
- update resize to update `GuiScale`
- draw UI after sky/chunks/actors and before `queue.submit` / `frame.present`
- if the active UI covers the world, clear with the same GUI/title background semantics desktop uses instead of drawing the world

The existing `mclone-render::target::RenderFrameTarget` path already fits the web surface.

### Browser Input Adapter

Browser event listeners should become raw input translation:

```text
browser event
  -> CSS/client coordinates
  -> GuiInputEvent
  -> WebChunkRenderSession.handleUiInput(...)
  -> GameUiAction(s)
  -> WebChunkApp applies platform/session effects
  -> if not consumed, gameplay input path
```

Rules:

- `Esc` opens pause during gameplay and closes eligible screens when UI is active.
- Pointer lock is suppressed or released while UI is active.
- Gameplay movement/look/block interaction should not run while a modal UI is active.
- Touch controls can send the same GUI input events as mouse/pointer where possible.

### DOM Burn-down

Delete visible DOM in stages. Do not replace a DOM control with another DOM control.

Allowed final `app.html` shape should be close to:

```html
<canvas id="mclone-canvas" tabindex="0"></canvas>
<script>...</script>
<script type="module" src="./mclone-web-app.js?..."></script>
```

CSS should be limited to body/canvas sizing, overflow/touch-action resets, and focus outline suppression if needed.

## Implementation Slices

### Slice 1 - Shared Menu Model Extraction

Goal: desktop keeps identical behavior, but the menu model no longer lives in the desktop app crate.

- [ ] Confirm workstream: native Rust + native web/WASM.
- [ ] Move `NativeScreen`, `OptionsParent`, `NativeUiAction`, and `NativeUi` concepts into `mclone-ui` with neutral names.
- [ ] Replace `winit::keyboard::KeyCode` in shared UI with a small `GuiKey` enum.
- [ ] Keep current draw-list output and widget IDs stable enough that native screenshots should remain visually equivalent.
- [ ] Update desktop to translate `winit` input into `mclone-ui` input/key types.
- [ ] Keep desktop `apply_ui_action` behavior in the app adapter.
- [ ] Update tests currently in `native-client/src/ui.rs` or move equivalent tests into `mclone-ui`.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-native-client
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --headless-ui /tmp/mclone-ui-title.png --width 960 --height 540
```

Inspect `/tmp/mclone-ui-title.png` before moving on.

### Slice 2 - Web GUI Renderer Integration

Goal: web can render the shared UI draw list through WebGPU even before DOM menu deletion.

- [ ] Add `mclone-ui` as a direct dependency of `mclone-web-client` if needed.
- [ ] Add `GuiRenderer` and shared UI state to `WebChunkRenderSession`.
- [ ] Initialize GUI renderer with the web surface format.
- [ ] Resize/update `GuiScale` with canvas backing size.
- [ ] Render the UI overlay after actors and before present.
- [ ] Add exported session methods for UI open/close/status if the TS adapter needs to trigger screen state.
- [ ] Add a web smoke path that captures a visible native UI menu screenshot to `/tmp`.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:build
pnpm native:web:app-smoke
```

Capture and inspect a browser screenshot with the native UI visible.

### Slice 3 - Web Menu Input And Action Application

Goal: the browser menu content is native UI; TS only forwards input and applies returned actions.

- [ ] Route browser pointer/key events to the Rust UI first when a screen is active.
- [ ] Add/adjust wasm exports for:
  - open pause/menu
  - process UI input
  - query whether UI is active/covers world
  - drain/apply `GameUiAction` results or return action reports
- [ ] Apply shared actions in `mclone-web-app.ts` / `WebChunkRenderSession`:
  - resume/start closes UI and restores gameplay input
  - render distance changes replace hardcoded `RADIUS_CHUNKS`
  - debug pane toggles native rendered debug UI
  - section occlusion/fullbright options update web render options where supported
  - shutdown/title behavior is explicit if supported, disabled if not
- [ ] Remove DOM menu button handlers for `#main-menu`, `#menu-resume`, `#menu-debug`, `#menu-settings`.
- [ ] Keep only a platform shortcut for opening native menu on touch, such as a gesture or a temporary invisible/native-rendered hamburger hit target.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:movement:smoke
```

Inspect desktop and mobile web screenshots with menu open/closed.

### Slice 4 - Native Debug HUD, Status, Crosshair, And Settings

Goal: remove the remaining visible HUD/status DOM.

- [ ] Move runtime/debug HUD formatting to Rust UI or shared presentation structs.
- [ ] Render debug pane through `mclone-ui` on both desktop and web.
- [ ] Render boot/ready/failure status through native UI once Rust is loaded.
- [ ] Render crosshair through native UI or renderer-owned screen overlay, not DOM.
- [ ] Move look sensitivity into native UI state/action; persist it via a platform storage adapter instead of direct HUD DOM ownership.
- [ ] Delete `#runtime-hud`, `#status`, `.hud`, `.status`, `.crosshair`, and associated DOM update code when replaced.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
pnpm native:web:typecheck
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-native-ui-debug.png --screenshot-debug-pane true --width 1280 --height 720
```

Inspect `/tmp/mclone-native-ui-debug.png` and web screenshots.

### Slice 5 - Touch Controls As Native UI Widgets

Goal: retire visible DOM touch controls.

- [ ] Represent touch joystick, jump, sprint, descend, and hamburger/menu affordances as `mclone-ui` widgets or a small native-rendered touch overlay.
- [ ] Keep browser pointer/touch event capture in TS, but send input facts into Rust.
- [ ] Preserve analog movement impulse behavior from `064`.
- [ ] Delete `.touch-controls`, `.touch-joystick`, `.touch-button`, and associated HTML once the native overlay is validated.
- [ ] Keep touch controls hidden on non-touch desktop unless explicitly enabled for testing.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
```

Inspect mobile screenshots and verify no page scroll/selection occurs during touch play.

### Slice 6 - HTML/CSS/TS Final Burn-down

Goal: make drift hard by removing the places where visible web UI can accumulate.

- [ ] Reduce `app.html` to canvas, scripts, and minimal non-game bootstrap fallback only.
- [ ] Remove CSS rules for menus, cards, HUD, settings, status, crosshair, and touch controls.
- [ ] Remove `mclone-web-hud.ts` if fully obsolete.
- [ ] Shrink `mclone-web-app.ts` state to runtime/test facts and platform adapters, not visible UI state.
- [ ] Add a static check or smoke assertion that `app.html` contains no visible game UI controls.
- [ ] Update `docs/gui.md`, `064`, `070`, and `071` only where they still describe the retired DOM UI as current.
- [ ] Update this doc's status/landed section, commit, and report the next high-value step.

Validation:

```bash
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:bundle
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
cargo test --manifest-path native/Cargo.toml
git diff --check
```

Verify staged/deploy output contains no `.ts` files and no obsolete visible DOM UI.

## Documentation And Commit Protocol

For every good implementation chunk:

1. Update this tactical:
   - change `Status:` if appropriate
   - check off completed items
   - add a short `## Landed` entry with files touched, behavior changed, validation commands, screenshots inspected, and known follow-ups
2. Update `docs/tactical/README.md` row status if the tactical state changed.
3. Run the relevant validation lane for the chunk.
4. Commit the code and doc update together.
5. Tell the user:
   - commit hash and title
   - what changed
   - validation results
   - the next highest-value step

Do not leave a completed chunk uncommitted unless the user explicitly asks not to commit.

## Open Decisions

- Should web keep any visible pre-WASM fatal error DOM, or should it fail blank plus console until Rust can render an error screen?
- Should render distance and web chunk radius become fully shared immediately, or should web expose only the radius currently safe for mobile until streaming cost is tuned?
- Should touch look sensitivity be a general camera option or a touch-only submenu?
- Should debug HUD formatting live entirely in `mclone-ui`, or in shared runtime presentation structs consumed by `mclone-ui`?

## Landed

Nothing yet.
