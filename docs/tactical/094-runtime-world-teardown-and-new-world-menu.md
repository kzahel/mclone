# 094: Runtime World Teardown and New-World Menu

Status: active; desktop runtime/menu path landed on 2026-06-26, and web, flat
Android, desktop XR, and Android XR New World replacement paths landed via
tactical 095 on 2026-06-27. The desktop app can boot to a no-world Title menu,
open a reroll-only New World screen, present loading/error status while
creating a world, create a fresh local integrated world in-process, and tear
the current world down back to Title. Web, flat Android, and XR can replace the
current scene/session from the shared New World menu while keeping the old
session live on setup failure. Remaining work is manual/device validation of
the interactive flows plus Android no-world/Quit-to-Title parity if mobile UX
needs that exact desktop state.

## Purpose

Let a player start a fresh world from the menu instead of only at process
launch. Today the world is built once at boot from the launch seed and can never
be replaced without restarting the executable. We want to:

1. Make the runtime able to **tear down the current world and bring up a new one
   with a different seed** in-process, reusing the GPU device/surface and
   immutable assets.
2. Add a **New World menu screen** that picks a seed by **reroll only** (no
   manual typing yet) and starts a new world.
3. Keep **auto-start into a world with the default (or `--seed`) seed** as the
   default launch behavior, and add a **boot intent** (CLI) to optionally boot
   straight to the menu with no world instead.

The end direction is "the game can start at a menu and worlds come and go at
runtime." Manual seed entry, world save/load selection, and XR/web parity for
this flow are explicitly deferred.

## Starting State

Before this tactical, the world was a one-shot built before any menu was shown,
and the seed was fixed for the life of the process.

- **Boots straight into gameplay.** The app constructs the in-game UI directly
  with no title gate: `ui: GameUi::new_ingame()`
  (`native/apps/mclone-native-client/src/app.rs:188`). The world runtime is built
  once at window startup: `WindowSceneRuntime::new(&scene)`
  (`native/apps/mclone-native-client/src/app.rs:58`).
- **`StartWorld` is a no-op.** In `apply_ui_action`, `GameUiAction::StartWorld`
  (and `Resume`, `BackToTitle`, ...) fall through to `self.ui.apply_action(action)`
  with no world creation — it just dismisses the overlay
  (`native/apps/mclone-native-client/src/app.rs:369-376`). The Title screen's
  "Start Local World" therefore only reveals the already-running world.
- **Runtime has no teardown/rebuild.** `WindowSceneRuntime` exposes only
  `new(&SceneOptions)` — there is no `reset`/`rebuild`/`set_seed`
  (`native/apps/mclone-native-client/src/scene_runtime.rs:111-123`). It owns the
  scene runtime plus immutable `actor_textures`.
- **Seed is launch-scoped.** `SceneOptions.seed: i64`
  (`native/apps/mclone-native-client/src/cli.rs:21`) is set from `--seed`
  (`cli.rs:448`) or `DEFAULT_SEED` (`cli.rs:189`). It flows
  `local_single_view_options` → `LocalSingleViewSceneOptions::new(scene.seed, ...)`
  (`scene_runtime.rs:80-89`) → `NativeIntegratedServerRunnerConfig::new(seed)` →
  `OverworldBiomeSource::new(seed, ...)` in `mclone-worldgen`.
- **Menu model exists and is extensible.** `mclone-ui` is a screen-state enum
  (`GameScreen { Title, Pause, Options { parent } }`) with `GameUiAction`
  variants and `Button`/`Checkbox`/`Slider`/`CycleButton` widgets
  (`native/crates/mclone-ui/src/lib.rs`). Title buttons are declared in
  `title_buttons(...)` and mapped to actions in `action_for(...)`. There is **no
  text/numeric input widget** and only `Escape` is routed through key handling.
- **Headless UI screenshots are enumerated.** `HeadlessScreenshotUi`
  (`cli.rs:134-142`) has `Title`/`Pause`/`Options*` variants used by
  `--screenshot-ui` for UI visual regression.

## Target End State

- The desktop app has a clear split between a surface/app shell and a droppable
  world session. The shell owns the window, wgpu device/surface, frame pacing,
  UI renderer, audio, and immutable CPU assets. The world session owns the live
  server/client runtime, camera/player pose, interaction state, actor
  interpolation, and render-section cache state.
- The world session can be dropped and recreated for a new local-world seed
  **without** recreating the wgpu device/surface or reloading immutable assets
  (mesh catalog/atlas, actor textures). Prefer reconstruction around retained
  assets over an in-place reset API.
