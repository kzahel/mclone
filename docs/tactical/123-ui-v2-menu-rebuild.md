# 123: UI V2 Menu Rebuild

Status: active; v2 XR panel texture and draw-list cache foundations landed
2026-07-02.

## Decision

Do not refactor the current menu system into shape.

Build a new retained UI path beside the legacy `GameUi`, migrate screens into it
one at a time, then delete the legacy menu implementation once the player-facing
surfaces have moved.

The old path can stay as a fallback during migration, but new work should not
add menu features to it unless the feature is an urgent unblocker. Coordinate
bugs, stale-state bugs, and XR repaint costs should be fixed by moving screens
to UI v2, not by adding more special cases to the legacy helpers.

Workstream: native Rust, shared implementation, desktop validation first.
`mclone-ui` owns UI behavior/state/layout/hit testing. `mclone-render` owns GPU
execution, glyph atlases, batching, and panel textures. App and XR crates own
platform event conversion and presentation only.

## Why

The current UI path has the wrong architecture:

- screens are mostly render helper functions, not retained state
- layout rectangles are recomputed during render and again during hit testing
- render and input can use different state snapshots
- conditional Options rows can differ between the last drawn frame and the click
  path
- text is exploded into many tiny rectangle commands
- XR world-space panels repaint unchanged menu content every frame
- diagnosing a click bug requires chasing coordinate conversion, layout helper
  state, render state, hit-test state, and event ordering separately

The macOS desktop click-position bug is a symptom of this shape. A global
titlebar or pointer offset is not the right first fix. The new UI path must make
it structurally difficult for visual rectangles and hit rectangles to diverge.

## Keep From Legacy

Reuse the parts that are already good boundaries:

- `GameUiAction`-style command output
- app-level event forwarding from desktop/web/Android/XR into shared input facts
- existing flat and XR presentation paths
- current `GuiDrawList` renderer as a temporary backend
- current screens as behavior references and fallback during migration
- current headless screenshot/offscreen validation machinery

## Throw Away

Do not carry these legacy properties forward:

- recomputing screen geometry independently for render and click handling
- passing ad hoc render state into every event handler
- screen-specific `widget_at(...)` chains as the main hit-test model
- using text rasterization as per-pixel rectangle commands in production
- treating XR panel content as something to repaint every frame
- mixing platform quirks into shared UI behavior

## Target Shape

### Single Frame State

Every rendered UI frame uses one immutable `UiFrameState` snapshot. Layout,
paint, and hit testing for that frame refer to the same snapshot.

The app constructs the snapshot once per UI update from platform/runtime facts:

- viewport and GUI scale
- active screen id
- render options
- movement/input preferences
- player model/visibility toggles
- session/server cadence state
- block palette facts
- loading/progress facts

Input events use the last committed layout plus the current frame state revision.
If a state change happens during an input event, it produces a new UI action and
the next frame lays out the new state.

### Retained Surface

`UiSurface` is the top-level retained object:

- stable surface id
- logical size and scale
- active screen object
- last committed `UiFrameState` revision
- layout tree
- widget registry
- paint list
- dirty flags
- optional panel texture revision

### Layout Tree

Layout produces a durable tree or flat registry:

- `WidgetId`
- widget kind
- bounds
- enabled/visible flags
- focusability
- semantic action or callback key
- optional text/value metadata
- parent/layer info

Rendering and hit testing both consume this committed layout. A widget cannot be
drawn in one rectangle and clicked in another unless the debug checks detect a
bad paint op.

### Paint

UI v2 can initially adapt back to `GuiDrawList` so the first slices stay small.
The long-term paint model is semantic:

- rect
- gradient
- border
- text run
- sprite/blit
- item icon
- clip start/end

Text should move to atlas-backed glyph quads after the first retained screen is
working. The v2 model should not depend on per-pixel text commands.

### Input

Input flow:

```text
platform event
  -> shared pointer/key/text event
  -> coordinate conversion to GUI point
  -> UiSurface hit-test against committed layout
  -> widget event state update
  -> optional GameUiAction
```

Pointer movement may update hover state. Pointer down captures the widget id.
Pointer up activates only when it releases on the captured widget, unless a
specific widget such as a slider has drag semantics.

## Implementation Slices

### Slice A: UI V2 Core Skeleton

Status: landed 2026-07-01.

Add a new v2 module in `mclone-ui` without changing player behavior yet.

Minimum types:

- `UiFrameState`
- `UiSurface`
- `UiScreenId`
- `UiWidgetId`
- `UiWidgetKind`
- `UiWidget`
- `UiLayout`
- `UiPaintList` or a v2-to-`GuiDrawList` adapter
- pointer/key event types if existing ones are not clean enough
- hit-test and captured-widget state

