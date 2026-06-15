# 027: Mclone UI Foundation

Status: proposed.

## Purpose

Add a native Rust game UI foundation for title, pause, options, loading, error, debug settings, HUD, and later inventory/chat flows.

The direction is a first-party `mclone-ui` layer: Minecraft-shaped in interaction model, but not an exact visual copy of Minecraft. The legacy TypeScript GUI is useful prior art for architecture and validation, not a skin to preserve.

## Context

The legacy TypeScript implementation had two UI eras:

- early visible DOM shells for start/debug/loading/touch controls
- later canvas-only WebGPU GUI screens under `src/client/gui/` and `src/renderer/gui/`

The later path is the useful one. It used `Screen`, `Widget`, `Button`, sliders, cycle buttons, focus/hover/input routing, a renderer-neutral GUI draw list, and a WebGPU overlay pass. It also used vanilla Minecraft assets such as `textures/gui/widgets.png` and `font/ascii.png`, which made it read as a close Minecraft visual clone.

Native should keep the engine-rendered UI architecture and replace the visual skin with original mclone assets.

## Target Shape

Create a small first-party UI stack with this ownership split:

```text
native/crates/mclone-ui/
  screen.rs
  screen_manager.rs
  input.rs
  draw_list.rs
  font.rs
  style.rs
  widgets/
    button.rs
    slider.rs
    checkbox.rs
    cycle_button.rs
    edit_box.rs

native/crates/mclone-render/
  gui/
    renderer.rs
    pipeline.rs
    atlas.rs
    shaders/gui_solid.wgsl
    shaders/gui_textured.wgsl
```

Exact file placement can shift if the crate boundaries argue for it, but preserve the split:

- `mclone-ui` owns UI state, screen lifecycle, widget behavior, focus, input routing, GUI-scaled coordinates, and draw-list generation.
- `mclone-render` owns `wgpu` resources, pipelines, bind groups, atlas uploads, scissor state, and encoding the draw list after the world pass.
- app crates own `winit`/browser event translation, pointer lock/cursor visibility, window lifecycle, and presentation pacing.
- no `winit`, DOM, Android, OpenXR, or `wgpu` handles should leak into UI model code.

## Reference Shape

Read the vanilla source for behavior shape before implementing equivalent classes:

- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/Screen.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/GuiComponent.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Widget.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/events/GuiEventListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractWidget.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractButton.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Button.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/AbstractSliderButton.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/Checkbox.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/CycleButton.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/components/EditBox.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/Font.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/TitleScreen.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/PauseScreen.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/OptionsScreen.java`

Use the TypeScript GUI as implementation prior art:

- `src/client/gui/`
- `src/renderer/gui/`
- `docs/gui.md`
- `docs/tactical/Gui0-webgpu-gui-foundation-and-dom-replacement.md`

Do not port the TypeScript browser orchestration or exact visual assets as the native direction.

## Style Direction

The UI should feel like an engine-native voxel game UI:

- full-canvas / full-window game UI, not host-native controls
- crisp integer GUI scaling
- bitmap or atlas-backed text with predictable metrics
- centered title/pause layouts and simple option rows
- small, explicit widgets with hover, pressed, disabled, and keyboard focus states
- nine-slice or sliced textured controls for buttons and panels
- restrained color, high contrast, and good readability over terrain

Avoid exact Minecraft visual cloning:

- do not use vanilla `widgets.png`, `ascii.png`, dirt background, logo, or icons for final player-facing UI
- create an original mclone GUI atlas with its own font, controls, icons, and panel textures
- keep the interaction grammar familiar, but make the art direction distinct

`egui` can be considered later for developer-only inspectors. It should not be the player-facing title/pause/options/inventory UI path.

## First Slice

Land the minimum real UI stack and prove it with pixels.

### 1. UI Model Crate

Add `mclone-ui` with:

- `GuiScale` / GUI size calculation
- `GuiInputEvent` for pointer move/down/up, wheel, key press/release, char input, and focus loss
- `GuiDrawList` commands:
  - solid rect
  - vertical gradient rect
  - textured rect
  - nine-slice rect if needed for first controls
  - text run / glyph quads
  - clip rect push/pop
- `ScreenManager` with one active screen and input routing
- `Screen` trait or struct pattern with `init`, `tick`, `render`, `resize`, `removed`, `on_close`, and `should_close_on_escape`
- `Widget` / event listener traits
- first widgets:
  - `Button`
  - `Slider`
  - `Checkbox`
  - `CycleButton`

Use Rust ownership that fits the app; do not force a Java class hierarchy if closures/enums are cleaner. Keep method names and behavior close enough that future parity work can still map back to vanilla.

### 2. Render Backend

Add a `wgpu` GUI renderer in `mclone-render`:

- render after the world pass into the same frame target
- alpha blending enabled
- depth disabled
- solid-color and textured pipelines
- nearest sampling for UI atlas textures
- scissor rectangles for clips
- no dependency on `winit`
- compatible with both swapchain and headless/offscreen targets through the existing render frame boundary

Keep the first batching simple. Correct draw order, clipping, and visual inspection matter more than optimal batching in this slice.

### 3. Original UI Assets

Add a small original mclone UI atlas or generated placeholder assets:

- simple bitmap font or MSDF-free atlas with fixed metrics
- button/panel/control texture slices
- basic icons only if a first screen needs them

Temporary procedural rectangles are acceptable for bring-up, but the tactical is not complete until the player-facing first screens no longer depend on vanilla GUI assets.

### 4. First Screens

Implement enough screens to replace command-line-only native interaction for normal smoke use:

- `TitleScreen`
  - Start Local World
  - Options
  - Quit
- `ProgressScreen`
  - stage text
  - optional detail text
  - progress bar when a fraction exists
- `PauseScreen`
  - Back to Game
  - Options
  - Quit to Title or Quit
- `OptionsScreen`
  - render distance / chunk radius
  - fullbright or lighting mode toggle if lighting is active
  - culling/debug visibility toggles as appropriate

Debug-only controls may be sparse. Keep the main loop usable without requiring CLI flags for the common local world path.

### 5. Input And Gameplay Boundary

Wire `winit` events at the native app boundary:

- convert window pointer/key/text events into `GuiInputEvent`
- route input to `ScreenManager` first when a screen is open
- suppress gameplay camera/movement while a screen is open
- on `Esc`, open pause during gameplay and close eligible screens
- release or show cursor while menus are open; recapture/hide for gameplay where appropriate

Do not bake desktop cursor behavior into shared UI code. Browser and Android input adapters should be able to produce the same UI events later.

## Validation

Minimum first-slice gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Rendered-pixel gates:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --headless-ui /tmp/mclone-ui-title.png --width 960 --height 540
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client
```

Inspect `/tmp/mclone-ui-title.png` and a live native window. The UI must be nonblank, readable, correctly scaled, not clipped, and responsive to pointer/keyboard input.

If the first slice affects the web/WASM shell, add or update the browser smoke so it proves the UI model and renderer remain web-compatible without visible DOM controls.

## Done For First Slice

- native app can show a rendered title screen before entering a world
- loading/progress is visible through the UI renderer, not terminal-only
- `Esc` pause flow works over the world view
- basic options mutate real runtime/render settings or clearly mark restart-required settings
- headless/offscreen capture can render at least one UI screen to `/tmp`
- no player-facing UI depends on host-native widgets, visible DOM, or vanilla GUI textures
- web/WASM still compiles

## Follow-Up Slices

Likely follow-ups after the foundation:

- edit box and world setup screen for seed/name/storage profile
- confirm/error screens for destructive reset, disconnect, and save failures
- scrollable lists for world selection and server list
- HUD layer, crosshair, debug text, and touch controls through the same draw-list path
- chat input and text wrapping
- inventory/container screens with item rendering handoff
- accessibility pass: keyboard traversal, repeat keys, text cursor, clipboard, and readable focus states
- original UI asset polish and visual regression screenshots

## Out Of Scope

- inventory, recipe book, chat, or item rendering in the first slice
- exact Minecraft panorama/logo/dirt/widgets/font visuals
- broad accessibility/narration parity
- IME/complex text shaping
- host-native UI frameworks for player-facing screens
- Android, OpenXR, WebXR, or Gradle scaffolding
- CSS/DOM menus in the web target
- a large immediate-mode UI dependency as the primary menu system
