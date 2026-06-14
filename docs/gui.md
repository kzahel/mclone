# 2D GUI Architecture

This document defines the in-game and shell UI direction for `mclone`.

The decision is:

**All visible game UI renders through WebGPU. The DOM is only a platform shell for the canvas and browser input events.**

This applies to:

- title / start world screens
- loading and connection status
- pause menus
- options and debug settings
- HUD and gameplay overlays
- touch controls and mobile affordances
- future inventory / chat / social / world-selection surfaces

Older DOM start/debug/loading/touch surfaces were transitional. Keep replacing any remaining visible browser UI with GPU screens instead of preserving it as an architecture.

## Vanilla Reference Shape

Minecraft Java 1.17.1 does not use native OS widgets or a browser-like retained DOM. Its GUI is game-engine UI rendered by the client renderer.

Primary reference files:

| Vanilla source | Role |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/Screen.java` | screen lifecycle, child widget lists, focus, narration, tooltip rendering, background rendering |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/GuiComponent.java` | filled rectangles, gradients, textured blits, string drawing |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Widget.java` | renderable widget interface |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/events/GuiEventListener.java` | pointer/key event surface |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractWidget.java` | rectangle hit testing, visible/active/focused/hovered state, shared widget rendering |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractButton.java` | press handling |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Button.java` | text button and tooltip hook |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractSliderButton.java` | slider value, mouse drag, keyboard increments |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Checkbox.java` | checkbox toggle |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/CycleButton.java` | option cycling |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/EditBox.java` | text input |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractSelectionList.java` | scrollable lists |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/Font.java` | text layout and glyph rendering |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/LoadingOverlay.java` | resource/loading overlay behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/LevelLoadingScreen.java` | world loading status screen |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/TitleScreen.java` | main menu screen |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/PauseScreen.java` | in-game pause menu |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/OptionsScreen.java` | options root |
| `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/VideoSettingsScreen.java` | video/options screen pattern |

Default implementation posture:

- Port GUI classes by reading their vanilla source first.
- Keep field names, method names, and logic flow where the browser/WebGPU platform does not force a divergence.
- Treat rendering backend calls as the sanctioned divergence: `RenderSystem`, `Tesselator`, `BufferBuilder`, and `BufferUploader` become WebGPU draw-list generation, buffers, pipelines, bind groups, and render passes.
- When a method diverges for WebGPU, add the normal one-line callout comment, for example:

```ts
// WebGPU: draw-list command instead of immediate BufferUploader submission
```

## Goals

- Replace all visible DOM UI with a WebGPU 2D GUI layer.
- Keep the model vanilla-shaped: screens, widgets, focus, hover, active/visible flags, button press callbacks, sliders, cycle buttons, edit boxes, lists, tooltips, and textured blits.
- Keep UI orchestration on the presentation thread. It may consume `ClientRuntime` presentation state and send commands, but it must not own authoritative world state.
- Keep loading status visible and honest. Existing `LoadingProgress` facts should feed WebGPU-rendered loading screens instead of DOM text.
- Make title/debug/menu flows feel Minecraft-shaped rather than web-app-shaped.
- Keep browser tests able to drive the app without relying on visible DOM widgets.

## Non-Goals

- Do not use DOM elements for visible in-game UI, menus, options, debug overlays, HUD, or touch controls.
- Do not adopt a broad retained web-app UI framework.
- Do not pull in a large immediate-mode GUI library as the default path. `egui` works well in Rust projects, but this repo already has a vanilla GUI reference to translate and the native rewrite should preserve a Minecraft-shaped GUI architecture rather than defaulting to host-native widgets.
- Do not keep old DOM shell layout shapes for compatibility. Only keep useful behavior.
- Do not implement every vanilla GUI screen before replacing the current UI. Start with the minimum real flow, then expand.

## Platform Boundary

The DOM remains allowed for platform plumbing:

- one visible `<canvas>`
- script loading
- browser keyboard, mouse, wheel, touch, pointer-lock, focus, resize, clipboard, and storage APIs
- test-only global hooks such as `window.__mcloneReady` and `window.__mcloneDebug`
- file picker or URL-opening adapter later, if a flow needs browser capability

The DOM is not allowed for visible UI:

- no buttons
- no forms
- no select boxes
- no text status overlays
- no debug panels
- no touch joystick elements
- no CSS-drawn menus, backgrounds, or controls

`index.html` and any debug shell should eventually be visually blank except for the canvas. CSS should only reset page sizing and make the canvas fill the viewport.

## Proposed Source Layout

