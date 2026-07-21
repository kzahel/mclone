# Tactical 203: Shared Interactive Router And Native Adoption

Status: complete 2026-07-21. The shared synchronous router is implemented and
adopted by desktop and flat Android; their parallel final semantic dispatch is
deleted, the source/adoption locks pass, and affected native, XR, Android, and
rendered-output lanes have been validated.

Topic: `platform-host-boundary`

Related topic:

- [`../topics/platform-host-boundary.md`](../topics/platform-host-boundary.md)

## Goal

Establish one synchronous shared-Rust route from neutral flat input through
bindings, active UI/game context, camera/hotbar/world actions, and mechanical
host outcomes. Adopt that route in desktop and flat Android, deleting their
parallel final `FlatInputFrame` dispatch without imposing browser encodings,
async machinery, or a universal platform adapter on native.

This is the first implementation slice of the platform-host-boundary series.
It proves the shared contract on direct typed Rust paths before a later
tactical changes the browser/Wasm ABI.

## Current Shape

The reusable pieces already exist:

- `mclone-input::KeyboardMouseInputAdapter` owns keyboard/mouse bindings,
  repeat handling, held state, and `FlatInputFrame` production;
- `TouchInputAdapter` owns the flat touch state machine used by Android;
- `McloneSceneHost::advance_mono_input_frame` owns movement gating,
  activation progression, and pose publication;
- `McloneSceneHost` already owns UI, hotbar, camera, and world-action meaning;
  and
- `HostEffects` already carries mechanical cursor, pacing, touch-mode,
  quit-to-title, and exit requests.

The remaining native duplication is the final route:

- desktop `ChunkApp::apply_flat_keyboard_frame`,
  `apply_flat_world_action_frame`, and `apply_flat_look_frame` select scene
  behavior;
- flat Android `AndroidGpuState::apply_flat_frame` repeats the same semantic
  decisions; and
- desktop and Android each route UI-active key and pointer input around those
  paths independently.

## Fixed Contracts

1. This is a synchronous input/context router, not an actor or mailbox.
2. `mclone-input` retains physical controls, bindings, held/repeat state,
   touch/gamepad state, and flat intent/frame values.
3. `mclone-scene` owns UI-versus-world routing and applies camera, hotbar,
   interaction, menu, help, and presentation-facing scene consequences.
4. Platform code converts winit facts, owns windows/surfaces/event loops, and
   executes cursor/redraw/exit mechanics. It does not match gameplay actions.
5. Native calls remain direct and typed. No strings, serialized frames,
   promises, SAB shapes, or browser callbacks enter the shared contract.
6. XR retains `XrControllerSnapshot` and its modality-specific scene path.
   Shared semantic helpers may be reused, but XR is not flattened into the
   flat event vocabulary.
7. Offscreen and scripted tests may continue to submit semantic
   `FlatInputFrame` values directly as explicit engine clients.
8. Existing input behavior is preserved unless the tactical records an
   observed platform drift and deliberately resolves it through the shared
   binding policy.
9. Host-facing outcomes are mechanical: handled, scene/redraw changed, clear
   transient input, pointer/cursor desire, or exit. They do not expose Jump,
   Attack, Use, menu, hotbar, or world-action result vocabulary merely for
   platform logging.
10. The shared router must build without a winit dependency. Reusable winit
    normalization, if extracted, lives at the platform integration rim.

## Implementation Slices

### Slice 0: Behavioral and ownership locks

Status: complete 2026-07-21.

- Freeze shared binding behavior separately from winit normalization.
- Record desktop and Android handling for menu, help, camera view, hotbar,
  Attack/Use, look, held movement, UI-active keys/pointer, focus loss, and
  mouse wheel.
- Add a source lock naming the duplicated final-dispatch sites so deletion is
  measurable rather than inferred from line counts.
- Classify the current wheel divergence explicitly: shared bindings and flat
  Android step the hotbar, while desktop changes no-clip camera speed.

### Slice 1: Shared scene router

Status: complete 2026-07-21.

- Add the smallest coherent scene-owned route for resolved flat frames and
  active mono UI input.
- Reuse existing `McloneSceneHost` methods and `HostEffects`; do not create a
  second scene-policy owner.
