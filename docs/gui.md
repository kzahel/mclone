# GUI Architecture

Status: draft for review.

This document describes the shared GUI architecture for mclone. The UI
system is owned by `mclone-ui`; GPU execution, glyph atlases, batching, item
icon rendering, and panel texture caching are owned by `mclone-render`.
Platform apps own only window/session/input glue.

The target is a vanilla-shaped Minecraft GUI model with a retained, cached GPU
implementation. Menus, HUD, hotbar, in-game block picker, tooltips, and XR
world-space panels must all use the same shared UI contracts. Validation should
exercise the relevant flat, XR, Android, and web boundaries without treating
one client target as the baseline.

## Core Decisions

1. UI behavior is retained and shared.
   Screens, widgets, focus state, hover state, list state, selected tabs, text
   field contents, scroll offsets, and layout results are durable objects in
   `mclone-ui`, not temporary frame-local helper data.

2. Painting is dirty-tracked.
   UI surfaces repaint only when semantic UI state, layout, text, texture
   assets, scale, or viewport size changes. Controller/head pose changes in XR
   do not force menu content to repaint.

3. Text is atlas-backed.
   Glyphs are cached into one or more font atlas pages. A glyph is rendered as
   a textured quad. The current bitmap-font path that emits one tiny rectangle
   per glyph pixel is a transitional implementation and is not acceptable for
   production menus or in-game UI.

4. In-game UI is a first-class performance path.
   Hotbar, crosshair, block picker, inventory-style grids, tooltips, and status
   overlays run in the main render loop and need the same caching discipline as
   menus. They should not be treated as a separate immediate-mode debug layer.

5. Renderer data is explicit.
   `mclone-ui` emits semantic paint operations. `mclone-render` converts them
   into GPU batches, glyph atlas uploads, item icon draws, and optional
   offscreen panel textures. XR and app crates choose where those surfaces are
   displayed; they do not own UI policy.

## Vanilla Reference Shape

Minecraft 1.17.1 is the behavioral reference for screens, widgets, text
measurement, inventory screens, and HUD semantics. The important source files
are under `reference/minecraft-1.17.1/src/net/minecraft/client`.

| Vanilla class | Idea to borrow | Mclone owner |
| --- | --- | --- |
| `gui/screens/Screen.java` | Screen lifecycle, children/renderables/narratables split, resize/init flow, focus traversal, tooltip placement | `mclone-ui` |
| `gui/GuiComponent.java` | Small primitive vocabulary: filled rects, gradients, blits, strings | `mclone-ui` paint model and `mclone-render` backend |
| `gui/components/AbstractWidget.java` | Widget rectangles, visible/active/focused/hovered state, click/drag bounds | `mclone-ui` |
| `gui/components/Button.java` | Press callbacks and vanilla button sizing/state behavior | `mclone-ui` |
| `gui/components/AbstractSliderButton.java` | Slider state, dragging, update/apply hooks | `mclone-ui` |
| `gui/components/Checkbox.java` | Toggle state and vanilla interaction shape | `mclone-ui` |
| `gui/components/CycleButton.java` | Cycling enum/list options | `mclone-ui` |
| `gui/components/EditBox.java` | Text cursor, selection, focus, filtering, narration hooks | `mclone-ui` |
| `gui/components/AbstractSelectionList.java` | Scrollable list state, rows, hover, selection | `mclone-ui` |
| `gui/Font.java` | Text measurement, bidi/shaping boundary, draw/drawShadow APIs, per-glyph rendering | `mclone-ui` text model and `mclone-render` glyph renderer |
| `gui/font/FontSet.java` | Lazy glyph lookup and glyph-info caches | `mclone-render` font cache |
| `gui/font/FontTexture.java` | Font atlas pages and glyph packing | `mclone-render` font atlas |
| `gui/font/glyphs/BakedGlyph.java` | One cached glyph becomes one textured quad | `mclone-render` text batches |
| `renderer/MultiBufferSource.java` | Render-type/material buckets with explicit flush points | `mclone-render` batching |
| `gui/Gui.java` | HUD/hotbar/crosshair semantic behavior | `mclone-ui`, with GPU execution in `mclone-render` |
| `gui/screens/inventory/AbstractContainerScreen.java` | Slot grid model, hovered slot, carried item, tooltip and highlight behavior | `mclone-ui` plus render item-icon boundary |
| `gui/screens/inventory/CreativeModeInventoryScreen.java` | Creative inventory tab/search/scroll model and visible slot window | `mclone-ui` block picker |
| `renderer/entity/ItemRenderer.java` | Item/block icon renderer boundary and item decorations | `mclone-render` |