Requirements:

- one code path creates widget rects
- render and hit testing consume the same committed layout
- debug formatting can print active screen, GUI scale, pointer point, hovered
  widget, captured widget, and widget rects
- no app crate owns layout or hit-test policy

Validation:

- unit tests for layout registry insertion, hit-test priority, disabled widgets,
  pointer down/up capture, and no-op pointer movement
- `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- `pnpm native:web:build`
- `git diff --check`

### Slice B: Pause Screen In V2

Status: first pass landed 2026-07-01.

Implement the pause screen in UI v2:

- Back To Game
- Options
- Quit To Title

Use the existing `GameUiAction` outputs. Render through the temporary
v2-to-`GuiDrawList` adapter. Keep the legacy pause screen available behind a
small internal switch until screenshots and clicks are proven.

Requirements:

- button rectangles are stored in the committed layout
- hover/pressed visuals read from v2 interaction state
- hit testing uses the committed layout, not recomputed pause button helpers
- visual debug overlay can draw pointer position, hovered rect, and captured
  rect when enabled

Validation:

- unit tests for all three pause actions
- native screenshot of pause screen saved to `/tmp`
- manual desktop click check on macOS
- `pnpm native:web:build`
- `pnpm native:web:smoke`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- Android XR compile check if the local Android target/toolchain is available:
  `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`

### Slice C: Desktop App Routing For V2

Status: first pass landed for Pause, Options, and Help 2026-07-01.

Wire desktop flat input to UI v2 when the active screen has migrated. Legacy
screens continue to route through legacy `GameUi`.

Requirements:

- one GUI point conversion result is passed to UI v2
- v2 debug logs include raw cursor, window inner size, surface size, GUI scale,
  GUI point, hovered widget, captured widget, and action
- no global macOS coordinate offset
- no titlebar/chrome compensation unless diagnostics prove a platform bug below
  the UI layer

Validation:

- native macOS pause screen click check
- v2 logs prove pointer point and button rect agree
- legacy screens still work as before
- web/WASM smoke still reaches a rendered frame with the legacy/v2 UI split
- XR compile/tests still pass with the legacy/v2 UI split

### Slice D: Options Screen In V2

Status: first pass landed 2026-07-01.

Rebuild Options on top of retained row models.

Rows:

- Section Occlusion
- Force Fullbright
- Far LOD
- Far LOD Range
- Player Box
- First Person Body
- Crosshair, when supported
- Player Model
- Movement
- Frame Pacing
- FPS Cap
- Render Distance
- Fly Speed
- Movement Speed
- Touch Controls, when applicable
- Touch Look, when applicable
- Controls
- Server Settings, when applicable
- Done/Back

Requirements:

- one `UiFrameState` controls both rendered rows and hit rows
- conditional rows are included/excluded once during layout
- sliders support capture and drag through retained widget state
- row rects can be dumped in debug mode
- no use of legacy `option_widgets()` for v2 hit testing

Validation:

- unit tests for row inclusion/exclusion and action mapping
- tests for slider value mapping from stored rects
- native screenshots of Options with the relevant row combinations
- manual macOS click check for First Person Body, Crosshair, Server Settings,
  Controls, and sliders

### Slice E: Help/Controls Screen In V2

Status: first pass landed 2026-07-01.

Rebuild the controls/help screen as retained content.

Requirements:

- shortcut rows are computed when the screen/state changes, not every render
- scrolling or pagination, if needed, is part of retained state
- text-heavy output is ready for atlas text once Slice F lands

Validation:

- screenshot of Controls
- command/paint count comparison against legacy Controls
- click Back reliably from the visible button rect

### Slice F: Atlas-Backed Text For V2

Status: first pass landed 2026-07-02.

Move v2 text rendering to glyph atlas quads.

Requirements:

- `mclone-ui` stores text runs and measurement requests
- `mclone-render` owns glyph lookup, glyph atlas pages, UVs, and text meshes
- shadow text reuses the same glyph run with offset/color
- legacy per-pixel text can remain only for old screens during migration

Validation:

- text measurement tests for migrated screens
- screenshots for Pause, Options, and Controls
- command/vertex count comparison on Controls

### Slice G: XR Panel Texture Cache For V2

Status: complete. Automated cache foundations and attached-headset Controls
validation landed 2026-07-02.

Tie world-space UI panel repainting to v2 surface content revisions.

Requirements:

- head pose changes do not dirty content
- controller pose/ray changes do not dirty content
- hover changes dirty only the relevant UI layer
- unchanged v2 panels skip both `GuiDrawList` rebuild and panel-texture repaint
- panel composite and panel repaint are separately reported

Validation:

- XR summary shows panel composite every visible frame but panel repaint only on
  content changes
- XR summary shows draw-list rebuilds only when the v2 panel revision changes
- Controls idle in XR does not repaint every frame
- headset visual validation before marking complete

### Slice H: HUD, Hotbar, And Block Picker V2

Status: flat crosshair/hotbar frame, hotbar selection/content retention, status
overlay retention, block picker v2 retained grid, and runtime cache reporting
landed 2026-07-02; selected item text, debug overlays, and touch/gamepad prompts
still pending.

Move in-game UI surfaces to v2 retained layers:

- crosshair
- hotbar background
- selected-slot overlay
- item/block icons
- selected item name fade
- debug/status overlays
- block picker slot grid
- block picker tooltip

Requirements:

- hotbar selection dirties only hotbar dynamic layers
- block picker uses a retained slot grid and visible item window
- pointer movement inside the same slot does not rebuild the grid

Validation:

- screenshots over a live world
- hotbar and block picker hit-test tests
- frame summary showing localized dirty work

### Slice I: Legacy UI Deletion

Status: pending.

Delete the legacy screen helpers once migrated screens have equivalent v2
coverage.

Requirements:

- no player-facing menu routes through legacy `GameUi`
- old `option_widgets()`/`pause_buttons()` style helpers are gone or test-only
- compatibility adapters are removed if no longer used
- docs and tactical status updated

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `pnpm native:web:build`
- screenshots for title/pause/options/controls/HUD/block picker as applicable

## Debug Mode

UI v2 should include an inert debug mode from the start. It may be env-driven or
app-command driven, but must be off by default.

Debug output:

- active surface/screen
- frame state revision
- layout revision
- raw cursor
- window inner size
- surface size
- GUI scale
- converted GUI point
- hovered widget id
- captured/pressed widget id
- released widget id
- emitted action
- widget rect dump for the active screen

Debug overlay:

- marker at computed GUI point
- outline hovered widget rect
- outline captured widget rect
- optional labels for widget ids

The overlay is for proving the new system, not for making the old system more
complex.

## Validation Matrix

Core gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
pnpm native:web:build
pnpm native:web:smoke
git diff --check
```

