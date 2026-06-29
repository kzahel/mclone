# 107: XR Stereo Uniform Ownership and Multiview

Status: active high-priority prerequisite; Slices A-C landed, Slice D desktop
proof landed, and Slice E's headless, Android XR, terrain-chunk proof, sky,
actor, selection-outline, world-GUI, and screen-effect render paths, and
terrain+sky / terrain+sky+actor multiview perf paths exist. Quest now exposes
`wgpu::Features::MULTIVIEW` after enabling
`VK_KHR_get_physical_device_properties2` on the OpenXR Vulkan instance and
threading `VK_KHR_multiview` into the wgpu-wrapped device extension list, and
the on-device proofs now validate true multiview layer writes/readback, the
left/right stereo projection guard, chunk-terrain rendering, and synthetic
selection/world-GUI/screen-effect overlay rendering through a two-layer OpenXR
swapchain. The production headset frame remains on the correct per-eye submit
path until the full-frame multiview switch is made and measured. Created after
`d0c5161` (`Restore XR per-eye command submission`) rolled back the unsafe
single-submit Quest XR optimization from `264c723`.

## Goal

Make XR stereo rendering safe to optimize again by giving every per-view uniform
write stable ownership for the whole GPU submission. Only after that foundation
is proven should we reintroduce one-submit stereo recording, multiview, or
cross-frame CPU/GPU overlap.

The immediate rule is:

> **Do not re-land bare single-submit.**

The current correctness baseline is per-eye encoder/submit/wait. It is slower
than the reverted optimization, but headset validation showed it renders correct
left/right projections.

## Why This Exists

Tactical 106 and commit `d0c5161` record the failure. The risky single-submit
path recorded the left eye, then the right eye, then submitted both together.
Most per-view renderers update one shared uniform buffer with
`queue.write_buffer(..., 0, ...)`. Under wgpu queue ordering, all writes issued
before a submit are visible to that submit in issue order. So this sequence:

```text
write(left uniforms) -> record left -> write(right uniforms) -> record right -> submit both
```

leaves the shared buffer holding right-eye data when the submit executes. The
left-eye commands can therefore read right-eye uniforms deterministically. That
matches the headset symptom: left-eye geometry/projection looked wrong while the
right eye looked correct.

This is not an underwater FOV issue and not a projection-math issue. It is a GPU
resource lifetime issue across the stereo pass.

## Affected Surface

The problem is broader than chunk terrain. The same per-view shared-uniform or
shared queue-written view-data pattern exists across the renderers that
participate in XR frames:

- chunk terrain (`mclone-render/src/chunk.rs`)
- sky (`mclone-render/src/sky_render.rs`)
- actors/entities (`mclone-render/src/entity.rs`)
- selection outline (`mclone-render/src/selection_outline.rs`)
- world GUI (`mclone-render/src/gui.rs`)
- screen effects (`mclone-render/src/screen_effect.rs`)

Any future one-submit, multiview, or frame-pipelined XR path must fix all of
these, not just the chunk camera uniform.

## Design Direction

### Minimal Safe Foundation: Dynamic-Offset Uniform Ring

Introduce a shared render-side helper for per-view uniform storage:

- allocate one uniform buffer with 256-byte-aligned slots, respecting
  `device.limits().min_uniform_buffer_offset_alignment`;
- reserve slots by `(frame_in_flight, view_index, renderer_uniform_kind)`;
- write each eye's data to a distinct slot;
- bind with `has_dynamic_offset: true` and pass the slot offset when setting the
  bind group;
- keep flat/single-view paths on the same API with view index `0` and one live
  slot, so the contract stays shared rather than XR-only.

For the current per-eye-submit baseline this is behavior-preserving. For a future
one-submit path it gives left and right eyes immutable data for the whole submit.
For a future E4 overlap path it prevents frame N+1 writes from clobbering frame N
while N is still in flight.

Separate per-eye buffers or per-eye bind groups are acceptable fallback
mechanics, but they should not be the default unless dynamic offsets hit a real
backend limitation. Push constants are not the preferred path: the chunk matrix
payload is already near the guaranteed Vulkan push-constant floor, and every
per-view renderer would need parallel shader changes.

