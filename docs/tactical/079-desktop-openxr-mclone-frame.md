# 079: Desktop OpenXR Mclone Frame

Status: complete for the first desktop mclone-frame smoke. Windows
VirtualDesktopXR/Quest 3 over Vulkan was validated on June 25, 2026; macOS
Metal has matching launcher/backend wiring and should be rerun on a Mac runtime.

## Purpose

Render a real mclone runtime frame through the desktop OpenXR path.

This is the first slice where XR should consume the client/server/render-session
stack. The goal is not polished VR gameplay. The goal is to prove that the
shared runtime, renderer resources, render-section streaming, and per-eye
view/target contract work together without forking the engine.

## Target Shape

The XR host owns:

- OpenXR frame wait/begin/end.
- Per-eye swapchain image acquisition and release.
- Per-eye depth target ownership.
- Runtime-provided view/projection conversion into `ChunkRenderView`.
- Optional mirror/diagnostic output if cheap.
- XR-specific startup and validation mode.

Shared code owns:

- `SingleViewRuntime` or its successor for client/server/render-session state.
- Render-section compile and upload policy.
- Asset loading through existing packed/loose source helpers.
- Full-frame world composition from explicit view/target facts.

This slice may still run from a fixed headset-relative pose and static
controller-free interaction. Movement, hand/controller input, in-world UI, and
comfort features should be separate follow-ups.

## Implementation Slices

### Slice 1 - Runtime Reuse In XR Host

- [x] Build the same integrated-runtime scene used by desktop and flat Android
  smokes.
- [x] Load the standard packed/loose Minecraft asset source through shared
  render asset helpers.
- [x] Poll until the initial chunk set is ready.
- [x] Compile and upload render sections before the first XR world frame.
- [x] Keep the runtime/asset setup independent of OpenXR types.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
```

### Slice 2 - Per-Eye World Rendering

- [x] Convert OpenXR view/projection facts into `ChunkRenderView` values.
- [x] Render sky and terrain once per eye through the shared full-frame helper.
- [x] Use one host-owned depth target per eye.
- [x] Reuse one render-section cache and texture atlas for both eyes.
- [x] Ensure frustum/culling behavior is view-specific but cache ownership is
  frame/runtime-shared.

Suggested smoke:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- \
  --xr-mclone-smoke --frames 240 \
  --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --day-time 6000 --freeze-time
```

The exact CLI spelling can change, but it should be bounded and scriptable.

### Slice 3 - Diagnostics And Capture

- [x] Log runtime name, backend, swapchain format, view count, target size, and
  frame count.
- [x] Log render-section counts and drawn indices for eye 0 at least once.
- [x] Add a mirror or readback screenshot only if it is cheap and reliable on
  the local desktop runtime.
- [x] Record inspected output or headset-visible validation notes in this doc.

## Landed

- Added `--xr-mclone-smoke [--frames N]` as a bounded XR mode that accepts the
  existing scene/render options (`--seed`, `--chunk-x`, `--chunk-z`,
  `--render-distance`, `--day-time`, `--freeze-time`, section occlusion, and
  fullbright toggles).
- Reused `WindowSceneRuntime`, render-section upload, texture atlas, sky
  renderer, actor draw resources, and `render_full_frame_for_view`; OpenXR
  types remain app-local in `mclone-native-client`.
- Converted runtime `xr::View` pose/FOV into per-eye `ChunkRenderView` values
  and render each eye into the OpenXR swapchain with a host-owned
  `ChunkDepthTarget`.
- Added a focused XR projection test proving symmetric OpenXR FOV conversion
  matches the existing `Mat4::perspective_rh` convention used by
  `ChunkCamera`.
- Matched Playbox's per-eye stereo submission lifetime for the mclone XR path:
  each eye render is submitted and waited before encoding the next eye. This
  keeps shared renderer uniform buffers from being overwritten by the right-eye
  view before the left-eye pass reaches the GPU.
- Extended `scripts/start-xr.sh` with `--smoke clear|mclone` for macOS/Linux
  and `scripts/start-xr.ps1` with `-Smoke clear|mclone` for Windows.
- Added package scripts:
  - `pnpm native:xr:mclone`
  - `pnpm native:xr:windows:mclone:connected`