Rendered validation:

- save screenshots to `/tmp`
- inspect screenshots before moving to the next visible slice
- include macOS desktop manual click checks for migrated menus
- include web/WASM smoke for every slice that changes shared UI contracts
- include XR compile/test coverage for every slice that changes shared UI
  contracts
- include headset validation for XR panel-cache or XR-visible v2 routing changes

Performance validation:

- track command/paint/glyph/batch counts for legacy and v2 screens while both
  exist
- make idle-frame cache hits visible once dirty tracking lands
- distinguish panel repaint count from panel composite count

## Landed First Chunk

Date: 2026-07-01.

Scope:

- added `mclone-ui::v2` with retained `UiSurface`, `UiFrameState`,
  `UiLayout`, widget ids/kinds, hit testing, hover/capture state, debug
  snapshots, and a temporary `GuiDrawList` renderer adapter
- implemented the v2 Pause screen with Back To Game, Options, and Quit To Title
  actions
- routed only the migrated Pause screen through v2 in the native flat client;
  all other screens still route through legacy `GameUi`
- added opt-in `MCLONE_UI_V2_HIT_DEBUG=1` diagnostics and overlay for v2 pointer
  point, hovered rect, captured rect, widget dump, and emitted action
- fixed the WASM render-timing path so browser render code does not call
  unsupported `std::time::Instant`
- aligned the browser smoke with the current Rust runtime smoke count and made
  the ordinary canvas assertion check shared-buffer use while leaving
  reuse/drop/fallback pressure to the existing topology stress

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-native-client -p mclone-render
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Rendered checks:

- native Pause screenshot inspected:
  `/tmp/mclone-ui-v2-pause.png`
- browser canvas smoke screenshot inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- XR still uses the existing shared UI panel path; this slice proves compile/test
  compatibility, not v2 XR routing
- Controls, Server Settings, HUD, hotbar, and block picker remain legacy
- v2 text still adapts to the existing rectangle-command font path until the
  atlas text slice lands
- macOS manual click signoff remains pending for migrated Pause/Options screens

## Landed Options Chunk

Date: 2026-07-01.

Scope:

- expanded `UiScreenId` and desktop routing so `GameScreen::Options` uses v2
- added v2 widget kinds for buttons, checkboxes, cycle rows, and sliders
- added a v2-owned retained Options row layout instead of using legacy
  `option_widgets()` for v2 hit testing