### Strategic Target: Stereo Multiview

The performance target remains Slice I from 106: render both eyes through a
two-layer target with multiview enabled and shader-side `@builtin(view_index)`.
Each per-view uniform becomes either:

- a dynamic-offset slot selected by the pass setup, for non-multiview paths; or
- a `[2]` uniform array selected by `view_index`, for the multiview path.

Multiview is the preferred place to spend complexity because it attacks the real
GPU bottleneck E1 exposed, and it removes duplicate per-eye encode/draw work. A
bare single-submit path only removes small submit/poll overhead and is not worth
reintroducing on its own.

### E4 Overlap Is Optional, Not Default

Cross-frame CPU/GPU overlap remains promising, and E2's loaded-vs-control
contention delta is still useful historical evidence. But E4 should be a runtime
engine setting for quick A/B testing, not a hardcoded Quest-only behavior. It
adds latency, so it must ship behind a user/developer-visible toggle and be
validated for comfort.

E4 must wait until per-view uniforms are immutable across frames in flight.

## Validation Gates

This tactical is pixel-affecting. It is not complete until both eyes are checked.

Required validation before reintroducing one-submit or multiview:

1. **Desktop/offscreen one-submit uniform test.** Record two views into one
   submit using deliberately different uniforms and read back two targets. The
   test must fail on the old shared-offset pattern and pass with the uniform-ring
   path.
2. **Real stereo projection check.** Use asymmetric/canted XR-like projections,
   not just an X-position offset. A fix that shares projection but changes only
   eye position is still wrong.
3. **Headset visual gate.** Inspect both eyes in the Quest headset. Confirm no
   left-eye off-center projection, no distortion, and stable depth fusion on near
   geometry.
4. **Per-eye capture when practical.** Prefer app-side capture of left and right
   render targets. Do not rely on `adb screencap` after teardown; the previous
   zero-byte capture attempt is not an acceptable visual gate.
5. **Perf rows only after correctness.** Any new timing rows must name the commit
   and state whether the path is per-eye submit, one-submit, or multiview.

## Ordered Slices

### Slice A - Fence or Remove Orphaned E1/E2 Surfaces

**Landed 2026-06-29.** The dead E1/E2 surfaces no longer produce fake data:
`native:android-xr:perf:frozen:rd10:gpu` and `:contention` package lanes were
removed; `--perf-gpu-timestamps` and `--perf-poll-contention` were removed from
the Quest validator and Android XR startup parser, so attempts to use them now
fail instead of running inert stubs; the Android app no longer emits
`MCLONE_ANDROID_XR_PERF_GPU=0.000` or the inactive contention marker; the
always-zero scene/app GPU timing fields, `poll_contention_active`, contention
buckets, no-op scene setters, and unused OpenXR timestamp-query feature request
were removed. Historical E1/E2 records remain in docs only.

`d0c5161` removed the E1 GPU timestamp and E2 poll-contention implementations,
but some command-line and package-script surface may still exist. Audit:

- `--perf-gpu-timestamps`
- `--perf-poll-contention`
- `native:android-xr:perf:frozen:rd10:gpu`
- `native:android-xr:perf:frozen:rd10:contention`
- app-side contention buckets / markers
- `poll_contention_active` timing fields

Do not leave benchmark lanes that appear to collect real data while the shared
scene stubs are inert. Either remove the lanes, or make them fail fast with a
message pointing to 106/107 and `d0c5161`.

### Slice B - Add the Uniform Ownership Helper

**Landed 2026-06-29.** Added `mclone_render::uniform::PerViewUniformBuffer`, a
shared dynamic-offset uniform helper that owns 256-byte-aligned slot sizing,
bind-group layout/resource creation, and slot writes. `SelectionOutlineRenderer`
is the first migrated per-view renderer; it still uses slot `0` on the existing
flat/per-eye-submit paths, but its bind group now uses a dynamic uniform offset.
This proves the helper without changing stereo submission behavior. Slice C must
still migrate the remaining per-view renderers before any one-submit path is
allowed.