- Tightened the Windows Quest/Virtual Desktop helper so connected smoke runs
  sleep the headset by default unless `-NoQuestRestore` is passed explicitly.

## Validation

Required gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --features xr symmetric_xr_fov_matches_chunk_projection_convention
pnpm native:web:build
git diff --check
```

Launcher checks:

```powershell
bash -n scripts/start-xr.sh
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start-xr.ps1 -CheckOnly
```

Windows runtime used on June 25, 2026:

- Launcher-selected manifest:
  `C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json`
- Entry: fallback loader
  `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\openxr_loader.dll`
- Runtime: `VirtualDesktopXR v1.0.10`
- System: `Meta Quest 3`
- Backend: Vulkan through `XR_KHR_vulkan_enable2`
- GPU: `NVIDIA GeForce RTX 4090`, Vulkan API `1.4.341`, queue family `0`
- Reference space: `STAGE`
- Swapchains: `Rgba8UnormSrgb`, `1728x1824` per eye, `3/3` images

Connected clear-smoke regression:

```powershell
pnpm native:xr:windows:prepare
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start-xr.ps1 -Frames 2 -NoQuestLaunch -NoQuestRestore
```

Observed result: `submitted=2 runtime_frames=2 skipped=0`.

Connected mclone-frame smoke:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/start-xr.ps1 -Smoke mclone -Frames 1500 -NoQuestLaunch
```

Observed result:

- Runtime scene: `seed=12345`, center `(0, 0)`, render distance `2`
- Initial scene upload: `chunks=49`, `sections=166`, `faces=130029`,
  `indices=780174`
- Warmup: `initial_polls=6295`, `elapsed_ms=9587.242`
- Frames: `submitted=1500 runtime_frames=1500 skipped=0`
- Eye-0 render summary: `sections=166`, `drawn_sections=66`,
  `indices=780174`, `drawn_indices=396498`, `actors=1`, `drawn_actors=1`
- Cleanup: restored Quest wake/proximity settings from the saved state file and
  sent scripted headset sleep.

Visual validation:

```powershell
adb shell screencap -p /sdcard/mclone-xr-mclone.png
adb pull /sdcard/mclone-xr-mclone.png C:\tmp\mclone-xr-mclone.png
```

The inspected screenshot at `C:\tmp\mclone-xr-mclone.png` showed stereo mclone
terrain rendered in both eyes with the Virtual Desktop overlay composited
above it.

Follow-up stereo check on June 25, 2026:

```powershell
cmd /c scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --forever --no-quest-restore --no-pause
```

- User headset inspection found the left eye appeared mirrored/wrong while the
  right eye looked correct.
- Cross-check against Playbox showed Playbox submits/waits each eye render
  independently; mclone had batched both mclone eye passes into one command
  encoder while reusing uniform-buffer-backed shared render resources.
- Updated mclone XR rendering to submit/wait left eye before encoding right
  eye.
- Revalidated with `C:\tmp\mclone-xr-eye-submit-fix.png`; the headset session
  reached `FOCUSED` and remained live for manual inspection.

## Out Of Scope

- Android XR / Quest packaging.
- Controller actions and locomotion.
- Hand tracking.
- In-world UI or XR panels.
- Passthrough and spatial-room support.
- Performance tuning beyond basic non-stutter diagnostics.
- Multi-player/remote-player-specific XR presentation.

## Review Rejection Criteria

- Duplicating the desktop or Android runtime/render-session stack inside an XR
  module.
- OpenXR types inside shared runtime, client, server, protocol, mesh, asset,
  light, worldgen, or render-session crates.
- A separate XR renderer for chunks or sky.
- Per-eye render-section rebuilds when the section cache could be shared.
- Any Android XR package or Quest manifest changes in this desktop slice.

## Completion Criteria

- Desktop OpenXR can render real mclone terrain through shared renderer/runtime
  code.
- The XR path uses explicit per-eye `ChunkRenderView` and host-owned
  `RenderFrameTarget`/depth facts.
- The render-section cache, texture atlas, and asset helpers are shared with
  desktop/Android rather than forked.
- The validation path is bounded, documented, and runnable by command.
- Android XR can be planned as a packaging/runtime-host slice instead of an
  engine-boundary refactor.