- Return a compact mechanical outcome suitable for winit, Android, and later
  browser adoption.
- Test menu/help/camera/hotbar/interaction/look routing and UI/game context in
  shared Rust.

### Slice 2: Desktop adoption

Status: complete 2026-07-21.

- Make desktop winit normalization feed the shared route.
- Delete `ChunkApp`'s parallel flat-frame semantic matches and corresponding
  `WinitFrameDriver` semantic forwarding methods once unused.
- Keep desktop-only diagnostics and renderer-development shortcuts explicitly
  classified. Move a shortcut only when it is ordinary shared product input;
  do not disguise platform diagnostics as game bindings.
- Preserve cursor grabbing, redraw scheduling, frame pacing, surface/device
  handling, and logging needed for desktop operation.
- Resolve wheel behavior through an explicit decision rather than retaining an
  accidental desktop/Android difference.

### Slice 3: Flat Android adoption

Status: complete 2026-07-21.

- Make keyboard, pointer, and touch frames use the same shared route.
- Delete the semantic branches in `AndroidGpuState::apply_flat_frame`.
- Preserve Android activity/window/surface lifecycle, coordinate conversion,
  touch pointer capture facts, audio construction, and redraw cadence.
- Reuse shared touch state and UI behavior without forcing Android through a
  browser or DOM-shaped interface.

### Slice 4: Closeout

Status: complete 2026-07-21.

- Run the shared input/scene/app-runtime test suites.
- Run desktop window/offscreen input and UI validation and inspect a rendered
  capture.
- Build and test the flat Android lane through the repository scripts; run the
  available AVD input/UI smoke and inspect its capture when the host supports
  it.
- Run XR compile/control gates affected by shared scene changes.
- Update the master topic audit and platform-parity tracker with the landed
  contract, remaining browser gaps, and validation evidence.
- Mark this tactical complete and create the browser raw-input tactical from
  the actual shared API.

## Implementation Record

`mclone-scene::MonoInteractiveInputRouter` now owns the final synchronous
route from shared keyboard/mouse frames into active UI or gameplay context. It
uses `KeyboardMouseInputAdapter` for bindings and held state, calls the
existing `McloneSceneHost` policy methods for menu, help, camera, hotbar,
world interaction, look, UI pointer, and frame advancement, and returns only
`MonoInputDisposition` mechanical facts. It is neither an actor nor a new
scene-policy owner.

The desktop `WinitFrameDriver` and flat Android surface driver own one router
each. Their leaf code still converts winit key, pointer, motion, wheel, and
touch facts and executes cursor, redraw, activity, and surface mechanics, but
no longer matches `FlatInputFrame` fields to select engine behavior. Desktop
wheel input now follows the shared hotbar binding instead of retaining its
old no-clip camera-speed exception. Desktop-only renderer and development
shortcuts remain explicitly app-local diagnostics.

Android retains coordinate conversion and touch-control hit testing around
the existing shared `TouchInputAdapter`; it feeds the resulting neutral flat
frames through the same scene route. Unifying the browser and Android touch
layout/model remains a browser-tactical concern rather than a hidden
requirement of this native extraction.

The `platform_host_boundary_lock` test requires both native clients to adopt
the router and rejects reintroduction of the deleted app-local final-dispatch
methods. The thin-adapter gate now names `advance_held_frame` as the shared
route expected at both native rims.

## Validation Evidence

Passed:

- all `mclone-scene` library tests, including the shared router's UI-context
  and held-state tests;
- all `mclone-native-client` tests (168 unit tests plus three review tests);
- `mclone-android-client` tests and compile check;
- the focused platform-host-boundary source lock;
- `pnpm native:thin-adapters:purity` and `pnpm native:xr:check`;
- `pnpm native:desktop-offscreen:smoke`; its textured forest/actor capture at
  `/tmp/mclone-desktop-offscreen.png` was inspected;
- `pnpm native:xr-emulation:smoke`; its stereo terrain and world-space menu
  capture at `/tmp/mclone-xr-emulation.png` was inspected;