Create the shared render helper and convert one renderer behind the existing
per-eye-submit path first. The helper should expose a small API that hides
alignment, slot selection, and dynamic offset binding from callers.

Acceptance:

- flat/single-view callers still render through offset `0`;
- XR per-eye-submit output is visually unchanged;
- no per-frame heap allocation is introduced for normal uniform writes.

### Slice C - Migrate All XR Per-View Renderers

**Landed 2026-06-29; headset visual check passed.** Chunk terrain, sky,
actors/entities, selection outline, and world GUI now use
`PerViewUniformBuffer` with dynamic uniform offsets and two stereo-capable slots.
Screen effects have no projection uniform today, but the underwater overlay
vertex buffer now uses two per-view slots and binds the selected slice, removing
the same queue-write clobber risk from that pass. The per-eye-submit baseline
was installed and checked in headset after this slice; left/right stereo looked
normal.

Move chunk terrain, sky, actors, selection outline, world GUI, and screen effects
onto the same ownership model. Do not start one-submit until this slice is
complete; partial migration leaves the same class of bug in the remaining
renderer.

Acceptance:

- focused renderer tests compile/pass;
- desktop flat and Android XR build gates pass;
- headset visual check still matches the per-eye-submit baseline.

### Slice D - Prove One-Submit Correctness Without Claiming The Perf Win

**Desktop proof landed 2026-06-29.** `mclone_render::uniform::PerViewSlot`
defines explicit single/left/right slots, the shared full-frame render path can
render into a caller-selected slot, and XR terrain now passes left slot `0` and
right slot `1` while still using the safe per-eye submit baseline. The ignored
GPU proof
`headless::tests::one_submit_keeps_distinct_per_view_uniform_slots_live`
records two render passes with different uniform-slot colors into one command
buffer, submits once, reads both targets back, and proves the left commands do
not read the right slot. No production one-submit path or perf lane was enabled.

Add a temporary diagnostic path or test-only path that records both eyes into one
submit using the new uniform slots. Its purpose is correctness proof, not the
final performance feature.

Acceptance:

- the offscreen one-submit test proves left/right views use distinct uniforms;
- both eyes are inspected in headset;
- no public perf lane reports this as the final optimization yet.

### Slice E - Implement Multiview As The Real Stereo Optimization

**Headless proof path landed 2026-06-29.** The renderer now requests optional
`wgpu::Features::MULTIVIEW` when a native adapter advertises it, `headless.rs`
can read back an individual array layer, and the ignored GPU proof
`headless::tests::multiview_renders_distinct_view_index_layers` records one
multiview pass into a two-layer `D2Array` target. The shader uses
`@builtin(view_index)` to write red for layer `0` and green for layer `1`, then
the test reads each layer separately.

Current Mac validation is compile/skip only: the local headless adapter does not
expose `wgpu::Features::MULTIVIEW`, so the ignored proof reports an explicit
skip instead of pretending to validate multiview execution. Production XR
multiview remains pending; the Quest path still uses two independent
`array_size: 1` color swapchains and per-eye depth targets.

**Android XR proof launch path added 2026-06-29.** The OpenXR Vulkan device now
requests optional `wgpu::Features::MULTIVIEW` when the runtime-backed adapter
advertises it, and the Android XR app has an explicit `--multiview-proof` mode.
That mode creates one two-layer OpenXR color swapchain, wraps it as a `wgpu`
`D2Array` texture, renders the minimal `view_index` multiview proof into both
layers, and presents layer `0` as the left projection view and layer `1` as the
right projection view. Normal terrain rendering remains on the correct per-eye
swapchain/submit path.

Validation command for the on-device proof:

```bash
pnpm native:android-xr:multiview-proof
```

This waits for `MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY`. The marker now means
two separate things:

- the Quest/OpenXR/Vulkan path can create the two-layer target, expose
  `wgpu::Features::MULTIVIEW`, and submit the minimal `view_index` multiview
  presentation proof;
- before presenting, the app renders an offscreen two-layer stereo projection
  fixture using the runtime OpenXR left/right poses and asymmetric FOVs, reads
  both layers back on-device, and verifies that a world-space marker lands at
  the expected per-eye pixel positions with real stereo disparity.