The useful Minecraft ideas are the model boundaries, not the OpenGL-era
submission path. Vanilla builds many GUI elements every frame, but it does so
with compact textured quads and render-type batching. In mclone, XR
world-space panels and Quest-class hardware make cached panel textures and
dirty-tracked paint lists a better fit.

## Goals

- Preserve vanilla-shaped screen and widget behavior where practical.
- Keep UI policy shared across desktop, XR, Android, and web.
- Repaint only changed surfaces/layers.
- Make text, icons, slots, and panel backgrounds batchable GPU data.
- Keep hotbar and block picker cheap enough for every-frame use.
- Keep renderer-facing data explicit and testable.
- Leave room for accessibility/narration, localization, IME/text editing, and
  controller/gamepad focus without tying them to one platform.

## Non-Goals

- Do not use DOM elements, OS widgets, or browser form controls for visible game
  UI.
- Do not adopt a broad retained web-app UI framework.
- Do not pull in a large immediate-mode GUI library as the default path.
- Do not make desktop-only, web-only, Android-only, or XR-only UI policy.
- Do not keep the current per-glyph-pixel text draw path as the production text
  renderer.

## Platform Boundary

All visible game UI renders through the engine renderer. On web/WASM, the DOM is
only a shell for:

- one visible canvas
- script loading
- browser keyboard, mouse, wheel, touch, pointer-lock, focus, resize,
  clipboard, and storage APIs
- test-only hooks such as readiness/debug globals
- browser capability adapters, such as file picker or URL opening, if a future
  flow needs them

The DOM is not allowed for visible menus, HUD, debug panels, touch controls,
loading text, or settings widgets. Native desktop, Android, and XR should follow
the same principle with their own platform shells.

## Source Layout

- `native/crates/mclone-ui`
  - Retained screen tree.
  - Widgets, layout, focus, hit testing, input dispatch.
  - Dirty flags and surface/layer state.
  - Semantic paint operations.
  - Text model: runs, styles, measurement requests, wrapping.
  - HUD, hotbar, block picker, menu, tooltip behavior.

- `native/crates/mclone-render`
  - GPU UI renderer.
  - Paint-op batching.
  - Glyph atlas and text mesh cache.
  - Sprite/icon atlas integration.
  - Item/block GUI icon rendering.
  - Offscreen UI panel texture cache for XR/world-space panels.

- `native/crates/mclone-render-session`
  - Render-target/view contracts shared by flat, XR, headless, and web.
  - UI pass scheduling and surface descriptors.

- `native/crates/mclone-input`
  - Shared logical actions, pointer/controller events, focus navigation inputs.

- `native/crates/mclone-xr-scene`
  - XR panel placement, controller rays, pointer projection, per-eye/multiview
    composition.
  - Requests UI surface repaint only when UI content is dirty.

- App crates
  - `winit`, browser, Android, and OpenXR lifecycle glue.
  - Surface/session/swapchain ownership.
  - Platform input conversion into shared input events.

## Data Model

### UiContext

`UiContext` owns global UI resources that are independent of one screen:

- current viewport and UI scale
- input state snapshot
- focused surface/widget
- pointer captures
- text measurement cache keys
- current theme/vanilla asset handles
- monotonic revision counters

### UiSurface

A `UiSurface` is a cacheable drawable UI target. Examples:

- pause menu panel
- options menu panel
- controls menu panel
- HUD overlay
- hotbar layer
- block picker layer
- tooltip layer
- loading screen
- debug/status overlay

Each surface has:

- stable `UiSurfaceId`
- logical size and scale
- retained screen/layer object
- layout revision
- paint revision
- text revision
- GPU batch revision
- optional panel texture revision
- dirty flags