- included conditional Crosshair, Touch Controls, Touch Look, and Server
  Settings rows during layout from one `GameUiRenderState` snapshot
- implemented slider capture/drag actions for Render Distance, Far LOD Range,
  Fly Speed, Movement Speed, and Touch Look
- kept rendering on the temporary `GuiDrawList` backend and existing visual
  widget primitives

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Rendered checks:

- native Options screenshot inspected:
  `/tmp/mclone-ui-v2-options.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- macOS manual click signoff for Pause/Options is still pending; this chunk
  proceeded on the assumption that committed v2 rects are sufficient until the
  user can test desktop clicks
- Server Settings still uses the legacy screen after Options opens it
- v2 text still uses the old rectangle-command font path; Options produced 7190
  GUI commands in the inspected screenshot, so atlas text remains important

## Landed Controls/Help Chunk

Date: 2026-07-01.

Scope:

- routed all `GameScreen::Help { parent }` screens through v2
- added retained Help shortcut rows to `UiLayout`
- computed Controls shortcut rows once during v2 layout instead of rebuilding
  the table during render
- kept Back as a committed v2 button rect that emits `CloseHelp(parent)`
- added v2 Help key handling for `Esc` and `F1`
- added a unit command-count comparison against the legacy Controls renderer

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Rendered checks:

- native Controls screenshot inspected:
  `/tmp/mclone-ui-v2-controls.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- v2 Controls still uses the old rectangle-command font path and produced 21061
  GUI commands in the inspected screenshot
- no scrolling/pagination was added because current rows fit the 960x540
  screenshot and existing Minecraft-style GUI scale target
- Server Settings, HUD, hotbar, and block picker remain legacy
- macOS manual click signoff remains pending for migrated Pause/Options/Controls
  screens

## Landed Headless Interaction Harness Chunk

Date: 2026-07-01.

Scope:

- added a driver-level headless UI pointer-click harness that records v2 debug
  snapshots before/down/up, emitted action, and action-application result
- added a state builder for driver-owned UI render state so headless UI clicks
  use the same Options facts as rendering
- exposed UI pointer clicks as an offscreen script step for future GPU/headless
  interaction captures
- added regression tests that click Options checkbox rows at multiple in-row
  points, including label area, right edge, and bottom-right edge
- verified that headless v2 clicks toggle Crosshair and First Person Body
  through `FlatClientDriver`, not only through pure `UiSurface`

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-native-client headless_ui_click -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client offscreen_script -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-options-headless.png --width 960 --height 540 --screenshot-ui options-pause --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native Options screenshot inspected:
  `/tmp/mclone-ui-v2-options-headless.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- this does not exercise macOS `winit` cursor delivery, Retina/window/surface
  conversion, or focus/titlebar behavior
- server-backed Options rows still need a GPU/offscreen script that starts a
  runtime, opens Options, clicks Server Settings, and renders the next frame
- sliders and cycles still need the same edge-point harness coverage
- atlas text remains the next performance chunk

## Landed Committed UI Frame State Chunk

Date: 2026-07-02.

Scope:

- made `FlatClientDriver` own the last committed `GameUiRenderState`
- committed the UI render state once in the native flat full-frame render path
  before producing the draw list
- changed native desktop pointer down/up/move handling so platform events no
  longer rebuild or pass a separate input-time `GameUiRenderState`
- changed the offscreen UI click harness to commit a frame state before
  applying the click
- added a regression test that proves the previous hidden-row divergence would
  hover `Crosshair`, while the native driver still clicks `First Person Body`
  from the committed state

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-native-client committed_ui_state -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client headless_ui_click -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-options-committed-state.png --width 960 --height 540 --screenshot-ui options-pause --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native Options screenshot inspected:
  `/tmp/mclone-ui-v2-options-committed-state.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- this commits the invariant in the native flat driver and offscreen harness;
  web/WASM and XR still have direct legacy `GameUiRenderState` builders and need
  the shared host wrapper before the invariant is platform-wide
- `UiSurface` still accepts `GameUiRenderState` on render and pointer calls as a
  compatibility adapter; the next shared-host chunk should hide that from all
  app/platform crates
- atlas text remains the next major performance chunk after the committed-state
  host is shared across web and XR

## Landed Shared Committed UI Host Chunk

Date: 2026-07-02.

Scope:

- added `GameUiHost` in `mclone-ui` as the shared owner of legacy `GameUi`, v2
  `UiSurface`, and the last committed `GameUiRenderState`
- moved committed-state pointer/key/render routing out of the native flat driver
  and into the shared host