- The app render/update loop tolerates a **no-active-world** state (menu only),
  drawing a menu background instead of polling/rendering a world.
- A **boot intent** decides startup: auto-start into a world with the default or
  `--seed` seed (current behavior, the default) **or** boot to the Title menu
  with no world.
- A **New World screen** reachable from Title lets the player reroll a seed and
  press Create World, which tears down any current world and brings up a new
  **local integrated** world with that seed, then returns to gameplay. "Quit To
  Title" tears the world down.
- Mouse-lock, camera/player pose, chunk interest, day-time, render-section GPU
  cache, and HUD all reset cleanly across world transitions.

## Landed First Pass

2026-06-26:

- Desktop window startup now takes a `WindowStartIntent`: default `InWorld`
  preserves existing launch behavior, and `--menu` / `--start-in-world false`
  boots to Title with no active world.
- `ChunkApp` now owns immutable `WindowSceneAssets` separately from an optional
  `WindowSceneRuntime`, so the window, wgpu surface/device, GUI renderer, audio,
  frame pacing, and asset atlases survive world teardown.
- The render/update loop tolerates `runtime: None`: world polling, actor sync,
  traversal readiness, underwater effects, mouse-lock, and block interaction are
  skipped while the menu still draws.
- `GameScreen::NewWorld` and app-owned actions
  `OpenNewWorld`/`RerollSeed`/`CreateWorld(i64)`/`QuitToTitle` are wired through
  desktop, shared UI, headless UI screenshots, web action labeling, flat Android
  ignores, and shared XR scene ignores.
- `CreateWorld(seed)` always starts a fresh local integrated world, clearing
  any launch `remote_addr`, resetting camera/interactions/interpolation/render
  stats, clearing GPU section uploads, recreating the runtime around retained
  immutable assets, uploading the new sections, then closing the menu only after
  creation succeeds.
- `QuitToTitle` drops the runtime and clears draw sections; `BackToTitle`
  remains non-destructive UI navigation.
- Create World now queues a pending local world start, presents a loading status
  frame first, then does the synchronous rebuild. Failure keeps the New World
  screen open, shows an error status, and leaves mouse-lock off. Initial
  auto-start failure also falls back to Title with an error status instead of
  exiting after the window is live.
- App-level rebuild coverage starts one seed, forces render-section cache
  construction, rebuilds with a second seed through the same app path, and
  asserts the center-chunk signature changes, the new center chunk is loaded,
  launch `remote_addr` is cleared for local Create World, and stale cached render
  sections do not survive the runtime swap.

Open follow-ups for this doc:

- Run a manual interactive desktop smoke for Quit To Title -> New World ->
  Reroll -> Create, including the visible loading status frame.
- Web, flat Android, and XR scene replacement now live in tactical 095; web
  Join Remote has a runtime reconnect path but still needs a connect-screen
  smoke and endpoint editing. Flat Android still needs the desktop-style
  no-world/Quit-to-Title state if that becomes required for mobile UX. XR and
  Android replacement menu flows still need headset/device validation.

## Implementation Slices

- [x] **Slice 1: app shell and world-session state (no-world allowed).**
  - Model world presence explicitly (e.g. `Option<WindowSceneRuntime>` or a small
    `WorldSession` enum) so the update/render loop can run with no world. This
    comes before teardown so boot-to-menu can create a window/surface without
    constructing a world first.
  - When there is no world, skip world poll/sync/draw and clear to a menu
    background; keep UI input, scaling, and frame pacing working.
  - Keep `BackToTitle` as UI navigation only. Add a distinct app-owned
    `QuitToTitle` action for destructive teardown from the pause screen.
- [x] **Slice 2: runtime world teardown/bringup (the unlock), desktop path.**
  - Have the app hold the runtime as droppable world-session state and
    reconstruct it around retained immutable assets. Avoid
    `WindowSceneRuntime::rebuild(&mut self)` unless later evidence shows that
    in-place reset is simpler and equally safe.
  - Enumerate world-coupled vs surface-coupled state in `App`. Preserve the wgpu
    device/queue/surface, frame pacing config, and loaded assets. Reset the
    render-section CPU/GPU cache, camera/player pose, interest center, day-time,
    interaction state, actor interpolation, and pending render/compile queues.
  - Use a loading/error state for world creation. Dropping the native server,
    worldgen, light, or render-compile workers may briefly block while an
    in-flight job finishes, and creation can fail.
  - Validate by building a runtime, tearing it down, and rebuilding with a
    different seed in a unit/integration test: assert the new center chunk loads,
    assert a fixed section/chunk signature changes across chosen test seeds, and
    assert no stale render sections survive.