Surfaces can be composed into a final view differently per platform. A flat
desktop menu may draw directly to the swapchain. XR may draw the same surface to
a cached panel texture and then place that texture on a world-space quad.

### UiLayer

Layers let dynamic and static content invalidate independently. A single screen
or HUD can contain several layers:

- background
- static chrome
- widget bodies
- text
- icons/items
- hover/focus/selection overlays
- tooltip
- cursor/controller reticle

The implementation does not need excessive layering at first. The important
rule is that high-frequency state, such as hover highlight or selected hotbar
slot, should not force a full text/menu rebuild when it can be isolated.

## Dirty Tracking

Dirty state is explicit and conservative. False positives are acceptable during
migration; false negatives are not.

### Dirty Kinds

- `LayoutDirty`
  - viewport size, UI scale, screen switch, list contents, wrapping width, font
    metrics, or widget tree changed

- `PaintDirty`
  - visual state changed without requiring new layout: hover target, pressed
    state, selected slot, slider value, checkbox value, scroll offset, tab,
    tooltip visibility

- `TextDirty`
  - text contents, style, language, font set, wrapping, or measurement cache
    changed

- `AssetsDirty`
  - texture atlas, block/item icons, glyph atlas page, theme assets, or sampler
    state changed

- `GpuDirty`
  - paint ops changed and CPU-side GPU batches must be rebuilt

- `PanelTextureDirty`
  - rendered offscreen panel contents are stale

### Dirty Rules

- Pointer movement updates hover only when the hovered widget/slot changes.
- Controller/head pose changes update interaction rays and world transforms,
  but not UI layout, paint ops, or panel texture contents.
- Text measurement is cached by font, scale, style, text, and wrapping width.
- Scrolling a list dirties the visible row window, not unrelated menu chrome.
- Search text in the block picker dirties the item list and visible slot window
  only when the text actually changes.
- Hotbar selection dirties the selection overlay and item animation state, not
  the whole HUD.
- Tooltips are independent small layers and should not invalidate their parent
  menu or inventory layer.

## Paint Model

`mclone-ui` should emit a semantic paint list, not final GPU vertices. The
paint list should be compact and stable enough to cache.

Example shape:

```rust
pub enum UiPaintOp {
    Rect {
        rect: UiRect,
        color: UiColor,
        radius: UiRadius,
    },
    Gradient {
        rect: UiRect,
        top: UiColor,
        bottom: UiColor,
    },
    Border {
        rect: UiRect,
        color: UiColor,
        width: f32,
    },
    Text {
        run: TextRunId,
        position: UiPoint,
        color: UiColor,
        shadow: bool,
    },
    Sprite {
        rect: UiRect,
        sprite: UiSpriteId,
        tint: UiColor,
    },
    ItemIcon {
        rect: UiRect,
        item: UiItemIconId,
        decorations: ItemDecorationFlags,
    },
    ClipStart(UiRect),
    ClipEnd,
}
```

The current `GuiDrawCommand` path is a transitional low-level draw list. It is
useful for bootstrapping and tests, but it is too low level to be the long-term
UI contract because text has already been exploded into per-pixel rectangles by
the time the renderer sees it.

## Renderer Model

`mclone-render` consumes dirty paint lists and produces cached GPU data.

Renderer-owned caches:

- glyph atlas pages
- glyph metadata and UVs
- text run meshes
- sprite/icon atlas bindings
- item/block GUI icon meshes
- per-surface UI batch meshes
- per-panel offscreen textures

Batching rules:

- One glyph is one textured quad.
- Rects and gradients are quads.
- Sprite blits are quads.
- Item/block icons use the item renderer boundary and may have their own
  material pipeline.
- Batches are grouped by material/atlas/clip/scissor state.
- Existing GPU buffers are reused when capacity allows.
- Rebuild GPU batches only when the source paint revision changes.

The renderer may still submit cached batches every frame when the surface is
visible. The expensive work to avoid is per-frame text rasterization, per-frame
menu layout, per-frame paint-list construction for unchanged menus, per-frame
vertex generation for unchanged panels, and per-frame offscreen panel repaint
when only XR pose changed.

## Text and Fonts

The text system should borrow Minecraft's `Font`, `FontSet`, `FontTexture`, and
`BakedGlyph` split:

- `mclone-ui` owns text runs, style, wrapping, and measurement requests.
- `mclone-render` owns glyph lookup, glyph atlas pages, and text mesh output.
- Font glyphs are lazily uploaded into atlas pages.
- Missing glyph and white glyph resources are explicit.
- Text width and line splitting are cached.
- Shadow text is a second draw of the same glyph run with offset/color, not a
  second per-pixel CPU rasterization.

Initial scope can be ASCII plus the existing bitmap data if needed, as long as
the renderer-facing model is atlas glyph quads. Localization, bidi shaping,
fallback font sets, and richer Unicode can be added behind the same text-run
and glyph-cache boundary.

## Screens and Widgets

Screens follow the vanilla lifecycle:

- construct screen state
- initialize at viewport size and scale
- create children/widgets/renderables
- update retained state from input
- mark layout/paint/text dirty as needed
- emit paint ops when dirty
- close or resize through explicit lifecycle methods

Widgets should preserve vanilla-style explicit state:

- bounds
- message/label
- visible
- active
- focused
- hovered
- pressed/dragging where relevant
- narration/accessibility metadata later

Useful initial widgets:

- button
- icon button
- checkbox/toggle
- cycle button
- slider
- text field
- scroll list
- tab bar
- slot grid
- tooltip

This remains intentionally simpler than a general DOM or flexbox system.
Minecraft UI is mostly absolute, grid, row, and centered layouts. We should add
layout helpers that fit those shapes instead of importing a broad UI framework.

## Assets And Style

Use vanilla GUI assets where they fit:

- `textures/gui/widgets.png` for buttons, sliders, and selected hotbar pieces
- `textures/gui/options_background.png` for dirt/options backgrounds
- `textures/gui/icons.png` for crosshair and HUD pieces

Do not invent a polished web-app visual language. The baseline should read as
Minecraft-like: pixel textures, simple panels, centered screen titles,
rectangular buttons, restrained status text, explicit options lists, and
inventory-style slots.

The GUI atlas may be separate from the block atlas. Keeping it separate avoids
coupling UI upload lifetime to terrain texture stitching. Item and block icons
can still cross the item-renderer boundary when a slot needs gameplay content.

## HUD, Hotbar, and Block Picker

The in-game UI should be split into independently cacheable layers.

### HUD

Always-on HUD state should have small update surfaces:

- crosshair
- hotbar background
- selected hotbar slot
- item icons
- health/food/air/armor bars when implemented
- selected item name fade
- debug/status overlays

The hotbar background and slot geometry are stable. Selected slot highlight,
item stack changes, durability bars, cooldowns, and pop animations are dynamic
but localized.

### Block Picker

The creative block picker should borrow the `CreativeModeInventoryScreen`
shape:

- persistent full item/block list
- visible slot window
- tab/category state
- search text
- scroll offset
- cached slot grid geometry
- cached item/block icon handles for visible slots
- tooltip layer for hovered slot

Changing search text, category, or scroll offset updates the visible slot
window. Moving the pointer within the same slot does not rebuild the grid.

### Item And Block Icons

`mclone-ui` should not know how to render block/item meshes. It should request
an `ItemIcon` paint op with an item/block identity, count, durability/cooldown
decoration state, and target rectangle. `mclone-render` resolves that through a
GUI item-icon renderer, analogous to vanilla `ItemRenderer`.

## XR And World-Space Panels

XR panels have two different kinds of change:

- content change: menu/widget/text/selection changed
- placement change: head pose, controller pose, or panel world transform changed

Only content change should repaint the panel texture. Placement change should
reuse the current panel texture and only update the world-space quad/ray
composition.

Required XR behavior:

- The same `UiSurface` works in flat and XR.
- Multiview and per-eye paths share surface cache state.
- Pointer rays and reticles can render every frame as small dynamic overlays.
- Panel textures have stable logical size unless the UI scale or panel layout
  actually changes.
- UI performance metrics distinguish paint-list build, GPU-batch build, panel
  repaint, and panel composite.

This avoids the current bad case where opening a text-heavy controls screen can
cause huge CPU draw-list generation and panel repaint work every XR frame.

## Input And Focus

Input is translated into shared UI events before it reaches screens:

