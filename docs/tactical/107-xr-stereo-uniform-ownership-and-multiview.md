# 107: XR Stereo Uniform Ownership and Multiview

Status: active high-priority prerequisite; Slices A-C landed, Slice D desktop
proof landed. Created after `d0c5161` (`Restore XR per-eye command submission`)
rolled back the unsafe single-submit Quest XR optimization from `264c723`.

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

Move from proof-of-correctness one-submit to the real target:

- two-layer XR color/depth target or equivalent swapchain path;
- render pipelines with `multiview: Some(2)`;
- shader `@builtin(view_index)` selection for per-eye view data;
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