This is a defensive check for swapped, mono, or miscentered eye projection
plumbing before a human headset pass has to catch it. It does not mean terrain,
sky, entities, GUI, or screen effects have been migrated to multiview.

Validation recorded for this slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-render headless::tests::one_submit_keeps_distinct_per_view_uniform_slots_live -- --ignored --exact --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render headless::tests::multiview_renders_distinct_view_index_layers -- --ignored --exact --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-xr-host -p mclone-xr-graphics -p mclone-render
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
bash -n android-xr/validate-quest-openxr.sh
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8'))"
git diff --check
./android-xr/build-apk.sh --release
pnpm native:desktop-offscreen:smoke
```

The multiview proof command compiled and exited successfully with the explicit
skip message `headless adapter does not expose wgpu MULTIVIEW`. Android XR
release APK SHA-256:
`8d7fab38d33bac6c260cd222233d0c683ac7a90aad1edae9e459e18ad8a43adc`.
Desktop offscreen screenshot `/tmp/mclone-desktop-offscreen.png` was inspected
and showed the expected nonblank terrain/cow scene.

Latest Android XR release build after adding the Android proof path passed with
APK SHA-256:
`7120607635b94e1586357b0e3b2cb0c92141e82db1494e3117c57aa46e8a704f`.
The first on-device attempt initially failed because the local `adb` server had
a stale empty device list. After `adb kill-server && adb start-server`, the Quest
3 appeared as `2G0YC1ZF93041Z`. The proof run then launched and created the
two-layer OpenXR swapchain (`eye=1680x1760 images=3 layers=2`), but failed
before drawing because the OpenXR-backed `wgpu` device reported
`multiview=false` and `render_multiview_layer_proof` bailed with
`wgpu device does not expose MULTIVIEW`.

Research signal gathered 2026-06-29 says this is likely an initialization or
wgpu-wrapper issue, not a Quest 3 hardware limit:

- Meta's Horizon OS docs say Multiview is enabled by default for OpenXR in the
  Meta Quest feature group:
  <https://developers.meta.com/horizon/documentation/unity/enable-multiview/>.
- Meta's OpenXR Quest Support settings describe Quest-specific optimizations and
  advanced rendering settings:
  <https://developers.meta.com/horizon/documentation/unity/unity-openxr-settings-quest/>.
- Vulkan documents `VK_KHR_multiview` as a VR-oriented feature for recording one
  set of commands with different behavior for each view:
  <https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_multiview.html>.
- `wgpu::Features::MULTIVIEW` is documented as enabling multiview render passes
  and `builtin(view_index)` on Vulkan:
  <https://wgpu.rs/doc/wgpu/struct.Features.html#associatedconstant.MULTIVIEW>.
- Khronos OpenXR guidance confirms a single layered swapchain can present both
  projection views by using the same swapchain and different array indices:
  <https://community.khronos.org/t/enable-vulkan-multiview-extension-in-openxr-the-right-way/107585>.
- The `openxr` crate's Vulkan example targets Vulkan 1.1, explicitly enables
  `VkPhysicalDeviceMultiviewFeatures { multiview: VK_TRUE }`, and renders with a
  multiview render pass.
- A public native OpenXR/Vulkan optimization lab reports Quest 3 / Adreno 740
  testing with Vulkan multiview:
  <https://github.com/myers/openxr-vulkan-multiview-msaa>.
- Qualcomm documents hardware-accelerated multiview rendering for Snapdragon XR2
  class Adreno GPUs in Vulkan and OpenGL ES:
  <https://www.qualcomm.com/developer/blog/2023/10/reducing-rendering-work-and-memory-operations-stereoscopic-scenes-new-multiview-extensions>.

Follow-up diagnosis found a concrete local initialization suspect: the OpenXR
Vulkan instance was wrapped into wgpu-hal with an empty enabled-instance-extension
list. wgpu-hal only loads `VK_KHR_get_physical_device_properties2` when that
extension is listed, even though the feature-query functionality is promoted to
Vulkan 1.1. Without properties2, wgpu-hal cannot query
`VkPhysicalDeviceMultiviewFeatures`, so the adapter can under-report
`wgpu::Features::MULTIVIEW`.

Follow-up implementation enables `VK_KHR_get_physical_device_properties2` when
advertised, passes that list to wgpu-hal, and logs raw Vulkan multiview
feature/property state beside the wgpu adapter/device bits. The first run after
that change proved the capability path was fixed, but hit a proof-shader
validation error: Naga 25 expects `@builtin(view_index)` to be `i32`, not `u32`.
Both proof shaders were corrected to pass a flat `i32` view index.

Quest 3 validation then passed:

```text
OpenXR Vulkan session: physical_device='Adreno (TM) 740' api=1.3.295 queue_family=0
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
OpenXR multiview proof swapchain: color_format=Rgba8Unorm eye=1680x1760 images=3 layers=2
MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY submitted=1 runtime_frames=1 skipped=0 eye=1680x1760 layers=2 multiview=true
```

Release APK SHA-256 for that proof run:
`54c5910e4cb5542329dcbe294b5e0974d82a3a54a48551837a91523d2e44703e`.

Follow-up defensive validation added a Quest-side stereo projection readback
guard to the same proof mode. Final validation passed:

```text
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
OpenXR multiview proof swapchain: color_format=Rgba8Unorm eye=1680x1760 images=3 layers=2
OpenXR stereo projection proof fixture: left_expected=(1151.1,695.3) right_expected=(710.9,695.3) expected_disparity_px=440.2 tolerance_px=26.4
OpenXR stereo projection proof readback: left_marker_pixels=1224 right_marker_pixels=1224 left_first=[5, 10, 20, 255] right_first=[5, 10, 20, 255]
MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY submitted=1 runtime_frames=1 skipped=0 eye=1680x1760 layers=2 multiview=true expected_disparity_px=440.2 tolerance_px=26.4 left_actual=(1151.0,695.0) left_expected=(1151.1,695.3) left_error_px=0.3 left_pixels=1224 right_actual=(711.0,695.0) right_expected=(710.9,695.3) right_error_px=0.3 right_pixels=1224
```

Release APK SHA-256 for that proof run:
`855be06a1537c71b87457ca4eb27854d1d96e26f2e3ea00cb698a0e4014f0aff`.

Follow-up true-multiview validation initially failed before any shader was
involved: a private `D2Array` `multiview: Some(2)` clear-only pass read back
zeroes from both layers. The root cause is the wgpu-hal 25 Vulkan imageless
framebuffer path on Quest/Adreno. Disabling that optional path makes the same
wgpu multiview pass write both layers correctly. The workspace now carries a
temporary patched `wgpu-hal 25.0.2` under
`native/vendor/wgpu-hal-25.0.2`, wired through `[patch.crates-io]`, with
`imageless_framebuffers` disabled until a deliberate wgpu upgrade removes the
workaround.

Final Quest 3 validation with the vendored workaround passed:

```text
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
OpenXR multiview private clear-only readback: left_clear_pixels=4096 right_clear_pixels=4096 minimum_expected_pixels=3686 left_first=[5, 10, 20, 255] right_first=[5, 10, 20, 255]
OpenXR multiview private readback: left_red_pixels=4096 right_green_pixels=4096 minimum_expected_pixels=3686 left_first=[255, 0, 0, 255] right_first=[0, 255, 0, 255]
OpenXR stereo projection proof fixture: left_expected=(1151.1,695.3) right_expected=(710.9,695.3) expected_disparity_px=440.2 tolerance_px=26.4
OpenXR stereo projection proof readback: left_marker_pixels=1224 right_marker_pixels=1224 left_first=[5, 10, 20, 255] right_first=[5, 10, 20, 255]
OpenXR multiview swapchain readback: left_red_pixels=2956800 right_green_pixels=2956800 minimum_expected_pixels=2661120 left_first=[255, 0, 0, 255] right_first=[0, 255, 0, 255]
MCLONE_ANDROID_XR_MULTIVIEW_PROOF_READY submitted=1 runtime_frames=1 skipped=0 eye=1680x1760 layers=2 multiview=true private_left_red=4096 private_right_green=4096 swapchain_left_red=2956800 swapchain_right_green=2956800 expected_disparity_px=440.2 tolerance_px=26.4 left_actual=(1151.0,695.0) left_expected=(1151.1,695.3) left_error_px=0.3 left_pixels=1224 right_actual=(711.0,695.0) right_expected=(710.9,695.3) right_error_px=0.3 right_pixels=1224
```

Release APK SHA-256 for that proof run:
`687a87f00554a651087c2cd5c10905be18b50e11a3da3e8b36ed7a397bb37a19`.

**Terrain chunk multiview proof path landed 2026-06-29.** Chunk terrain now has
a separate multiview shader/pipeline path using a `[2]` view uniform array
selected by `@builtin(view_index)`, plus a two-layer chunk depth target. The
proof mode renders prepared terrain sections into one two-layer OpenXR color
swapchain, culls each eye independently, draws the union of visible sections in
one multiview pass, and reads both swapchain layers back. The sky background
then gained the same multiview shape in `9be4fec` (`Add sky multiview rendering
path`). The normal headset frame still uses the safe per-eye submit path.

Validation command:

```bash
pnpm native:android-xr:terrain-multiview-proof
```

Quest 3 validation passed:

```text
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
OpenXR multiview terrain layer difference readback: different_pixels=2317676 minimum_expected_different_pixels=2956 left_first=[27, 41, 16, 255] right_first=[41, 62, 24, 255]
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY submitted=209 runtime_frames=209 skipped=0 eye=1680x1760 layers=2 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 different_pixels=2317676 minimum_different_pixels=2956
```

Release APK SHA-256 for that proof run:
`8f48d2b1aa1d666fc5179e8d898660ca9ac498d98269b700d65c7431176c6409`.

**Terrain-only A/B perf probe landed 2026-06-29.** Android XR now has a
diagnostic `--terrain-multiview-perf` launch mode:

```bash
pnpm native:android-xr:terrain-multiview-perf
```

The probe waits until terrain chunks are drawable, then renders frozen terrain
offscreen with the current per-eye shape (left submit/wait, right submit/wait)
and the terrain multiview shape (one `multiview: Some(2)` pass). It warms both
paths, alternates measurement order, takes 60 samples, and excludes any layer
readback. It is a terrain microbenchmark, not a full-frame headset pacing claim.

Initial Quest 3 results are mixed but useful:

```text
Default tiny scene:
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 stereo_avg_ms=1.637 stereo_p50_ms=1.447 stereo_p95_ms=2.492 multiview_avg_ms=1.665 multiview_p50_ms=1.494 multiview_p95_ms=2.111 delta_avg_ms=-0.027 speedup=0.984