- pointer move/down/up
- wheel/scroll
- key down/up
- text input
- gamepad/controller navigation
- XR ray hover/press

`mclone-ui` owns hit testing, focus changes, pointer capture, drag state, and
screen-level shortcuts such as Escape/back. Platform crates own only conversion
from OS/browser/XR events into these shared events.

Focus navigation should be compatible with vanilla `Screen` and
`GuiEventListener` behavior: keyboard Tab and controller navigation can move
between focusable widgets without depending on mouse coordinates.

## Accessibility And Narration

Vanilla screens and widgets carry narration state. The first implementation does
not need full screen-reader integration, but the retained model should keep
narratable text and focus priority available on screens and widgets.

This is especially important because visible DOM UI is not the chosen
architecture. Accessibility data should be part of the shared UI model rather
than an accidental side effect of browser elements.

## Instrumentation

The UI renderer should report enough numbers to make regressions obvious:

- UI surface id/name
- screen/layer name
- paint op count
- text run count
- glyph count
- newly uploaded glyphs
- glyph atlas pages
- GPU batch count
- vertex/index count
- layout build time
- paint-list build time
- text shaping/measurement time
- GPU batch build time
- panel repaint time
- panel composite time
- cache hit/miss counts

XR summaries must include the real UI command/paint counts for the active menu
or panel. Reporting zero for a rendered panel hides the exact class of issue
this architecture is meant to prevent.

## Validation

Validation should include correctness and performance checks.

Correctness:

- headless screenshots for menu screens
- screenshots for HUD/hotbar/block picker
- XR panel screenshots where available
- widget hit-test tests
- focus traversal tests
- text measurement/wrapping tests
- slot-grid scrolling/search tests

Performance:

- open pause/options/controls menus and record paint op, glyph, batch, and panel
  repaint counts
- assert unchanged menus do not rebuild paint lists or repaint panel textures
  across idle frames
- assert pointer movement within the same hover target does not repaint the
  whole surface
- assert hotbar selection dirties only the relevant layer
- assert block picker scroll/search dirties only the visible slot window and
  tooltip/hover layers

For any slice that produces pixels, capture and inspect screenshots
before moving on, per `AGENTS.md`.

## Migration Plan

1. Add UI instrumentation around current paths.
   Measure draw-list build time, command counts, glyph-like rectangle counts,
   renderer prepare time, panel repaint time, and XR panel composite time.

2. Introduce atlas-backed text rendering.
   Keep the current font appearance initially if that is fastest, but change
   renderer-facing output to glyph quads and cached text runs.

3. Add `UiSurface`, `UiLayer`, and dirty flags.
   Start with conservative invalidation. Make menus produce cached paint lists
   rather than rebuilding every frame.

4. Cache XR panel textures.
   Repaint panel content only when the surface content revision changes. Keep
   controller rays and panel placement dynamic.

5. Convert menus to retained widgets.
   Pause/options/controls should stop building all rows and all text every
   render call.

6. Convert HUD/hotbar.
   Separate stable hotbar geometry from selected-slot/item animation state.

7. Convert block picker.
   Use a retained item list, visible slot window, cached grid geometry, item
   icon paint ops, and independent tooltip layer.

8. Tighten batching and cache lifetimes.
   Group by atlas/material/clip, reuse buffers, and make cache invalidation
   visible in diagnostics.

## Architectural Divergence Review

Minecraft 1.17.1 is immediate in many GUI call sites: `Screen.render`,
`Gui.render`, `AbstractContainerScreen.render`, and widget `render` methods run
every frame. That shape works in vanilla because rendering is mostly compact
quads, texture blits, glyph atlas draws, and `MultiBufferSource` render-type
batches.

mclone should borrow the screen/widget/text/item model, but diverge in the
renderer and cache policy:

- retained UI surfaces instead of rebuilding all screen paint every frame
- dirty-tracked layout/paint/text/GPU/panel revisions
- atlas-backed text from the start of the production path
- cached XR panel textures
- explicit item-icon render boundary
- platform-independent input/focus model

This divergence should make future vanilla parity easier, not harder. The
behavioral model remains close to vanilla, while the GPU and cache model is
adapted to wgpu, headless validation, web/WASM, and XR performance needs.