- routed desktop flat, web/WASM, flat Android, desktop XR, and Android XR through
  the shared host for migrated v2 screens while preserving legacy fallback
  screens
- changed web, flat Android, and XR pointer input so app/platform crates pass
  converted pointer points only, not freshly rebuilt layout/render state
- added a shared-host regression test proving a divergent input-time state would
  hit `Crosshair`, while committed host input still activates `First Person
  Body`

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui game_ui_host_pointer -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client committed_ui_state -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-options-shared-host.png --width 960 --height 540 --screenshot-ui options-pause --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native Options screenshot inspected:
  `/tmp/mclone-ui-v2-options-shared-host.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- legacy screens still render and hit-test through legacy `GameUi` inside
  `GameUiHost`; this chunk removes app-local state divergence, not legacy screen
  implementations
- `UiSurface` still exposes state-taking render/pointer APIs for tests and
  compatibility; app/platform crates should use `GameUiHost`
- atlas text remains the next major performance chunk

## Landed Atlas-Backed Text Chunk

Date: 2026-07-02.

Scope:

- added semantic `GuiDrawCommand::Text` commands to `mclone-ui` so v2 screens no
  longer explode every glyph pixel into a separate rectangle command
- added atlas text helpers to the existing fixed 5x7 `Font` path while leaving
  the legacy rectangle-font helpers available for non-migrated screens
- added a renderer-owned glyph atlas, UV lookup, and `GlyphAtlas` prepared draw
  path in `mclone-render`
- routed v2 Pause, Options, and Help/Controls headings, labels, buttons,
  checkboxes, cycle buttons, sliders, and shortcut rows through atlas text
- kept world-space/XR and flat presentation on the shared `GuiDrawList` renderer
  path, so migrated v2 panels inherit the glyph atlas backend

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui font_atlas -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render prepares_text_command -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-pause-atlas-text.png --width 960 --height 540 --screenshot-ui pause --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-options-atlas-text.png --width 960 --height 540 --screenshot-ui options-pause --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-controls-atlas-text.png --width 960 --height 540 --screenshot-ui controls --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native Pause screenshot inspected:
  `/tmp/mclone-ui-v2-pause-atlas-text.png`
- native Options screenshot inspected:
  `/tmp/mclone-ui-v2-options-atlas-text.png`
- native Controls screenshot inspected:
  `/tmp/mclone-ui-v2-controls-atlas-text.png`
- browser canvas smoke regenerated:
  `/tmp/mclone-native-web-canvas.png`

Command counts from inspected native screenshots:

- Pause: 32 GUI commands
- Options: 170 GUI commands, down from 7190 with rectangle-font text
- Controls: 104 GUI commands, down from 21061 with rectangle-font text

Known limits:

- legacy screens still use the old per-pixel rectangle font path
- this is still the built-in fixed 5x7 debug font, not a Minecraft font-provider
  or Unicode font atlas implementation
- glyph atlas text removes command explosion, but it does not yet cache rendered
  v2 panel textures or skip unchanged UI repaints in XR

## Landed XR Panel Cache Foundation Chunk

Date: 2026-07-02.

Scope:

- added `UiPanelRevision` reporting from `UiSurface`/`GameUiHost`, split into
  content and interaction revisions
- changed v2 widget rendering to derive hover/pressed visuals from committed
  hovered/captured widget ids instead of raw pointer coordinates
- kept pointer coordinate changes from dirtying panel interaction revision unless
  the debug overlay is visible
- added opt-in cached world-panel rendering APIs in `WorldGuiRenderer`; existing
  `render_panel*` calls still repaint every time unless a cache revision is
  supplied
- cached XR v2 menu panel textures when startup progress and status overlays are
  hidden, while leaving dynamic overlay frames on the uncached repaint path
- kept panel compositing and controller ray-line rendering every visible frame
- added `WorldGuiPanelRenderStats` counters for repaint, cache hit, texture
  recreation, and composite counts
- surfaced the counters through `XrTerrainFrameSummary`, desktop XR summary
  prints, and Android XR perf settle logs/quiet checks

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui panel_revision -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render panel_cache -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render gui -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-controls-panel-cache-sanity.png --width 960 --height 540 --screenshot-ui controls --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native Controls screenshot inspected:
  `/tmp/mclone-ui-v2-controls-panel-cache-sanity.png`
- browser canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- startup progress and status overlays intentionally bypass the cache until they
  have their own revision inputs
- texture recreation is reported separately from panel repaint; panel repaint is
  a render pass into the cached texture, not a CPU texture upload
- actual headset visual validation for idle Controls remains pending

## Landed XR Draw-List Cache Proof Chunk

Date: 2026-07-02.

Scope:

- added a `GameUiHost` v2 panel draw-list cache keyed by `UiPanelRevision`
- kept the cache opt-in through `render_v2_panel_draw_list`; existing flat/web
  callers of `render_draw_list` keep their current behavior
- added `UiDrawCacheStats` counters for draw-list rebuilds and cache hits
- routed XR menu panel draw preparation through the cached v2 path when startup
  progress and status overlays are hidden
- surfaced draw-list cache counters through `XrTerrainFrameSummary`, desktop XR
  summary prints, and Android XR perf settle logs/quiet checks
- kept transient startup/status overlay composition explicitly uncached

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui v2_panel_draw -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene xr_menu_panel_draw -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-controls-draw-cache-sanity.png --width 960 --height 540 --screenshot-ui controls --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Automated proof coverage:

- `game_ui_host_v2_panel_draw_cache_tracks_panel_revision` proves same-revision
  v2 panels reuse draw commands, hover changes rebuild, and pointer jitter inside
  the same widget remains a cache hit
- `xr_menu_panel_draw_cache_hits_until_panel_revision_changes` proves the XR
  Controls panel rebuilds on first draw, hits cache on the second unchanged draw,
  rebuilds on hover change, and hits again for controller-pose-only jitter
- `xr_menu_panel_draw_bypasses_revision_cache_for_transient_overlays` proves
  startup/status-style transient overlays stay off the revision cache path

Rendered checks:

- native Controls screenshot inspected:
  `/tmp/mclone-ui-v2-controls-draw-cache-sanity.png`
- browser canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

## Landed HUD Retained-Layer Foundation Chunk

Date: 2026-07-02.

Scope:

- added a host-owned retained HUD layer in `GameUiHost`
- cached crosshair, flat hotbar slot panel geometry, and selected-slot outline
  by a compact retained HUD state
- kept hotbar icons/slot labels, touch/gamepad prompts, status overlays, debug
  overlays, and the block picker on the existing immediate path
- routed desktop flat, web/WASM, and flat Android HUD composition through the
  shared host-owned HUD path
- kept `render_flat_hud` as a compatibility wrapper for tests and non-host
  callers

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Automated proof coverage:

- `game_ui_host_flat_hud_retained_cache_tracks_static_geometry` proves the HUD
  retained layer rebuilds on first render, hits on unchanged frames, remains a
  hit when only hotbar icons change, and rebuilds when the selected slot changes
- `game_ui_host_flat_hud_draw_matches_standalone_renderer` proves the host-owned
  retained/dynamic split produces the same draw list as the standalone HUD
  renderer

Rendered checks:

- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- the native offscreen screenshot lane currently sets `hud: None`, so native
  live-world HUD pixel capture needs a separate diagnostic option
- block picker and block-picker hit testing are still legacy

## Landed HUD Runtime Cache Counters Chunk

Date: 2026-07-02.

Scope:

- added flat HUD retained-cache stats to shared full-frame render summaries
- reported the HUD retained rebuild/cache-hit counts through desktop headless
  screenshot reports, web/WASM render reports, and flat Android first-frame logs
- kept the counters shared-owner first by routing them through
  `FullFrameRenderSummary` instead of app-local UI state
- left default native offscreen screenshots compatible with `hud: None`, where
  the HUD counters are expected to stay zero

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Runtime proof:

- browser smoke report exposed `flatHudRetainedRebuilds` and
  `flatHudRetainedCacheHits`; the live-world smoke observed a cached HUD frame
  with `flatHudRetainedRebuilds=0` and `flatHudRetainedCacheHits=1`
- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- native desktop live-world HUD pixel capture still needs a dedicated diagnostic
  path because the current offscreen screenshot command does not attach HUD
  state
- XR was compile-checked for this counter plumbing, but the real headset idle
  Controls smoke remains the acceptance check for XR panel repaint behavior

## Landed HUD Hotbar Retained-Layer Chunk

Date: 2026-07-02.

Scope:

- split the flat HUD cache into a static crosshair/hotbar-frame layer and a
  retained hotbar selection/content layer
- removed selected slot from the static HUD cache key, so slot changes no longer
  rebuild crosshair or hotbar-frame commands
- keyed the hotbar layer by GUI scale, selected slot, and the nine hotbar icon
  UVs; icon or selected-slot changes rebuild only that layer
- kept `render_flat_hud` and `GameUiHost::render_flat_hud_draw_list` visually
  aligned by composing the same static, hotbar, and transient layers

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hotbar -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Runtime proof:

- `game_ui_host_flat_hud_retained_cache_tracks_static_geometry` now proves a
  visible flat hotbar has two retained layers: first render rebuilds both,
  unchanged frames hit both, and icon or selected-slot changes produce one hit
  plus one rebuild
- `game_ui_host_flat_hud_draw_matches_standalone_renderer` proves the cached host
  composition still matches the standalone HUD renderer
- browser live-world smoke observed `flatHudRetainedRebuilds=0` and
  `flatHudRetainedCacheHits=2` on quiet frames

Rendered checks:

- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- touch-specific hotbar controls, gamepad prompts, status/debug overlays, and
  selected item name fade are still transient immediate layers
- native desktop live-world HUD pixel capture still needs a dedicated diagnostic
  path because the current offscreen screenshot command does not attach HUD
  state

## Landed Block Picker V2 Retained-Grid Chunk

Date: 2026-07-02.

Scope:

- moved `GameScreen::BlockPalette` into `UiScreenId`, so player-facing block
  picker rendering and hit testing now route through `GameUiHost`'s v2 surface
- added committed v2 slot widgets for occupied block-palette entries, with
  `AssignHotbarBlock` actions derived from the committed render state
- added a retained block-palette grid draw-list cache keyed by GUI scale,
  selected hotbar slot, and palette entry contents
- kept hover, press, and tooltip visuals in a transient layer so pointer
  movement does not rebuild the grid
- anchored the v2 tooltip to the committed slot rect instead of the raw pointer,
  preserving cache stability for pointer jitter inside one slot

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui block_palette -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui game_ui_host_block_palette_uses_v2_panel_cache -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-block-palette.png --width 960 --height 540 --screenshot-ui block-palette --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Automated proof coverage:

- `block_palette_layout_uses_committed_slots_for_actions` proves v2 slot hit
  testing uses committed rects and emits the expected `AssignHotbarBlock`
  action
- `block_palette_grid_cache_survives_hover_and_pointer_jitter` proves the grid
  cache rebuilds on first render, hits on unchanged frames, survives hover and
  same-slot pointer jitter, and rebuilds when palette entry contents change
- `game_ui_host_block_palette_uses_v2_panel_cache` proves `GameUiHost` now treats
  BlockPalette as a v2 panel, including top-level cache hits for pointer jitter

Rendered checks:

- native block-palette screenshot generated and inspected:
  `/tmp/mclone-ui-v2-block-palette.png`
- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- block picker visual content is retained as GUI commands, not yet as a GPU-side
  texture/atlas independent of the normal GUI renderer
- touch-specific hotbar controls, gamepad prompts, status/debug overlays, and
  selected item name fade are still transient immediate layers

## Landed Block Picker Legacy Route Removal Chunk

Date: 2026-07-02.

Scope:

- removed the old `GameUi` BlockPalette render path
- removed the old `GameUi` BlockPalette hit-test/widget-id/action path
- removed legacy block-picker tests that exercised the deleted route
- kept `GameScreen::BlockPalette` and `GameUiAction::OpenBlockPalette` as routing
  state because `GameUiHost` still uses the legacy screen field to select the v2
  surface
- kept shared block-palette geometry and rendering helpers used by the v2
  surface

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-ui block_palette -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-ui-v2-block-palette-after-legacy-removal.png --width 960 --height 540 --screenshot-ui block-palette --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true
```