Use a split between GUI model and WebGPU renderer:

```text
src/client/gui/
  gui-component.ts
  font.ts
  gui-input.ts
  screen-manager.ts
  screens/
    screen.ts
    title-screen.ts
    pause-screen.ts
    options-screen.ts
    debug-settings-screen.ts
    loading-screen.ts
  components/
    widget.ts
    gui-event-listener.ts
    abstract-widget.ts
    abstract-button.ts
    button.ts
    abstract-slider-button.ts
    checkbox.ts
    cycle-button.ts
    edit-box.ts
    selection-list.ts

src/renderer/gui/
  gui-renderer.ts
  gui-draw-list.ts
  gui-texture-atlas.ts
  bitmap-font-renderer.ts
  gui-pipeline.ts
```

`src/client/gui` should be mostly renderer-agnostic. It knows about GUI-scaled pixels, input events, screen lifecycle, and draw-list APIs, but not raw `GPUDevice`.

`src/renderer/gui` owns WebGPU resources and turns draw-list commands into GPU work.

The exact paths can move if the codebase converges on a different presentation namespace, but keep this ownership split.

## Screen Model

Use a vanilla-shaped screen stack with one active screen plus optional overlays where needed.

Minimum model:

```text
ScreenManager
  currentScreen: Screen | null
  overlay: Overlay | null
  setScreen(screen | null)
  tick()
  render()
  route input
```

`Screen` responsibilities:

- owns `children`, `renderables`, and `narratables` equivalents
- has `width`, `height`, `title`, and `font`
- exposes `init`, `tick`, `removed`, `resize`, `render`, `renderBackground`, `onClose`
- routes mouse, keyboard, char, wheel, focus, and drag events through children
- owns tooltip rendering and focus traversal

Widgets should follow vanilla's small-object style. A button knows its rectangle and active/visible/focused/hovered state. A screen owns where it is placed and what callback it triggers.

## Rendering Model

The GUI render pass runs after the world pass and before final presentation.

Initial draw primitives:

- solid color rect
- vertical gradient rect
- textured rect / blit
- nine-slice or vanilla two-half widget blit for buttons
- scissor / clip rect for lists and text fields
- text glyph quads
- optional debug outline primitive for development probes

Coordinate system:

- GUI code works in scaled GUI pixels, like vanilla's GUI scale model.
- The renderer converts GUI pixels to normalized device coordinates with an orthographic transform.
- Device-pixel ratio and canvas backing size are renderer concerns.
- Layout must be stable across resize. Widget rectangles are explicit integers, not DOM flow.

Batching:

- Accumulate a `GuiDrawList` each frame.
- Sort or partition by texture, pipeline state, and clip rect.
- Keep the first implementation simple: correctness and visual inspection before clever batching.
- Later, coalesce rects/text by texture atlas and clip region.

WebGPU pipelines:

- `position_color` equivalent for filled/gradient rects.
- `position_tex_color` equivalent for textured blits and glyphs.
- alpha blending enabled.
- depth disabled or always-on-top depth state.
- scissor rectangles for clipped widgets and selection lists.

## Text and Fonts

Text is the highest-risk part of not using DOM. Start with the vanilla bitmap font path before considering richer shaping.

Recommended sequence:

1. Load the extracted Minecraft ASCII/default font assets or a generated bitmap font atlas.
2. Implement `Font.width`, `draw`, `drawShadow`, `drawWordWrap`, and centered string helpers enough for screens and buttons.
3. Keep Unicode/fallback/complex shaping out of the first replacement slice unless a visible screen needs it.
4. Add edit-box cursor, selection, clipboard, and repeat-key behavior after buttons/sliders/cycle controls work.

This mirrors the practical vanilla shape: most menu UI can be bootstrapped with simple glyph metrics and shadowed text.

## Assets and Style

Use vanilla GUI assets where they fit:

- `textures/gui/widgets.png` for buttons and sliders
- `textures/gui/options_background.png` for dirt/options background
- `textures/gui/icons.png` for crosshair/HUD pieces later

Do not invent a polished web-app visual language. The baseline should read as Minecraft-like: pixel textures, simple panels, centered screen titles, rectangular buttons, restrained status text, and explicit options lists.

The GUI atlas may be separate from the block atlas. Keeping it separate is simpler and avoids coupling UI upload lifetime to terrain texture stitching.

## Input Routing

Browser event listeners should produce engine input records, not mutate visible DOM.

Minimum input path:

```text
browser events
  -> RawInputSampler
  -> GuiInputFrame
  -> ScreenManager route
  -> if not consumed, gameplay input / pointer lock path
```