- [x] **Slice 3: boot intent / CLI.**
  - Add a startup intent to `Cli::Window` (e.g. `start: StartIntent { InWorld |
    Menu }` or `--menu` / `--start-in-world true|false`). Default = auto-start in
    a world with `SceneOptions.seed` (preserves today's behavior and `--seed`).
  - Thread the intent into `run_window`/`App::new` to choose initial UI
    (`new_ingame` vs Title) and whether to build a world at boot.
- [x] **Slice 4: New World menu screen (reroll-only).**
  - Add `GameScreen::NewWorld` and `GameUiAction::CreateWorld(i64)` (plus a reroll
    affordance — either a `RerollSeed` action or screen-local candidate-seed
    state). Repoint Title "Start Local World" to open `NewWorld`.
  - Widgets (reuse `Button`): seed display, Reroll, Create World, Back. No text
    entry. Add a `HeadlessScreenshotUi::NewWorld` + `--screenshot-ui new-world`
    for UI regression, matching the existing convention.
  - Reroll entropy lives in the app layer (not the deterministic worldgen path)
    and should be injectable so headless screenshots stay deterministic.
- [x] **Slice 5: wire menu actions to teardown end-to-end, desktop path.**
  - `CreateWorld(seed)` starts a local integrated world from an explicit local
    start intent, updates the app's current local-world seed, runs the Slice 2
    reconstruction path, and returns to gameplay with mouse-lock armed only after
    creation succeeds. If `--remote-addr` was used for launch, New World still
    creates a local world; remote server world selection is a later tactical.
  - `QuitToTitle` tears the world down (Slice 1 no-world state). `BackToTitle`
    remains non-destructive navigation from title-owned submenus.
  - Verify camera/HUD/mouse-lock/day-time reset correctly across Title → New
    World → Create and back to Title.

## Validation

Baseline code gates run for the landed first pass:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot C:\tmp\mclone-new-world-menu-after-loading.png --screenshot-ui new-world --width 960 --height 540 --seed 12345
```

Per-slice additions:

- Slice 1: no-world render path exists through `--menu`; loading/error status
  renders through the same GUI overlay path.
- Slice 2: rebuild-with-new-seed app test covers changed center-chunk signature,
  loaded new center chunk, cleared remote address, cleared stale render cache,
  and reset render stats.
- Slice 3: CLI parse tests cover default in-world startup and menu startup.
- Slice 4: `--screenshot-ui new-world` PNG was generated and inspected; UI
  hit-test/action unit tests cover Open New World, Reroll, Create World, and
  Back.
- Slice 5: code path is wired with queued loading/error status; manual desktop
  smoke remains a follow-up.

## Guardrails

- **Do not build manual seed entry / a `TextInput` widget / XR virtual keyboard
  in this doc.** Reroll-only. Typed/shareable seeds are a later tactical.
- **Keep auto-start-in-world the default.** Booting to the menu is opt-in via the
  new intent; existing `--seed` and headless/screenshot/perf/XR CLI paths must be
  unchanged.
- **Reuse the wgpu device/surface and immutable assets across world rebuilds.**
  Teardown is a world reset, not a renderer/window reset; do not reload the mesh
  catalog/atlas or actor textures per world.
- **Do not conflate remote host mode with local new-world creation.** In this
  first pass, Create World always means a fresh local integrated world. Remote
  world/session selection belongs behind a later host-mode menu.
- **First pass targets the desktop flat window path.** The shared XR scene only
  applies scene-local actions and ignores app-owned ones like quitting
  (`089-shared-xr-menu-surface.md`); `CreateWorld`/teardown is app-owned, so XR
  and flat Android parity for new-world is a separate follow-up, not a blocker
  here. Web parity for local New World restart moved under the shared session
  coordinator in tactical 095.
- **No world save/load selection here.** This is "discard and regenerate from a
  seed," not named-world management or persistence-directory selection.

## Related

- `027-mclone-ui-foundation.md` — the `mclone-ui` screen/widget model this builds on.
- `075-shared-single-view-runtime-prereq.md`, `084-single-view-platform-alignment.md`
  — shared single-view runtime shell the teardown/bringup lives behind.
- `089-shared-xr-menu-surface.md` — shared menu/pointer path and the app-owned vs
  scene-local action split that gates XR new-world parity.
