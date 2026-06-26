# 082: Desktop XR Player Locomotion

Status: active first pass landed.

## Purpose

Add the first basic gameplay locomotion path for desktop OpenXR mclone frames.
The goal is intentionally modest: use Quest controller thumbsticks/buttons to
drive the same native player movement and collision controller used by desktop,
while keeping tracked headset pose layered inside the XR rig for rendering.

The model for this slice is:

```text
player/root in world
  tracking-space origin
    HMD pose from OpenXR runtime
      per-eye poses
```

Artificial locomotion moves the player/root through the existing
`EngineCameraController` and server pose-sync path. Physical room movement
changes the HMD pose under that root. More advanced body recentering, crouch
policy, hand-relative locomotion, snap-turns, and comfort fades are follow-up
slices.

## Playbox References

- `~/code/playbox/src/xr/locomotion.rs`
  - stage/world transform boundary
  - controller thumbstick dead zone and yaw-speed constants
  - startup view-pose alignment around the current stage view
- `~/code/playbox/src/xr/actions.rs`
  - Touch controller button/stick action shape
- `~/code/playbox/src/xr/mod.rs`
  - per-frame order: locate views, read controllers, update locomotion,
    transform stage-space inputs for rendering
- `~/code/playbox/docs/architecture/platforms.md`
  - platform-owned OpenXR input/rig state, shared engine kept OpenXR-free

## Scope

- Keep OpenXR controller/action decoding in `mclone-native-client`.
- Reuse `mclone-render-session::EngineCameraController` for movement,
  collision, jumping, and pose snapshots.
- Share player pose synchronization between desktop window and XR paths through
  `WindowSceneRuntime`, instead of duplicating server correction and
  chunk-interest logic.
- Map controls for the first basic pass:
  - left thumbstick: analog strafe/forward movement
  - right thumbstick X: smooth horizontal turn
  - right A button: jump
- Keep HMD pose as a render-only offset inside the player/root.
- Keep swapchain/frame submission and existing mclone XR rendering unchanged.

## Landed

- Added shared `WindowSceneRuntime` helpers for synchronizing an
  `EngineCameraController` with the server and chunk-interest runtime.
- Updated the desktop window path to use that shared player pose-sync helper,
  while keeping spectator mirroring desktop-local.
- Added a dedicated Touch right-A action so jump is not conflated with the
  generic right select/B binding.
- Added an XR player-root rig for mclone smoke rendering:
  - server initial position corrections are accepted first,
  - `--view-pose` is then applied as a real player/root pose,
  - the initial OpenXR stage view is captured as the tracking origin,
  - per-frame HMD poses render under the current player/root.
- Mapped XR input into shared `EngineCameraInput`:
  - left stick to analog `EngineCameraMovementImpulse`, using the
    Quest/VirtualDesktopXR-observed transposed axes and corrected strafe sign,
  - right stick X through the desktop mouse-look yaw path,
  - right A to jump.
- Added unit coverage for XR rig alignment, physical HMD offset, thumbstick
  dead-zone filtering, movement impulse mapping, right-stick yaw mapping, and
  A-button jump mapping.

## Out Of Scope

- Teleport/Blink/Shift locomotion.
- Hand-relative movement mode.
- Snap turning and turn comfort settings.
- Crouch/height policy.
- Body/capsule recentering or rubber-band policy beyond anchoring the stage
  origin to the player root.
- Head-through-wall fade/clamp.
- XR block interaction.

## Validation

Required gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
pnpm native:web:build
git diff --check
```

Implementation validation on June 25, 2026:

- `cargo fmt --manifest-path native/Cargo.toml --all --check`: passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`:
  passed, 78 tests.
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`:
  passed.
- `pnpm native:web:build`: passed.
- `git diff --check`: passed.

Live Windows validation target:

```powershell
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --forever --no-quest-restore --no-pause
```

Record after implementation:

- Runtime: `VirtualDesktopXR v1.0.10`.
- Headset/system: `Meta Quest 3`.
- Backend: Vulkan through `XR_KHR_vulkan_enable2`, physical device
  `NVIDIA GeForce RTX 4090`.
- Command:

```powershell
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,120,-96,180 --frames 1200 --no-pause
```

- The launcher started/reused Virtual Desktop Streamer, prepared Quest over
  ADB, reported `Quest battery: level=99% status=2 power=AC`, and restored
  Quest wake/proximity state afterward.
- The initial server spawn correction was acknowledged before applying the XR
  startup pose. The root stayed at `root_eye=(0.00, 120.00, -96.00)` with
  `root_yaw_degrees=180.0`.
- OpenXR submitted `1200` frames with `skipped=0`.
- Mclone frame summary reported real terrain draw work:
  `sections=122 drawn_sections=18 indices=580194 drawn_indices=163944`.
- Quest screencap: `C:\tmp\mclone-xr-locomotion-validation-3.png`.
  The capture path still includes Virtual Desktop shell overlays, but the
  runtime log confirms mclone XR frame submission and terrain drawing.
- Controller action data remained active but untracked/zero in this unattended
  run:
  `left_active=1200 right_active=1200 left_tracked=0 right_tracked=0 max_thumbstick=0.000 a_pressed_frames=0`.
  Live thumbstick/yaw/A-jump feel still needs a person in-headset with awake
  controllers.

Human headset verification on June 26, 2026:

- Open-ended command:

```powershell
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --forever --no-quest-restore --no-pause
```

- User verified the basic XR locomotion loop in-headset on Quest 3 through
  VirtualDesktopXR.
- Initial movement feel was good except the left thumbstick axes were
  transposed: physical left/right drove forward/back in-game.
- After swapping left-stick locomotion axes, forward/back was correct but
  strafe left/right was inverted.

Regression check after Android XR shared-locomotion extraction, June 26, 2026:

- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`:
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr`:
  passed, 80 tests.
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`:
  passed, 6 tests.
- `cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,120,-96,180 --frames 120 --no-pause`:
  passed.
- Observed runtime: `VirtualDesktopXR v1.0.10`, `Meta Quest 3`,
  `NVIDIA GeForce RTX 4090`.
- OpenXR submitted `120` frames with `skipped=0`.
- Mclone frame summary reported real terrain draw work:
  `sections=122 drawn_sections=18 indices=580194 drawn_indices=163944`.
- Added a bounded-smoke submitted-frame progress timeout so a future
  VirtualDesktopXR/Quest focus problem fails fast instead of hanging the
  regression script indefinitely.
- After inverting only the strafe sign, user reported the controls were
  "all good".
- Validation before the final manual relaunch:
  - `cargo fmt --manifest-path native/Cargo.toml --all --check`
  - `cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr xr_locomotion`
  - `cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr`
- After manual validation, the live `mclone-native-client` process was stopped
  and `Restore-McloneQuestVirtualDesktopState -StopQuestApp -SleepAfterRestore`
  was run to restore Quest wake/proximity settings and request headset sleep.

## Next Step

Add XR block-selection diagnostics from controller aim poses so movement and
interaction can be validated together before comfort options.
