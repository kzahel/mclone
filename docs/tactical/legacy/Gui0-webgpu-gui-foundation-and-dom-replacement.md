# Gui0: WebGPU GUI Foundation and DOM UI Replacement

Status: active GUI arc.

Current implementation state:

- Gui0 foundation exists under `src/client/gui/` and `src/renderer/gui/`.
- The opt-in GPU title flow can show title, loading progress, and a live generated world.
- `Esc` opens a minimal GPU-rendered `PauseScreen` over the live world; Back to Game closes it and gameplay input resumes.
- A minimal GPU-rendered `OptionsScreen` is reachable from title and pause, with sliders/cycle/checkbox controls for browser render settings.
- A minimal GPU-rendered `DebugSettingsScreen` is reachable from title and pause, sharing the existing debug session storage keys.
- `/?mode=debug` is now a root-shell debug launch mode. Its loading/progress/error/HUD text and pause/options/debug-settings flows render through the WebGPU GUI pass, and `window.__mcloneDebug` remains as a machine API.
- Quit-to-title, destructive confirm screens, and any richer touch HUD remain later slices.

Durable architecture: [`../gui.md`](../gui.md).

## Goal

Replace the current visible DOM UI with a vanilla-shaped 2D GUI system rendered through WebGPU. The first milestone is not a complete Minecraft UI; it is enough infrastructure to boot to a GPU-rendered title screen, show honest loading progress, enter a world, open a pause/options/debug settings screen, and remove the current DOM start/debug overlays.

The existing loading status model is useful and should be preserved. The current visual surface shape is disposable.

## Current DOM Inventory

Visible DOM surfaces to replace:

| Surface | Current role | Replacement |
|---|---|---|
| `index.html` start shell | seed/preset/movement form, continue/quickstart links, CSS background | WebGPU title and world setup screens |
| old debug shell | retired visible debug page UI and overlays | root shell with debug query params |
| `src/renderer/debug/debug-free-cam.ts` overlay helpers | retired loading/error text, progress bar, debug settings form binding | WebGPU loading/error/options screens |
| `src/renderer/debug/debug-input.ts` DOM joystick/buttons | retired mobile/touch debug controls | WebGPU touch widgets or remove until a real touch HUD lands |
| browser confirm dialogs | destructive reset confirmation | WebGPU confirm screen |

Non-visible browser APIs remain allowed: canvas, event listeners, localStorage, IndexedDB, pointer lock, clipboard, tests, and machine-readable globals.

## Hard Constraints

- Do not add any visible DOM UI while replacing the old UI.
- Do not adopt a generic app UI framework.
- Port vanilla-shaped GUI classes only after reading their 1.17.1 source files.
- Keep rendering divergence behind WebGPU draw-list and renderer modules.
- Preserve `window.__mcloneReady` / browser test hooks as machine APIs, not visible UI.
- Save screenshots under `/tmp` for every visible milestone.

## Tactical Sequence

| Slice | Scope | Validation | Done when |
|---|---|---|---|
| Gui0 | GUI primitives, screen manager, WebGPU overlay pass, bitmap text, buttons, loading screen probe | unit + browser probe | a GPU-rendered screen with text/buttons/progress draws over the world/clear frame |
| Gui1 | Canvas-only title/start flow replacing `index.html` UI | browser smoke + probe | `/` shows a WebGPU title screen and can start/continue a world without visible DOM |
| Gui2 | Loading and error screens wired to real `LoadingProgress` and scene init | browser smoke + probe | world boot progress/status renders through WebGPU from existing progress sources |
| Gui3 | Pause/options/debug settings screens replacing the old debug form/overlay | browser integration + probe | **done for debug launch mode** - live world uses `Esc`/menu flow for settings, no DOM debug panel remains |
| Gui4 | In-game menu lifecycle and quit-to-title/reset flow | browser integration | save/quit/disconnect/clear-storage confirmation flows use WebGPU screens |
| Gui5 | HUD/touch-control cleanup and DOM deletion pass | browser integration + mobile/touch probe if kept | DOM joystick/buttons and obsolete CSS UI are gone; HUD/touch controls are GPU widgets |

The sequence can merge slices if implementation stays small, but preserve the validation gates. Do not finish a visible slice without looking at a screenshot.

## Gui0 Detailed Scope

