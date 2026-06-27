# 090: Flat Android Client Parity

Status: active; shared menu/touch, player camera/look, touch movement, and
first-class x86_64 AVD validation tooling slices landed in code. Parent status
matrix:
[`../topics/platform-parity.md`](../topics/platform-parity.md). This tactical
owns the flat-Android follow-up that tactical
[`089-shared-xr-menu-surface.md`](089-shared-xr-menu-surface.md) previously
called out as "flat Android `mclone-ui` adoption."

## Purpose

Move flat Android from an integrated render/runtime smoke shell toward the same
client feature posture as desktop flat and web. The engine/runtime/network/render
foundation is already shared; the remaining gap is the app/player layer:
shared UI, touch input, player movement, HUD/hotbar, and gameplay interaction.
The platform-wide input capability contract is now tracked in
[`098-flat-input-capability-convergence.md`](098-flat-input-capability-convergence.md);
new HUD/hotbar/interaction work should use that shared flat-client layer rather
than adding Android-only gameplay controls.

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
- The current app/player shell is still thin: closed-menu touch look and
  movement controls now drive `EngineCameraController` and sync player
  pose/interest through the shared runtime, `mclone-ui` renders pause/options
  and touch controls in code, but gameplay-facing HUD/hotbar, block
  interaction, actors, and device validation are still open.

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
  - Route touch down/move/up into `GameUi` before closed-menu look handling.
  - Apply scene-local menu actions: resume/open/back, fullbright, section
    occlusion, and render distance.
  - Device visual/touch validation is still pending.
- [x] **Slice 2: player camera/look adapter.**
  - Replace the orbit smoke camera with `EngineCameraController` and the shared
    player pose/interest-center path.
  - Convert closed-menu touch drag into the same mouse-look path used by the
    shared player controller.
  - Commit initial and touch-look player pose through the same local/remote
    runtime command/update path used by desktop/XR.
  - Touch movement intentions are intentionally deferred to Slice 3.
- [x] **Slice 3: Android touch HUD controls.**
  - Reuse `mclone-ui` touch overlay for movement/jump/menu affordances.
  - Keep control state render-only in UI and gameplay intent state in shared
    controller/runtime code.
  - Expose shared touch action hit rects from `mclone-ui`.
  - Convert the Android movement zone into analog `EngineCameraMovementImpulse`
    and action buttons into shared jump/sprint/descend camera input.
  - Keep requesting redraws while movement controls are held so movement ticks
    continuously.
  - Device visual/touch validation is still pending.
- [ ] **Slice 4: gameplay interaction parity.**
  - Route block raycast/break/place through the shared interaction controller.
  - Add hotbar/debug palette presentation and touch selection through the
    shared flat input capability layer tracked in
    [`098-flat-input-capability-convergence.md`](098-flat-input-capability-convergence.md).
- [x] **Slice 5a: first-class AVD validation lane.**
  - Make `android/build-apk.sh` ABI-selectable instead of hard-coding
    `arm64-v8a`.
  - Make `android/validate-avd.sh` build `x86_64` by default for local
    emulator images while preserving arm64 as the physical-device default.
  - Repair app-scoped external asset ownership on rootable emulators after
    `adb push`, so API 35 AVDs can read the staged pack.
  - Require the Mclone rendered-frame log marker before accepting an Android
    smoke as passed.
  - Route flat Android `pnpm` scripts through a native Git Bash launcher on
    Windows so they do not resolve to WSL `bash` and miss native cargo.
  - Clear `debug.mclone.remote_addr` with the `__mclone_none__` sentinel because
    Android `setprop` cannot write an empty value; the app filters the sentinel
    back to local integrated mode.
  - Document `pnpm native:android:apk:avd` and the AVD/device ABI split.
- [ ] **Slice 5b: device validation and parity matrix update.**
  - Run flat Android validation on emulator for local and remote modes.
  - Run Quest-flat validation on attached headset hardware.
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

Slice 2 validation on 2026-06-26:

- [x] `cargo fmt --manifest-path native/Cargo.toml --all`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-render-session`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android`
- [ ] Device visual/touch smoke of shared player camera/look on flat Android.

Slice 3 validation on 2026-06-26:

- [x] `cargo fmt --manifest-path native/Cargo.toml --all`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- [x] `cargo test --manifest-path native/Cargo.toml -p mclone-render-session`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android`
- [ ] Device visual/touch smoke of movement joystick and jump/sprint/descend
      controls on flat Android.

Slice 5a validation on 2026-06-26:

- [x] `bash -n android/build-common.sh android/build-apk.sh android/validate-common.sh android/validate-avd.sh android/validate-quest-flat.sh`
- [x] `cargo fmt --manifest-path native/Cargo.toml --all --check`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target x86_64-linux-android`
- [x] `cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android`
- [x] `pnpm native:android:apk:avd`
- [x] `pnpm native:android:avd-smoke -- --skip-build`
      - Screenshot inspected: `/tmp/mclone-android-avd-chunk.png`
- [x] `pnpm native:android:avd-touch-smoke -- --skip-build`
      - Screenshot inspected: `/tmp/mclone-android-avd-touch.png`

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