- `pnpm native:android:apk` and the dual-ABI AVD APK build; and
- all three flat-Android AVD lanes: ordinary terrain/HUD, injected touch look,
  and menu-to-new-world lifecycle. Captures at
  `/tmp/mclone-android-avd-chunk.png`,
  `/tmp/mclone-android-avd-touch.png`, and
  `/tmp/mclone-android-avd-session.png` were inspected.

The full workspace `cargo test --manifest-path native/Cargo.toml` control ran
all affected router/native suites successfully but did not finish green: the
unrelated server test
`sqlite_restart_restores_mclone_profile_before_unseen_generation` failed, and
failed again in isolation with a missing generated block assertion. This
tactical does not touch server persistence, generation, or that test; the
focused and affected-package gates above are green. The failure is recorded
rather than repaired by expanding an input-boundary tactical.

## Closeout

Acceptance criteria 1-12 are satisfied. Browser TypeScript and the Wasm action
ABI were deliberately unchanged, so the next tactical can be designed from
the landed typed router rather than from a speculative browser abstraction.

## Acceptance Criteria

1. Desktop and flat Android use one shared scene route for resolved flat input.
2. Their app code no longer matches `FlatInputFrame` fields to select menu,
   help, camera, hotbar, Attack, Use, or look behavior.
3. Shared tests prove those meanings once; leaf normalization tests prove only
   winit-to-neutral conversion.
4. UI-active input routing is shared or any remaining platform-local piece is
   demonstrably mechanical and recorded.
5. Focus/context transitions clear held and transient input without stuck
   movement.
6. Wheel behavior is intentional and consistent with the selected shared
   binding policy.
7. Desktop and Android retain direct typed hot paths and platform-native event
   loops.
8. XR behavior and data fidelity remain unchanged.
9. Offscreen semantic test clients remain available.
10. The source lock reports no native final-dispatch duplication targeted by
    this tactical.
11. Required native and Android validation passes, and rendered outputs are
    captured and inspected under `docs/platforms.md`.
12. No browser TypeScript or Wasm action ABI is changed in this tactical.

## Expected Follow-Ups

After this tactical closes, create these tacticals one at a time from landed
code rather than reserving speculative files or numbers:

1. browser raw-input adoption and deletion of TypeScript/web-Rust semantic
   dispatch;
2. Rust-authored diagnostic observer and production smoke-surface isolation;
   and
3. shared preference/bootstrap meaning with mechanical browser storage,
   capability, and resource executors.

The master sequence and cross-cutting acceptance criteria remain in
[`platform-host-boundary.md`](../topics/platform-host-boundary.md).

## Stop Conditions

Stop for renewed review if:

- the router would duplicate rather than call existing scene/client policy;
- the contract requires a winit dependency in `mclone-input` or
  `mclone-scene`;
- native would need browser serialization or async indirection;
- correct UI routing requires platform code to know a screen/game action not
  represented in shared state;
- XR pose/tracking data would be flattened into the flat contract; or
- preserving a current platform behavior conflicts with the accepted shared
  binding policy and requires a product choice rather than removal of obvious
  accidental drift.

Routine Rust refactoring, generated test changes, and platform cursor/redraw
mechanics are not stop conditions.

## Non-Goals

- changing the browser input ABI or TypeScript in this tactical;
- building an actor, universal adapter, or generic event loop;
- redesigning bindings, touch-control appearance, or an end-user rebinding UI;
- moving desktop renderer/debug shortcuts into product bindings without a
  separate decision;
- changing session, persistence, Worker, WebSocket, or catalog ownership; or
- eliminating semantic APIs from explicit offscreen/test clients.

## Code Map

Shared owners:

- `native/crates/mclone-input/src/lib.rs`
- `native/crates/mclone-scene/src/mono.rs`
- `native/crates/mclone-scene/src/host_effects.rs`

Desktop:

- `native/apps/mclone-native-client/src/app.rs`
- `native/apps/mclone-native-client/src/winit_frame_driver.rs`
- `native/apps/mclone-native-client/src/app/tests/input_adapter.rs`

Flat Android:

- `native/apps/mclone-android-client/src/surface_driver.rs`

XR/offscreen regression boundaries:

- `native/crates/mclone-xr-host/src/actions.rs`
- `native/apps/mclone-native-client/src/xr_emulation.rs`
- `native/apps/mclone-native-client/src/offscreen_scene_host.rs`
