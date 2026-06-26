# 089: Shared XR Menu Surface

Status: active; shared XR menu state plus first world-space panel renderer have
landed. Headset visual validation and controller-pointer interaction remain
open.

## Purpose

Give every runtime a shared player-facing menu path without making connect UI
or text input the immediate blocker. XR needs the same `mclone-ui` pause/options
surface as desktop/web first, then a stereo/world-space presentation and
controller input can land behind shared contracts.

## Current State

- Desktop flat and web render the shared `mclone-ui` menu/HUD path.
- Desktop XR and Android XR share `mclone-xr-scene`, but previously passed an
  empty `GuiDrawList` through `app-runtime::frame_render`.
- `GuiRenderer` remains a flat NDC overlay renderer, but
  `mclone-render::gui::WorldGuiRenderer` can now rasterize a `GuiDrawList` to a
  texture and draw it as a 3D quad from a `ChunkRenderView`.
- XR controller actions already expose a left-hand `select_pressed` input. On
  Touch controllers this is bound to left X/Y; on simple controllers it is the
  left select click.
- Flat Android still has an app-local empty UI path and should adopt
  `mclone-ui` separately.

## Target End State

- All client lanes use `mclone-ui` for title/pause/options/status surfaces.
- XR can toggle a menu from a controller button, show it as a readable stereo
  panel a few meters in front of the headset, and interact through a controller
  ray/pointer.
- Raw keyboard/mouse, touch, and XR controller input converge through shared
  menu/pointer/intention contracts instead of app-local UI forks.
- Server address entry, world selection, text input, and loading/progress flows
  build on this surface later. They are not part of the first XR menu slice.

## Implementation Slices

- [x] **Slice 1: shared XR overlay menu bring-up.**
  - `XrMcloneTerrainState` owns `GameUi` and `GuiRenderer`.
  - Left-hand select toggles the pause menu open/closed on edge.
  - Locomotion is gated while the menu is active so the player does not move
    while reading the menu.
  - Both desktop XR and Android XR render the same menu through
    `render_full_frame_for_view`.
  - XR frame summaries include GUI command count and active UI state.
- [x] **Slice 2: first true XR panel presentation.**
  - `WorldGuiRenderer` renders `GuiDrawList` to an offscreen texture and draws
    it as a transparent 3D quad.
  - `mclone-xr-scene` recenters the menu panel in front of the stereo HMD center
    when the menu opens, then reuses the same world pose for both eyes.
  - Initial sizing is 1024x576 pixels, 1.75 blocks wide, and 2.2 blocks in front
    of the HMD.
  - Headset visual readability and comfort validation are still pending.
- [ ] **Slice 3: XR pointer and menu actions.**
  - Raycast controller aim against the panel.
  - Map trigger press/release to shared pointer down/up.
  - Apply menu actions in the shared XR scene where they are scene-local
    (`Resume`, render-distance, fullbright, section occlusion), and surface
    app-owned actions explicitly.
- [ ] **Slice 4: flat Android `mclone-ui` adoption.**
  - Replace the empty GUI draw list with the shared menu path.
  - Route touch menu/pointer input through the same `GameUi` APIs used by
    desktop/web.
- [ ] **Slice 5: shared input-intent contract.**
  - Factor raw input to menu/pointer/gameplay intentions across keyboard/mouse,
    touch, and XR controllers.
  - Keep platform glue in app crates and shared gameplay/menu semantics in
    shared crates.

## Validation

Baseline code gates for Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
```

Slice 1 validation on 2026-06-26:

- [x] `cargo fmt --manifest-path native/Cargo.toml --all`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
- [ ] Headset visual smoke of the menu toggle path. Superseded by the Slice 2
      world-panel validation item for current behavior.

Slice 2 validation on 2026-06-26:

- [x] `cargo fmt --manifest-path native/Cargo.toml --all`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-xr-scene`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
- [x] `pnpm native:web:build`
- [ ] Headset visual smoke of the world-space panel.

Device gates for Slices 2-3:

```bash
pnpm native:xr:run
pnpm native:android-xr:validate -- --debug --skip-build --view-pose 0,120,-96,180
```

## Guardrails

- Do not build a second XR-only menu model. XR consumes `mclone-ui`; only the
  presentation and raw-input mapping are XR-specific.
- Do not block this work on server address entry or `EditBox`. CLI, launch argv,
  Android properties, and web query params are acceptable connect surfaces until
  the shared menu surface is interactive.
- Do not treat the first per-eye overlay as the final XR UX. It is a shared
  ownership/rendering milestone; the target is a stereo-readable panel with
  controller pointer interaction.
