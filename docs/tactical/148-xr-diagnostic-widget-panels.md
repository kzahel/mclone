# 148: XR Diagnostic Widget Panels

Status: Slice 1 landed 2026-07-06; later slices proposed.

Workstream: native Rust shared UI, diagnostics, and XR presentation. This is a
successor follow-up to tactical
[`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md)
Slice 7, and it should coordinate with the organization cleanup tracked in
[`145-native-rust-organization-refactor.md`](145-native-rust-organization-refactor.md).

## Purpose

Make diagnostics visible in XR without turning desktop-only HUD overlays into
the permanent XR UI model.

Tactical 144 Slice 7 added the shared `Frame Metrics` menu action and a
desktop flat/offscreen frame-pipeline overlay. XR currently projects the menu
action as unsupported, even though Android XR already builds and logs
`FramePipelineReport` data in the perf path. The missing pieces are presentation
and report plumbing:

- get the latest `FramePipelineReport` into `mclone-xr-scene`;
- render the existing shared UI overlay as an XR-safe passive world panel;
- enable the `frame_pipeline_overlay` capability for XR once both per-eye and
  multiview render paths are covered.

This tactical also sets the longer-term direction for debug UI parity. New
diagnostic surfaces should be shared widgets with platform-specific presenters,
not hidden desktop keyboard overlays such as "press tilde" or "press F7" paths
that XR cannot see.

## Current State

- `mclone-ui` owns the frame-pipeline overlay draw model:
  `FramePipelineHudOverlay` plus `render_frame_pipeline_overlay`.
- Desktop flat and offscreen attach that model to `FlatHud` and render it as a
  screen-space overlay.
- `mclone-xr-scene` already renders the shared pause/options/block-palette UI
  through `WorldGuiRenderer` as a world-space panel. That path supports both
  per-eye and full-frame multiview.
- XR explicitly disables the frame-pipeline overlay:
  - `current_ui_render_state` forces `frame_pipeline_overlay_visible = false`;
  - `SetFramePipelineOverlayVisible(_)` is ignored;
  - `xr_client_experience_profile` marks `frame_pipeline_overlay` unsupported.
- Android XR builds `FramePipelineReport` from the shared diagnostics schema in
  the perf/logging path, but that report is not maintained as live scene state.

## Direction

Use a shared-widget, platform-presenter split:

```text
FramePipelineReport
  -> mclone-ui diagnostic widget model
    -> desktop flat screen-space presenter
    -> XR passive world-panel presenter
```

The first widget is frame metrics. Later widgets can include the tilde/debug
stats surface, render-readiness mini panels, and other diagnostics.

The XR panel is passive by default. It should not receive controller pointer
hits and should not block menu, block-palette, or gameplay interaction. If a
future diagnostic widget needs interaction, use explicit `mclone-ui` widget hit
regions or actions; do not infer interactivity from rendered texture alpha.

## Non-Goals

- Do not add a general "show desktop HUD in XR" product feature in the first
  slice. A flat-HUD mirror may be useful as a developer fallback later, but it
  is not the desired end state.
- Do not create an XR-only frame metrics model. The data and widget stay shared;
  XR owns placement and projection.
- Do not make the diagnostic panel pointer-interactive in the first slice.
- Do not change scheduling, frame pacing, render budgets, or accounting math.
  This tactical only changes report delivery and presentation.
- Do not leave new diagnostic toggles as desktop-only keyboard shortcuts.

## Ownership

Shared crates:

- `mclone-diagnostics`: report schema and accounting math.
- `mclone-ui`: diagnostic widget state, labels, and `GuiDrawList` rendering.
- `mclone-app-runtime::client_experience`: shared action classification,
  capability projection, and settings state for diagnostic visibility.

XR shared crate:

- `mclone-xr-scene`: diagnostic panel state, world-space placement, per-eye and
  multiview rendering, and XR capability enablement once report plumbing exists.

App/platform adapters:

- Desktop OpenXR host and Android XR host own OpenXR frame-loop timing facts and
  platform-only counters.
- Android XR can factor its current perf-report helper so the live overlay and
  log sink use the same `FramePipelineReport` data without duplicating math.

## Module Shape

Start reducing `mclone-xr-scene/src/lib.rs` as this work lands. Prefer small
private modules with stable crate-local boundaries:

- `diagnostic_panel.rs`
  - `XrDiagnosticPanel`
  - visibility state
  - latest frame metrics widget
  - passive world-panel render helpers
  - per-eye and multiview render entry points
- `panel.rs`
  - reusable world-panel placement helpers
  - head-centered menu panel placement
  - left-hand game-UI panel placement
  - off-center diagnostic panel placement
- `menu_panel.rs`
  - existing XR menu draw preparation and overlay cache
  - menu pointer hit/ray helper code
- `profile.rs`
  - `xr_client_experience_profile`
  - capability status decisions
- `diagnostics_report.rs` or host-side equivalent
  - small report bridge types/functions if needed to avoid copying Android XR
    perf helpers into the scene crate.

Do not introduce a large trait hierarchy in the first slice. A concrete frame
metrics panel is enough if the module boundaries make the next widget obvious.

## Placement Policy

Initial XR frame metrics placement should be readable but not central:

- world-space panel, not a tightly head-locked overlay;
- offset roughly 20-30 degrees from center and slightly below or beside the
  primary sight line;
- recenter when toggled on or when explicitly requested, not every frame;
- transparent texture, no opaque full-screen background;
- passive: no controller ray clipping and no pointer hit testing.

The exact dimensions can start near the existing XR menu texture scale if that
keeps text readable, but the panel should be smaller than the main menu and
should not obscure the crosshair/interaction center.

## Guardrails

- Every XR-visible diagnostic renderer must support both per-eye and full-frame
  multiview paths, or document why one path is intentionally unavailable.
- Capability projection must be honest. Enable `frame_pipeline_overlay` for XR
  only after the report stream and XR panel render path are both wired.
- New debug UI actions must route through shared `GameUiAction` /
  `ClientExperienceController` classification. A desktop-only shortcut without
  an XR capability projection is a regression.
- Do not reuse the interactive menu panel renderer state in a way that invalidates
  menu texture caches every frame. Use a dedicated `WorldGuiRenderer` or an
  equivalent separate cache for diagnostic panels.
- Do not use texture alpha as the input hit-test source.

## Slice 1: Frame Metrics Passive XR Panel

Goal: the existing `Frame Metrics` option toggles a headset-visible passive
world panel in desktop XR and Android XR.

Deliverables:

- Factor the XR diagnostic panel into a separate module instead of growing
  `mclone-xr-scene/src/lib.rs`.
- Add `XrDiagnosticPanel` with a dedicated world-GUI renderer/cache.
- Add a scene method to accept the latest `FramePipelineReport` plus revision,
  or a tiny shared bridge that produces `FramePipelineHudOverlay`.
- Render the frame metrics widget in both per-eye and multiview XR paths.
- Apply `SetFramePipelineOverlayVisible(visible)` in `mclone-xr-scene`.
- Reflect the visibility bit in `GameUiRenderState`.
- Enable `frame_pipeline_overlay` in `xr_client_experience_profile` only when
  report input exists for the lane.
- Keep the panel passive: no pointer hits, no controller ray clipping, no
  gameplay interaction blocking.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui frame_pipeline
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Device validation when available:

```bash
pnpm native:xr:check
pnpm native:android-xr:apk
```

Headset smoke expectation: open Options, toggle `Frame Metrics`, close the menu,
and verify the passive frame metrics panel remains readable off-center in the
world. It must not interfere with block-palette pointer interaction or gameplay
ray/use/attack input.

Slice 1 implementation record:

- Added `mclone-xr-scene::diagnostic_panel` with a dedicated
  `WorldGuiRenderer` and cache for the shared `FramePipelineHudOverlay` draw
  model.
- Added `mclone-xr-scene::frame_pipeline_reporter` as the live XR
  host-fed `FramePipelineReport` accumulator. Android XR and desktop XR smoke
  now feed completed-frame timing and the latest `XrTerrainFrameSummary` into
  the scene.
- Rendered the passive frame metrics panel in both per-eye and full-frame
  multiview paths using a single stereo-derived off-center panel pose that is
  recomputed from the headset views so the panel stays sticky relative to the
  headset instead of anchoring in world space.
- Follow-up headset tuning moved the sticky panel to a centered position below
  the eye centerline and increased its physical size for readability.
- Wired the shared `Frame Metrics` menu action in XR:
  `SetFramePipelineOverlayVisible(visible)` now changes XR diagnostic panel
  visibility, `GameUiRenderState` reflects the bit, and the XR profile marks
  the capability supported.
- Kept the first panel passive. It has no pointer hit-test path, does not clip
  controller rays, and does not participate in gameplay or menu interaction.

Validation run:

```bash
cargo fmt
cargo check -p mclone-xr-scene
cargo check -p mclone-native-client --features xr
cargo test -p mclone-ui frame_pipeline
cargo test -p mclone-xr-scene
cargo check -p mclone-android-xr-client
```

Android target validation attempted with:

```bash
cargo check -p mclone-android-xr-client --target aarch64-linux-android
```

That target check was blocked on the macOS host by a missing Android C compiler:
`aarch64-linux-android-clang`. Device/APK validation remains required on an
Android-prepared host.

## Slice 2: Shared Diagnostic Widget Surface

Goal: make frame metrics a first-class shared diagnostic widget instead of a
flat-HUD-owned concept.

Deliverables:

- Introduce a small shared diagnostic widget model in `mclone-ui`, for example:
  - `DiagnosticWidget::FrameMetrics(FramePipelineWidget)`; or
  - a concrete `FrameMetricsWidget` plus a later enum when the second widget
    arrives.
- Keep desktop flat rendering visually equivalent to the tactical 144 overlay.
- Have desktop flat and XR call the same widget renderer, with different
  presenters:
  - desktop: screen-space placement;
  - XR: passive world-panel placement.
- Update naming and docs so `FramePipelineHudOverlay` is no longer treated as
  flat-HUD-only if the rename is worth the churn.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client frame_pipeline
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
```

## Slice 3: Tilde Debug Overlay Convergence

Goal: stop the existing desktop tilde/debug surface from being permanently
desktop-only.

Deliverables:

- Inventory the current desktop tilde/debug overlay data and actions.
- Define the shared diagnostic widget state needed for the useful subset.
- Route the toggle through shared client-experience action classification.
- Present the widget through:
  - desktop screen-space diagnostic presenter;
  - XR passive diagnostic panel presenter.
- Project unsupported or pending diagnostics explicitly in profiles instead of
  silently doing nothing.

Non-goal: do not port every historical debug label if the data source is
desktop-only or obsolete. Prefer a small useful shared subset first.

## Slice 4: Diagnostic Action Tripwires

Goal: prevent new debug UI from landing as desktop-only shortcuts.

Candidate checks:

- unit tests that classify diagnostic `GameUiAction`s through
  `ClientExperienceController`;
- a focused grep tripwire for app-local debug shortcut handling that bypasses
  shared actions;
- documentation in `client-experience-architecture.md` that new visible debug
  diagnostics need flat and XR projections, or an explicit unsupported status.

This slice should be lightweight. The point is to catch the pattern early, not
to build a brittle static-analysis system.

## Open Questions

- Slice 1 chose a smaller always-on live report stream in `mclone-xr-scene`;
  the Android perf probe remains the richer explicit perf/log sink. Revisit
  only if the live panel needs worst-frame history or GPU/compositor panels.
- What exact off-center placement is most comfortable on Quest 3? First device
  validation should record the chosen angle, distance, and panel size.
- Should a developer-only flat-HUD mirror ever exist? Current direction says
  not as a product feature; reconsider only if several diagnostics need a
  short-lived fallback before widget extraction.
