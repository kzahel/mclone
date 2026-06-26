# 090: Flat Android Client Parity

Status: active; first shared menu/touch slice landed in code. Parent status matrix:
[`../topics/platform-parity.md`](../topics/platform-parity.md). This tactical
owns the flat-Android follow-up that tactical
[`089-shared-xr-menu-surface.md`](089-shared-xr-menu-surface.md) previously
called out as "flat Android `mclone-ui` adoption."

## Purpose

Move flat Android from an integrated render/runtime smoke shell toward the same
client feature posture as desktop flat and web. The engine/runtime/network/render
foundation is already shared; the remaining gap is the app/player layer:
shared UI, touch input, player movement, HUD/hotbar, and gameplay interaction.

## Current State

- Flat Android is a real `NativeActivity` / `winit` / `wgpu` client using the
  native Rust workspace.
- It composes `NativeSingleViewSceneRuntime<AndroidRemoteServerSession>`, so
  local integrated and remote dedicated TCP host modes already exist.
- It renders real textured terrain, lighting, fluids-as-terrain, sky/daytime,
  mesh assets, and render-section streaming through shared native crates.
- Remote dedicated connect is wired through the Android property
  `debug.mclone.remote_addr`; in-app connect UI is intentionally out of scope
  for the first parity slices.
- The current app/player shell is still thin: touch input still orbits a smoke
  camera when the menu is closed, `mclone-ui` now renders pause/options and a
  touch menu button in code, and gameplay-facing HUD/hotbar, player movement,
  block interaction, and actors are not surfaced.

## Target End State

- Flat Android uses the same `mclone-ui::GameUi` menu model as desktop/web.
- Android-specific code is limited to touch/presentation adapters, not duplicate
  menu/gameplay logic.
- Touch controls drive the shared player movement/camera path rather than a
  standalone orbit camera.
- HUD/hotbar and block interaction converge on the same shared gameplay
  controllers used by desktop/web.
- Local integrated and remote dedicated play stay orthogonal to platform; no UI
  or input work should fork host-mode behavior.

## Implementation Slices

- [x] **Slice 1: shared menu rendering and touch pointer.**
  - Add `GameUi` and `GuiRenderer` to the flat Android renderer.
  - Replace the empty `GuiDrawList` with shared pause/options draw lists.
  - Render the shared touch menu button when the menu is closed.
  - Route touch down/move/up into `GameUi` before orbit-camera handling.
  - Apply scene-local menu actions: resume/open/back, fullbright, section
    occlusion, and render distance.
  - Device visual/touch validation is still pending.
- [ ] **Slice 2: player camera/movement adapter.**
  - Replace the orbit smoke camera with `EngineCameraController` and the shared
    player pose/interest-center path.
  - Keep touch look/move as Android input glue feeding shared movement
    intentions.
- [ ] **Slice 3: Android touch HUD controls.**
  - Reuse `mclone-ui` touch overlay for movement/jump/menu affordances.
  - Keep control state render-only in UI and gameplay intent state in shared
    controller/runtime code.
- [ ] **Slice 4: gameplay interaction parity.**
  - Route block raycast/break/place through the shared interaction controller.
  - Add hotbar/debug palette presentation and touch selection.
- [ ] **Slice 5: device validation and parity matrix update.**
  - Run flat Android validation on device/emulator for local and remote modes.
  - Update platform parity status only after actual device evidence.

## Validation

Baseline code gates for Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
```

Slice 1 validation on 2026-06-26:

- [x] `cargo fmt --manifest-path native/Cargo.toml --all`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android`
- [ ] Device visual/touch smoke of the shared Android menu path.

Device gates after visual slices:

```bash
pnpm native:android:avd-smoke
pnpm native:android:avd-touch-smoke
pnpm native:android:quest-flat -- --remote-addr HOST:PORT
```

## Guardrails

- Do not add an Android-only menu model. Flat Android consumes `mclone-ui`.
- Do not block on text input or in-app server connect. Remote connect can remain
  property/launcher-driven until the shared menu surface is already in place.
- Do not let touch controls own gameplay semantics. Touch converts platform
  events into shared UI/gameplay intentions.
- Keep the existing local/remote host selection path intact while replacing the
  app/player shell.