Add the foundation modules:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/client/gui/screens/screen.ts` | vanilla-shaped base screen with `init`, `tick`, `render`, `resize`, `onClose`, child lists, and focus routing |
| 2 | `src/client/gui/components/*` | `Widget`, `GuiEventListener`, `AbstractWidget`, `AbstractButton`, `Button`, basic `SliderButton`, `Checkbox`, `CycleButton` |
| 3 | `src/client/gui/gui-component.ts` | `fill`, `fillGradient`, `blit`, `drawString`, `drawCenteredString` facade that writes draw commands |
| 4 | `src/client/gui/font.ts` | first bitmap font metrics and shadowed text draw commands |
| 5 | `src/client/gui/screen-manager.ts` | active screen ownership, input routing, resize, tick/render lifecycle |
| 6 | `src/renderer/gui/gui-draw-list.ts` | rect, gradient, textured-quad, text-glyph, and clip commands |
| 7 | `src/renderer/gui/gui-renderer.ts` | WebGPU overlay pass with alpha blending and orthographic GUI coordinates |
| 8 | `src/renderer/gui/gui-texture-atlas.ts` | GUI asset upload for widgets/background/font textures, separate from the block atlas if simpler |
| 9 | probe screen | test screen with background, title, buttons, slider, checkbox, and progress bar |

Allowed first-slice simplifications:

- ASCII/default glyph coverage only.
- Simple fixed layouts, no full selection list yet.
- One GUI texture atlas.
- Basic click/hover/keyboard focus without full narrator output.
- Synthetic loading progress for the first visual probe.

Do not include:

- edit-box text input unless needed for the first title flow
- inventory, chat, item rendering, or recipe book
- full vanilla accessibility/narration implementation
- full Unicode shaping
- DOM fallback UI

## Gui1: Title and World Setup

Replace the visible root page with canvas-rendered screens.

Target behavior:

- `index.html` becomes a canvas-only shell.
- The app initializes WebGPU before world creation and shows `TitleScreen`.
- `TitleScreen` supports Continue, Start World, Options, and Debug Settings.
- `WorldSetupScreen` supports seed and preset selection.
- Existing localStorage-backed defaults may remain, but the controls are WebGPU widgets.
- Starting a world transitions into `LoadingScreen`, then gameplay.

Implementation notes:

- If edit-box support is too much for Gui1, accept a small set of seed buttons first and move arbitrary seed entry to Gui1b. Do not reintroduce a DOM input field.
- Continue/quickstart behavior can map to the existing stored debug/session config keys initially, then be renamed later.
- Browser smoke must still resolve `window.__mcloneReady` with enough data for tests.

Validation:

- `pnpm typecheck`
- focused unit tests for screen navigation and option mutation
- `pnpm test:browser`
- a browser probe screenshot of title -> loading -> first world frame under `/tmp`

## Gui2: Loading and Error Status

Wire real progress into GPU screens.

Target behavior:

- `LoadingProgress` from scene/resource/world initialization renders as stage, detail, and optional progress bar.
- Remote/world host errors render as a WebGPU error or disconnected screen.
- Clearing IndexedDB/storage shows a progress/confirm flow instead of DOM text or browser confirm.
- No visible loading text is written through `textContent`, CSS overlays, or HTML progress bars.

Validation:

- unit tests for progress formatting
- browser probe with synthetic slow progress if needed
- default browser smoke still sees successful boot

## Gui3: Debug Settings and Options

Move current debug page controls into the screen system.

Target behavior:

- The debug launch mode no longer owns a visible form or separate HTML shell.
- `Esc` opens `PauseScreen` in a live world.
- `OptionsScreen` owns player-facing settings: render distance, view distance, lighting mode, liquid simulation mode, storage mode.
- `DebugSettingsScreen` owns development-only controls: movement mode, preset, clear storage, remote/local transport, perf counters visibility, and other debug toggles.
- Settings changes either apply live where safe or clearly require restart/reload through a WebGPU confirm screen.
- `window.__mcloneDebug` remains for tests and automation.

Validation:

- `pnpm test:browser:integration` if debug camera/player controls or resize behavior changes
- browser probe for live world + pause/options/debug settings
- inspect screenshot for text fit, focus/hover state, and no DOM overlay

## Gui4: In-Game Menu Lifecycle

Make menu flow a real game concept.

Target behavior:

- `PauseScreen`
  - Back to Game
  - Options
  - Debug Settings
  - Save and Quit to Title
- `ConfirmScreen`
  - destructive reset confirmation
  - quit/disconnect confirmation if needed
- `TitleScreen`
  - receives control after quit-to-title
  - can reopen the same save or start a new one
- Gameplay input is suppressed while a screen is open.
- Pointer lock is acquired only during gameplay and released/suppressed during menus.

Validation:

- unit tests for screen transitions
- browser integration for start world -> pause -> options -> back -> quit to title
- visual probe screenshots for pause and confirm screens

## Gui5: HUD and Touch Cleanup

Finish the visible-DOM removal.

Target behavior:

- DOM joystick and fly buttons are gone.
- If touch controls are still needed, they render as WebGPU widgets with the same input routing as other GUI controls.
- Minimal HUD can include crosshair and status text/counters only if they render through WebGPU.
- Debug/perf overlays, if retained, are GUI overlays toggled by debug settings.
- Remove obsolete CSS and HTML nodes from `index.html`.

Validation:

- grep for visible DOM mutation patterns in presentation code: `textContent`, `innerHTML`, `style.display`, `classList`, DOM form controls, and visible buttons.
- `pnpm test:browser`
- relevant browser integration/probe lanes

## Input and Browser Test Compatibility

Tests should not need DOM controls to drive UI.

Keep or add test APIs that operate at the engine level:

- current ready/debug globals
- direct screen command helpers where useful
- input injection helpers that simulate GUI events or gameplay input

Do not add test-only visible HTML.

## Validation Baseline

Docs-only edits can stop at `git diff --check`.

Implementation slices should run:

- `pnpm typecheck`
- focused `pnpm test` coverage for GUI classes
- `pnpm test:browser`
- `pnpm test:browser:integration` when touching debug/live controls, resize, pointer lock, chunk-interest movement, or runtime flow
- focused `pnpm probe:browser -- test/browser/probes/<name>.probe.ts` for each visible milestone

Every GUI probe screenshot must be saved under `/tmp` and inspected before continuing.

## Done for the Arc

- `index.html` and any remaining debug shell are canvas-only visible surfaces.
- All menus, loading status, settings, debug settings, HUD/touch overlays, and confirm/error screens render through WebGPU.
- The GUI model is vanilla-shaped enough to port additional Minecraft screens without changing architecture.
- The old DOM UI code paths are deleted, not merely hidden.
- Browser smoke and integration lanes no longer depend on visible DOM widgets.
