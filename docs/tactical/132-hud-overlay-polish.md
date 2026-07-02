# 132: HUD Overlay Polish

Status: active; Slices A-C landed.

Workstream: native Rust, shared `mclone-ui` overlay behavior with desktop
validation first; web/WASM, flat Android, and XR must keep compatible paths.

## Goal

Clean up the remaining non-menu HUD/overlay surfaces after the UI v2 menu
rebuild. The target is not visual redesign. The target is to make loading,
debug, and notification overlays predictable, cacheable, and cheap when they
appear in startup, debug, and XR panel paths.

## Findings From Initial Audit

The menu refactor moved the persistent in-game HUD pieces into retained layers:
crosshair/hotbar frame, hotbar contents/selection, status, touch/gamepad
prompts, and flat debug overlays.

Remaining overlay surfaces are mostly separate from `FlatHud`:

- startup loading progress overlay
- debug view-readiness mini-panel
- XR menu startup/status/progress overlays
- selected item name fade, which is not currently implemented as a real overlay
  model

The highest-value issue is the loading-progress data shape. `LoadingProgressOverlay`
stores cells as a `Vec`, while `status_at(relative_x, relative_z)` linearly
searches that vec. Both `render_loading_progress_overlay` and
`render_loading_progress_panel_at` loop every grid coordinate and call
`status_at`, making rendering O(grid cells * overlay cells). For
view-readiness snapshots, the server can generate one cell for every chunk in
the tracking square, so this becomes quadratic at larger radii.

The second issue is cache ownership. Native flat startup loading and debug
view-readiness are appended through app-level immediate draw calls in
`FlatClientDriver::render_full_frame_with_ui`. XR uses
`prepare_xr_menu_panel_draw`; if startup progress or status is visible, it
bypasses the v2 panel revision cache and rebuilds a combined draw list. Those
paths are understandable, but they are not yet the retained-layer shape we want.

Selected item name fade is not a performance issue yet. There is no durable
notification state or label source for it today. Adding the fade should start
with the gameplay/client event contract, not with a renderer-only timer.

## Non-Goals

- Do not reopen the old menu system.
- Do not add selected-item fade as a hard-coded string or local app timer.
- Do not make loading overlays block UI v2 menu caching unless they actually
  cover the menu panel.
- Do not change gameplay loading semantics or chunk readiness rules.

## Desired Shape

Loading and notification overlays should be shared data models in `mclone-ui`
or a nearby shared crate. Apps should supply state and presentation placement;
they should not hand-roll overlay drawing in platform-specific render loops.

`GameUiHost` can remain the owner of retained overlay draw lists if the overlay
is screen-space GUI. XR can consume the same draw-list layers and decide whether
they live on the menu panel, a startup panel, or a separate world-GUI panel.

## Slice A: Loading Overlay Data Shape

Make loading progress lookup linear for construction and O(1) or direct for
rendering.

Candidate implementation:

- keep the public `LoadingProgressOverlay` shape stable where practical
- normalize cells into a dense grid keyed by `display_radius`
- make `status_at` O(1), or have renderers iterate the dense grid directly
- preserve playable-cell lookup without scanning on every render
- add tests for sparse startup snapshots and dense view-readiness snapshots

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-ui loading_progress`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime loading_progress`

Implementation note, 2026-07-02:

- `LoadingProgressOverlay::new` now keeps the incoming cell list for existing
  callers while building a dense row-major status grid keyed by
  `display_radius`.
- `status_at(relative_x, relative_z)` indexes the dense grid and returns
  `None` outside the normalized display radius.
- `playable_cell()` returns the latest playable cell captured during
  construction instead of scanning cells during rendering.
- Focused tests cover duplicate/latest-cell behavior, sparse startup-style
  snapshots, dense view-readiness-style snapshots, panel rendering, and
  app-runtime snapshot mapping.

## Slice B: Retained Loading Layers For Flat Paths

Move native flat loading overlays behind a retained shared layer.

Scope:

- full-screen startup loading overlay
- compact debug view-readiness panel
- host-owned cache key includes GUI scale, overlay state, overlay variant, and
  placement
- unchanged startup/view-readiness overlays hit the cache
- changed percent or cell state rebuilds only the loading layer, not menu/HUD
  layers

Validation:

- unit tests proving first render rebuilds, quiet frame hits, and cell/percent
  changes rebuild
- native screenshot with `--screenshot-debug-pane true` still shows the
  view-readiness mini-panel
- startup screenshot still shows loading overlay when a startup pump is active

Implementation note, 2026-07-02:

- `GameUiHost` now owns retained `LoadingProgressOverlayLayer` draw caches for
  fullscreen startup overlays and compact panel overlays.
- Flat desktop/offscreen composition appends startup progress and debug
  view-readiness through the shared retained host path instead of drawing them
  directly in `FlatClientDriver`.
- Cache tests cover first-render rebuilds, quiet-frame hits, percent/cell
  invalidation, placement invalidation, fullscreen/panel cache separation, and
  independence from v2 menu and flat HUD caches.
- Offscreen `--startup-wait none` / `frames:N` now mirror window-mode
  nonblocking startup closely enough for validation captures: they start the
  local startup pump but do not drain it to playable before screenshot frames.

## Slice C: XR Panel Overlay Split

Stop treating XR startup/status/progress as a reason to throw away the whole
menu panel cache.

Scope:

- keep v2 menu panel draw caching keyed by panel revision
- compose retained status/progress overlay layers over the cached panel draw
- keep the existing test coverage that transient startup/status overlays do not
  corrupt the panel revision cache, but update the expectation from "bypass the
  cache" to "compose separate retained overlay layers"
- preserve multiview/per-eye behavior; these are GUI draw-list layers, not new
  world renderers

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- desktop XR debug UI smoke with startup/status panel visible

Implementation note, 2026-07-02:

- XR menu panel rendering now always prepares the cached v2 menu panel draw
  first, keyed by its normal `UiPanelRevision`.
- Startup progress and status overlays are prepared as a separate retained draw
  layer with its own progress/status cache key and independent
  `UiPanelRevision`.
- XR scene rendering uses a second `WorldGuiRenderer` for the overlay panel so
  the transient overlay texture cache does not invalidate the base menu panel
  texture cache in per-eye or multiview rendering.
- Controller ray lines are rendered once on the topmost active panel layer, so
  overlay composition preserves the previous visual ordering.
- Unit coverage now asserts composition over a cached base panel for both
  status-message changes and startup-progress changes.
- Local desktop XR smoke was attempted with `--xr-mclone-smoke --xr-debug-ui
  pause`, but this machine has no OpenXR loader configured.

## Slice D: Selected Item Notification Model

Only do this if we actually want the selected item name fade now.

First define the state source:

- event source: hotbar slot changes and block assignment changes
- label source: current debug block palette/catalog labels, later inventory item
  display names
- ownership: shared client/UI state, not a desktop-only timer
- output: `SelectedItemNotification { label, shown_at, duration, alpha }` or an
  equivalent deterministic frame-state model

Then render it as a retained/dynamic HUD layer:

- layout is cached by GUI scale and label
- alpha/time changes should not rebuild static text geometry once text atlas
  support can separate color/opacity cheaply; until then, document rebuild cost
- route desktop, web, and Android flat through the same model

## Validation Matrix

Required for any implementation slice:

- `cargo fmt --manifest-path native/Cargo.toml --all`
- `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`

Rendered validation when pixels are affected:

- native loading/debug screenshot under `/tmp`
- web smoke screenshot if flat HUD composition changes
- XR debug panel smoke if `prepare_xr_menu_panel_draw` changes

## First Recommended Chunk

Start with Slice A.

Reason: it fixes the only clearly bad algorithmic shape found in the audit and
is useful whether or not we later retain the loading overlay draw lists. It is
also small enough to validate with focused unit tests before touching desktop,
web, or XR composition.
