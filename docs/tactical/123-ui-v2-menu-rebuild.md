# 123: UI V2 Menu Rebuild

Status: active; Options v2 first pass landed 2026-07-01.

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

Status: first pass landed for Pause and Options 2026-07-01.

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

Status: pending.

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

Status: pending.

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

Status: pending.

Tie world-space UI panel repainting to v2 surface content revisions.

Requirements:

- head pose changes do not dirty content
- controller pose/ray changes do not dirty content
- hover changes dirty only the relevant UI layer
- panel composite and panel repaint are separately reported

Validation:

- XR summary shows panel composite every visible frame but panel repaint only on
  content changes
- Controls idle in XR does not repaint every frame
- headset visual validation before marking complete

### Slice H: HUD, Hotbar, And Block Picker V2

Status: pending.

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
- Controls still uses the legacy help/controls screen
- Server Settings still uses the legacy screen after Options opens it
- v2 text still uses the old rectangle-command font path; Options produced 7190
  GUI commands in the inspected screenshot, so atlas text remains important

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

## Next Recommended Chunk

Move Controls/Help into v2 as Slice E:

- build retained shortcut rows once when the screen/state changes
- route `GameScreen::Help { parent: OptionsTitle | OptionsPause | Pause }`
  through v2 for the migrated menu paths
- keep Back as a committed v2 button rect
- add a command-count comparison against legacy Controls, because this is the
  text-heavy screen that originally exposed the worst menu frame cost
- decide whether basic scrolling/pagination is needed before atlas text lands

This should be done before XR panel caching because Controls is the most
text-heavy migrated menu and gives the atlas/caching work a concrete target.

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