RD10 frozen pose 0,80,-96,180:
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=18 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=1650 right_drawn_indices=1650 stereo_avg_ms=1.044 stereo_p50_ms=0.908 stereo_p95_ms=1.442 multiview_avg_ms=0.726 multiview_p50_ms=0.574 multiview_p95_ms=1.088 delta_avg_ms=0.319 speedup=1.439

RD10 frozen pose 0,120,-96,180:
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=3 right_drawn_sections=3 left_drawn_indices=4644 right_drawn_indices=4644 stereo_avg_ms=1.441 stereo_p50_ms=0.913 stereo_p95_ms=1.488 multiview_avg_ms=1.060 multiview_p50_ms=0.578 multiview_p95_ms=0.996 delta_avg_ms=0.381 speedup=1.360
```

Interpretation: there is no universal terrain win yet. The tiny/default case is
flat to slightly worse on average. The two RD10 frozen-pose microbenchmarks show
a positive terrain-only signal (about `1.36x` to `1.44x`) where removing the
second submit/pass appears to matter. This is enough to continue to full-frame
multiview migration, but not enough to claim production frame pacing improvement
until sky/entities/GUI/screen effects are migrated and measured in the normal
headset frame loop.

Release APK SHA-256 for the perf runs:
`3c95848259dda09f80abd92271f6f51452a57016eb1bb23e0db9f6596cdc2a2b`.

**Sky+terrain A/B perf probe added 2026-06-29.** Android XR now also has a
diagnostic launch mode that measures the current per-eye sky+terrain shape
against one multiview sky pass plus one multiview terrain pass:

```bash
pnpm native:android-xr:sky-terrain-multiview-perf
```

Quest 3 results keep the same pattern as terrain-only: the tiny/default case is
flat, while the RD10 frozen pose shows a positive microbenchmark signal.

```text
Default tiny scene:
MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 stereo_avg_ms=2.035 stereo_p50_ms=1.870 stereo_p95_ms=2.742 multiview_avg_ms=2.015 multiview_p50_ms=1.898 multiview_p95_ms=2.453 delta_avg_ms=0.020 speedup=1.010