Rules:

- When a screen is open, pointer lock should be released or suppressed unless the screen explicitly supports captured mouse input.
- `Esc` closes the current screen when `shouldCloseOnEsc` is true; otherwise it opens `PauseScreen` during gameplay.
- Keyboard text input flows to focused `EditBox`.
- Mouse/touch coordinates are converted from CSS pixels to GUI-scaled pixels before hit testing.
- Touch controls, if present, are GUI widgets rendered through WebGPU. They are not DOM elements.

## Loading Status

The existing loading progress data is valuable and should be kept.

Current sources such as `src/renderer/loading-progress.ts`, host world progress messages, and scene initialization callbacks should feed GUI screens:

- `LoadingScreen`: boot/resource/world generation progress before a playable world exists
- `ReceivingLevelScreen`: joining or reconnecting to a remote host
- `ProgressScreen`: blocking operations such as clearing storage or saving/quitting

The UI surface changes; the progress model does not need to.

## Menu Flow Target

Initial screens:

- `TitleScreen`
  - Continue
  - Start World
  - Options
  - Debug Settings
- `WorldSetupScreen`
  - seed
  - preset
  - movement mode if still needed during development
  - storage mode / clear storage affordance if needed
- `LoadingScreen`
  - stage
  - detail
  - progress bar when known
- `PauseScreen`
  - Back to Game
  - Options
  - Debug Settings
  - Save and Quit to Title
- `OptionsScreen`
  - render distance
  - view distance
  - lighting mode
  - liquid simulation mode
  - storage mode
- `DebugSettingsScreen`
  - debug-only toggles and smoke/probe settings that should not be normal player options

Remote multiplayer can reuse the same screen system later with a connection screen and disconnect screen.

## UI Replacement Targets

Replace or keep retired these visible DOM surfaces:

| Current surface | Replacement |
|---|---|
| `index.html` start menu | WebGPU `TitleScreen` / `WorldSetupScreen` |
| `index.html` CSS world-preview background | WebGPU title background or simple rendered world/panorama later |
| old debug visible overlay/settings | retired; WebGPU pause/options/debug settings screens |
| `src/renderer/debug/debug-free-cam.ts` DOM loading/error overlay | retired; WebGPU loading/progress/error screens |
| `src/renderer/debug/debug-input.ts` DOM joystick and fly buttons | retired; WebGPU touch-control widgets or remove if not needed |
| browser `window.confirm` for destructive world storage reset | WebGPU confirm screen |

Keep machine hooks where tests need them. Visible test control must not become product UI.

## Validation

GUI changes produce pixels, so validation must include screenshots.

Minimum lanes:

- unit tests for widget hit testing, focus traversal, slider/cycle state, text measurement, and draw-list geometry
- `pnpm typecheck`
- `pnpm test:browser` for default boot paths
- focused `pnpm probe:browser -- test/browser/probes/<name>.probe.ts` for every visible GUI milestone
- screenshots saved under `/tmp` and inspected before building the next layer

First visual probes should be small:

1. clear world frame plus a single WebGPU button
2. title screen with three buttons
3. loading screen fed by synthetic progress
4. live world plus pause menu
5. options/debug settings screen with sliders and cycle buttons

## Accessibility Note

Vanilla has narration plumbing. Browser DOM would give accessibility for free in some cases, but visible DOM UI is not the chosen architecture.

Do not let that disappear silently. Keep the vanilla-shaped narration model in the screen/widget interfaces even if the first implementation only records narration text for tests or future browser speech output.

## Architectural Divergence Review

What vanilla does:

- GUI state is Java client objects: `Screen`, widgets, lists, font, and texture blits.
- Rendering goes through OpenGL-era `RenderSystem`, `Tesselator`, `BufferBuilder`, and shader selection.

Why vanilla rendering is a poor fit:

- The browser target is WebGPU, not OpenGL.
- WebGPU requires explicit pipelines, bind groups, buffers, texture views, command encoders, and render passes.
- The DOM cannot be used if we want one rendering/input path for in-game menus, HUD, touch controls, and future fullscreen/gamepad-friendly UI.

Scope of divergence:

- Rendering backend only.
- Browser platform input adapters only.
- Screen/widget logic remains vanilla-shaped.

Parity impact:

- Neutral to positive. A vanilla-shaped GUI model makes future Minecraft-like screens easier to port than a generic web UI framework would.

Constraint:

- Any class with a vanilla counterpart must be read before porting, and direct translation remains the default. WebGPU-specific code stays behind draw-list / renderer boundaries.