Rendered checks:

- native block-palette screenshot regenerated and inspected:
  `/tmp/mclone-ui-v2-block-palette-after-legacy-removal.png`
- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- block picker visual content is retained as GUI commands, not yet as a GPU-side
  texture/atlas independent of the normal GUI renderer
- touch-specific hotbar controls, gamepad prompts, debug overlays, and selected
  item name fade are still transient immediate layers

## Landed HUD Status Retained-Layer Chunk

Date: 2026-07-02.

Scope:

- split the flat HUD status overlay out of the transient HUD pass
- added an optional retained status layer in `GameUiHost`, keyed by GUI scale,
  visible message, and ok/error style
- hidden or empty status clears the cached status layer and contributes no cache
  counters, preserving the existing quiet-frame count when status is hidden
- kept `render_flat_hud` and `GameUiHost::render_flat_hud_draw_list` visually
  aligned by composing the same retained, hotbar, status, and transient layers

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Automated proof coverage:

- `game_ui_host_flat_hud_status_cache_rebuilds_independently` proves a visible
  status adds a third retained HUD layer, quiet frames hit all three layers,
  status message changes rebuild only the status layer, and hiding/restoring
  status does not dirty crosshair or hotbar layers
- `game_ui_host_flat_hud_draw_matches_standalone_renderer` continues to prove
  retained host composition matches the standalone HUD renderer with status
  visible