RD10 frozen pose 0,120,-96,180:
MCLONE_ANDROID_XR_SKY_TERRAIN_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=3 right_drawn_sections=3 left_drawn_indices=4644 right_drawn_indices=4644 stereo_avg_ms=1.090 stereo_p50_ms=1.007 stereo_p95_ms=1.411 multiview_avg_ms=0.693 multiview_p50_ms=0.636 multiview_p95_ms=1.248 delta_avg_ms=0.397 speedup=1.574
```

Interpretation: sky itself does not turn the tiny/default case into a win, but
the RD10 pose improves more strongly than the terrain-only row at the same pose
(`1.57x` vs `1.36x`). This is still an offscreen microbenchmark. It says the
multiview path is worth continuing, not that normal headset frame pacing has
improved yet.

Release APK SHA-256 for the sky+terrain perf runs:
`8d2b302ff8865038b09ed4f2831ea06d4e4cd25403dcf996ccf8bbf714dd0aa9`.

**Actor multiview path and sky+terrain+actor A/B probe added 2026-06-29.**
Actors now have a separate multiview shader/pipeline using a `[2]` view uniform
array selected by `@builtin(view_index)`. Android XR has a diagnostic launch
mode that measures the current per-eye sky+terrain+actor shape against one
multiview sky pass, one multiview terrain pass, and one multiview actor pass:

```bash
pnpm native:android-xr:sky-terrain-actors-multiview-perf
```

The marker reports actor counts so a run cannot silently claim actor coverage
when the scene has none. The current default and RD10 scenes both exercised one
actor:

```text
Default tiny scene:
MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 actors=1 drawn_actors=1 stereo_avg_ms=2.038 stereo_p50_ms=1.951 stereo_p95_ms=2.582 multiview_avg_ms=2.071 multiview_p50_ms=1.954 multiview_p95_ms=2.493 delta_avg_ms=-0.033 speedup=0.984

RD10 frozen pose 0,120,-96,180:
MCLONE_ANDROID_XR_SKY_TERRAIN_ACTORS_MULTIVIEW_PERF_SUMMARY samples=60 warmup=12 eye=1680x1760 sections=6 left_drawn_sections=3 right_drawn_sections=3 left_drawn_indices=4644 right_drawn_indices=4644 actors=1 drawn_actors=1 stereo_avg_ms=1.390 stereo_p50_ms=1.228 stereo_p95_ms=1.788 multiview_avg_ms=0.898 multiview_p50_ms=0.714 multiview_p95_ms=1.312 delta_avg_ms=0.491 speedup=1.547
```

Interpretation: adding one actor does not improve the tiny/default case; it
stays flat to slightly worse. The RD10 actor-inclusive row remains strongly
positive (`1.55x`), roughly matching the sky+terrain RD10 signal. This keeps
multiview worth pursuing, but still does not prove normal headset frame pacing
until the remaining full-frame passes are migrated and measured in the live
frame loop.

Release APK SHA-256 for the sky+terrain+actor perf runs:
`a28c45dc44579360c390ea389d1618458087dc7e68132f55f7ae507f44731527`.

**Selection outline and world-GUI multiview paths added 2026-06-29.**
Selection outlines now have a separate multiview shader/pipeline using a `[2]`
view uniform array. World GUI panel quads and world-space line/ray overlays have
matching multiview panel and line pipelines; the offscreen 2D panel texture
remains single-view because it is shared UI content, not eye-specific
projection data.

The existing terrain multiview proof now submits a small synthetic overlay smoke
after terrain rendering and before readback. That smoke draws a selection
outline plus a world-GUI panel/line into the acquired two-layer target, so Quest
validation exercises these new lazy pipelines on a real multiview device.

Quest 3 validation passed:

```text
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY submitted=208 runtime_frames=208 skipped=0 eye=1680x1760 layers=2 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 different_pixels=2311886 minimum_different_pixels=2956
```

Release APK SHA-256 for that proof run:
`1d38b499657de49dffa05e9358e4bad7dd3de5cd0a9429970c864ee6178f4712`.

**Underwater screen-effect multiview path added 2026-06-29.** The underwater
screen effect now has a dedicated multiview shader/pipeline. Unlike terrain or
actors it has no view-projection uniform, but it does carry per-eye overlay
state in a `[2]` uniform array selected by `@builtin(view_index)`, so midpoint
mode broadcasts the same tint while per-eye validation mode can keep distinct UV
offsets/alpha or a transparent eye at the waterline.

The synthetic overlay smoke in the terrain multiview proof now draws a low-alpha
underwater screen effect before the selection outline and world-GUI panel/line.
Quest 3 validation passed:

```text
OpenXR wgpu features: multiview=true
OpenXR Vulkan multiview diagnostics: instance_properties2_ext=true device_khr_multiview_ext=true raw_feature=true raw_geometry_shader=false raw_tessellation_shader=false max_views=6 max_instance_index=4294967295 wgpu_adapter=true wgpu_device=true
MCLONE_ANDROID_XR_TERRAIN_MULTIVIEW_PROOF_READY submitted=208 runtime_frames=208 skipped=0 eye=1680x1760 layers=2 sections=6 left_drawn_sections=2 right_drawn_sections=2 left_drawn_indices=15924 right_drawn_indices=15924 different_pixels=2319253 minimum_different_pixels=2956
```

Release APK SHA-256 for that proof run:
`0f4d24cfbfac13387c90355d6b5fa4a0c794cdeae650650f81437bce92fcee70`.

Move from proof-of-correctness one-submit to the real target:

- switch a full production XR frame to the multiview renderer stack behind a
  validation/perf lane;
- decide whether terrain's union-of-eye-culls policy is acceptable for the first
  production path or should be tightened before enabling;
- shared draw list feeding both eye layers.

Acceptance:

- left/right eye images are correct in headset;
- per-eye capture or equivalent validation exists;
- frozen RD10 rows compare current per-eye submit vs multiview at the same pose;
- any E1-style GPU timing reintroduced here is compatible with multiview and not
  tied to the removed single-encoder probe.

### Slice F - Optional E4 Frame Pipelining

Only after Slice B/C make uniform data safe across frames in flight:

- add an engine/runtime setting for pipelined XR rendering;
- keep the default conservative until comfort validation exists;
- measure motion-to-photon before and after;
- re-run a contention probe on the correct stereo path.

Acceptance:

- runtime toggle supports quick A/B;
- headset comfort check passes;
- perf records clearly identify the toggle state.

## Non-Goals

- Do not revive the invalid `264c723` behavior as a standalone optimization.
- Do not fork Quest-only renderers to avoid fixing shared render ownership.
- Do not treat historical E1/E2 numbers as current-main instrumentation.
- Do not land latency-increasing E4 behavior as an always-on default.

## References

- Rollback record: [`106-android-xr-static-render-cpu-reduction.md`](106-android-xr-static-render-cpu-reduction.md),
  "Post-Validation Rollback".
- Revert commit: `d0c5161` (`Restore XR per-eye command submission`).
- Risky optimization: `264c723` (`Submit Quest XR stereo eyes together`).
- Prior multiview contract: [`077-multiview-render-contract.md`](077-multiview-render-contract.md).
- Shared threading / future overlap boundary:
  [`062-shared-threading-topology.md`](062-shared-threading-topology.md).