- browser live-world smoke still observed hidden-status quiet frames with
  `flatHudRetainedRebuilds=0`, `flatHudRetainedCacheHits=2`, and
  `statusOverlayVisible=false`

Rendered checks:

- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- touch-specific hotbar controls, gamepad prompts, debug overlays, and selected
  item name fade are still transient immediate layers

## Landed Native Headless HUD Screenshot Opt-In Chunk

Date: 2026-07-02.

Scope:

- added `--screenshot-hud true|false` to native full-frame screenshots
- kept the default `false` so clean world/menu captures remain stable
- when enabled, native headless screenshots build the same shared flat HUD shape
  as live desktop: keyboard/mouse input resolution, crosshair visibility,
  hotbar icons from the mesh catalog, block-palette world-HUD exception, and
  session status overlay
- routed the resulting `FlatHud` through `FlatClientUiFrame`, so headless HUD
  screenshots exercise the same retained HUD layers and cache counters as live
  desktop/web/Android

Validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-native-client cli_parses_full_frame_screenshot_options -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui flat_hud -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-render -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm native:web:build
pnpm native:web:smoke
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-headless-hud-off.png --width 960 --height 540 --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true --screenshot-hud false
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-headless-hud-on.png --width 960 --height 540 --startup-wait frames:1 --render-distance 2 --lighting false --fullbright true --screenshot-hud true
```

Rendered checks:

- native HUD-off screenshot regenerated and inspected:
  `/tmp/mclone-headless-hud-off.png`
- native HUD-on screenshot regenerated and inspected:
  `/tmp/mclone-headless-hud-on.png`
- browser live-world canvas smoke regenerated and inspected:
  `/tmp/mclone-native-web-canvas.png`

Known limits:

- `--screenshot-hud true` currently uses keyboard/mouse flat input resolution;
  touch/gamepad prompt screenshot modes should become explicit options when
  those retained layers land
- touch-specific hotbar controls, gamepad prompts, debug overlays, and selected
  item name fade are still transient immediate layers

## Non-Goals

- Spending more effort on legacy Options hit-test fixes than needed to keep the
  app usable during migration.
- A global macOS coordinate offset.
- A general DOM/flexbox-style layout engine.
- Adopting `egui` or another broad immediate-mode GUI library for player-facing
  UI.
- A visual redesign.
- A complete inventory/item/crafting system.
- XR-only menu behavior or desktop-only shortcuts.

## Landed XR Headset Controls Validation Chunk

Landed 2026-07-02.

Implementation:

- added a smoke-only `--xr-debug-ui none|pause|controls` selector for desktop
  OpenXR and Android XR validation paths
- kept normal XR behavior unchanged unless the debug selector is explicitly set
- made stationary Android XR perf automation preserve the explicit debug UI panel
  so cache validation can hold Controls open while the camera is stationary
- added Android validation-script passthrough for `--xr-debug-ui`

Validation:

```sh
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene xr_menu_panel_draw -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client xr_debug_ui --features xr -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash ./android-xr/validate-quest-openxr.sh --debug --xr-debug-ui controls --perf-seconds 3 --perf-settled-stationary --wait-seconds 180 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 1 --day-time 6000 --freeze-time --lighting false --fullbright true --perf-summary /tmp/mclone-android-xr-ui-controls-perf-summary-2.txt --log /tmp/mclone-android-xr-ui-controls-logcat-2.txt
```

Attached Quest validation produced quiet Controls frames with cached content and
continued visible compositing:

- `ui_draw_rebuilds=0`
- `ui_draw_cache_hits=2`
- `ui_panel_repaints=0`
- `ui_panel_cache_hits=2`
- `ui_panel_texture_recreates=0`
- `ui_panel_composites=2`

## Next Recommended Chunk

Continue with remaining HUD transient-layer work.

Next scope:

- choose the next Slice H retained layer: selected item name fade,
  touch/gamepad prompt retention, or debug overlay retention

## Completed First Recommended Chunk

Start with Slice A plus the smallest useful part of Slice B:

- add v2 core layout/hit-test types
- implement v2 pause screen
- render it through the existing `GuiDrawList` backend
- add debug overlay/logging for v2 pointer, hovered rect, and captured rect
- route only Pause through v2 on desktop flat first
- prove web/WASM still builds and smokes with the v2 types linked in
- prove XR still compiles/tests against the shared UI boundary even before XR is
  routed to v2

This proves the important invariant before touching the complicated Options
screen: the rectangle that is drawn is the rectangle that is clicked.
