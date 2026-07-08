# 158: Reversed-Z Depth Precision (thin-decoration z-fighting at altitude)

Status: **Slices 1–2 implemented 2026-07-08** — reversed-Z + `Depth32Float`
landed across all depth pipelines; desktop offscreen smoke + render/xr-host unit
tests green (no depth-ordering regression). XR in-headset validation still
pending on a Windows/Quest host. Accepted 2026-07-08 as **Option 1 (reversed-Z +
`Depth32Float`)** — the chosen and only approach; the interim mitigations
previously sketched here are dropped. Standalone fix for the depth-buffer
precision z-fighting diagnosed in
[`../issues/001-thin-decoration-z-fighting-at-altitude.md`](../issues/001-thin-decoration-z-fighting-at-altitude.md)
(Option 1). This is the implementation tactical for that issue; the issue doc is
the bug statement.

Workstream: native Rust rendering (`mclone-render`, `mclone-render-session`,
`mclone-xr-host`, `mclone-xr-scene`), applied to both the per-eye and XR
multiview paths.

## Goal

Thin surface decorations that rest a small distance above the surface beneath
them — **lily pads** (~0.128 block above the water below) and **snow layers**
(~0.125 block above the block below) — flicker / z-fight when viewed from far
away (player high up looking down). Reported on Quest; the same precision curve
applies to desktop at sufficient altitude.

The root cause is **depth-buffer precision starvation at large view distance**,
not coplanar geometry: the ~0.125-block gap is real but small, and a 24-bit
forward-Z buffer with `near = 0.05` cannot resolve it past a few hundred blocks
of view distance, so the two surfaces quantize to the same depth intermittently
and fight.

This tactical moves the depth pipeline to **reversed-Z with a floating-point
depth buffer**, which gives near-uniform precision across the whole range and
essentially eliminates this class of z-fighting.

### Distinct from tactical 156

This is **not** the far-from-origin float precision fix. Tactical
[`156-camera-relative-chunk-rendering.md`](156-camera-relative-chunk-rendering.md)
addresses *vertex-position* f32 loss with horizontal distance from spawn (Issue
001 §3 / Option 4). This tactical addresses *depth-buffer* resolution with view
distance at altitude (Issue 001 Option 1). They are orthogonal — near spawn the
vertices are already precise, so 156 leaves the altitude flicker unchanged, and
this fix does nothing for the 1e6-blocks-out collapse. Land them independently.

## Root cause (confirmed by source inspection)

Render configuration shared by desktop and XR/multiview:

| Property | Value | Location |
| --- | --- | --- |
| Depth format | `Depth24Plus` (24-bit) | `native/crates/mclone-render/src/chunk.rs:33`; mirror `color_profile.rs:4` |
| Depth compare | `LessEqual`, forward-Z (no reversed-Z) | `chunk.rs:1958`, `:2272`; `far_lod.rs:182`; `entity.rs:755`; `selection_outline.rs:199`,`:526` |
| Near plane | `0.05` | desktop `mclone-render-session/src/camera.rs:1291`; XR `mclone-xr-scene/src/lib.rs:135` (`XR_NEAR`) |
| Far plane | XR `700.0`; desktop `700 + render_distance*128` | `mclone-xr-scene/src/lib.rs:136` (`XR_FAR`); `camera.rs:1292` |
| Depth bias | none (`bias: Default::default()`) | `chunk.rs:1960`,`:2274`; `far_lod.rs:184`; `entity.rs:757`; `selection_outline.rs:201`,`:528` |
| Projection | `Mat4::perspective_rh` (0→1, forward) | `chunk.rs:98`,`:231`,`:312`; XR `xr_fov_to_projection_rh` in `mclone-xr-host/src/lib.rs:1673` |

For a forward-Z 24-bit buffer, world-space depth granularity at view distance `z`
is roughly `Δz ≈ z² · (far−near)/(near·far) / 2²⁴`. `near = 0.05` dominates the
middle term (≈ 20 for both XR and desktop far values), so `Δz ≈ 1.2e-6 · z²`:

| View distance | Depth granularity | vs. ~0.125-block gap |
| --- | --- | --- |
| 240 blocks | ~0.07 block | OK (marginal) |
| ~340 blocks | ~0.14 block | swallows the gap → flicker |
| 500 blocks | ~0.30 block | fights badly |

The far plane barely matters (near dominates), so this is not fixed by shrinking
far; and backface culling is already on (`chunk.rs:1952 cull_mode: Some(Back)`),
so it is not the decoration's own faces self-fighting — it is each decoration's
top face fighting the surface ~0.125 block below.

Vanilla note: MC 1.17 also uses 24-bit forward-Z with `near ≈ 0.05`, so vanilla
is not immune; it mostly avoids the symptom because the world-height cap bounds
how far down you can look. An XR clone where the player can fly much higher
exposes the precision limit, so a reversed-Z upgrade beyond vanilla is warranted.

## Fix: Reversed-Z + `Depth32Float`

Map near→1, far→0 with a float depth buffer. Reversed-Z's precision boost comes
largely from the float buffer's exponent distribution cancelling the perspective
`1/z` nonlinearity, so `Depth32Float` + reversed is the standard pairing — durable,
near-uniform precision that kills the whole class. It touches every depth pipeline
and both render paths, so the audit below enumerates *all* sites; the risk is a
partial flip (one pass left forward against a reversed buffer inverts occlusion),
not difficulty.

The two cheap alternatives were considered and rejected, recorded here so the
rationale is not lost:

- **Push the near plane out** (`near 0.05` → `0.2`–`0.5`): buys ~4–10× granularity
  for one line, but clips blocks and the player's own hands close to the face —
  worst on Quest, the exact platform this bug was reported on. Rejected as a net
  regression in VR, not a clean win.
- **Per-decoration depth epsilon** (mirror `LIQUID_EPSILON`): cheapest and most
  local, but only patches the two block types someone noticed, leaves the general
  precision limit intact, and diverges the lily-pad/snow mesh from the vanilla
  model (against the reference-porting policy). Rejected as a band-aid.

Because there is no interim mitigation, this lands atomically (per pipeline
group) and is validated as one change; there is nothing to revert afterward.

## Depth-site audit (2026-07-08)

Exhaustive sweep of `mclone-render`, `mclone-app-runtime`, `mclone-xr-*`, and the
XR app adapters. Every depth-stencil state, depth clear, depth texture format,
and projection builder is accounted for below; nothing else touches depth.

### 1. Depth format — `Depth24Plus` → `Depth32Float`

The format flows from one shared const plus two XR swapchain mirrors. Change the
constants; the pipeline/texture sites that read them follow automatically.

| Site | Action |
| --- | --- |
| `mclone-render/src/chunk.rs:33` `DEPTH_FORMAT` | change const (master) |
| `mclone-render/src/color_profile.rs:4` `DEFAULT_RENDER_DEPTH_FORMAT` | change const |
| `mclone-render/src/color_profile.rs:312` `assert_eq!(…, Depth24Plus)` | update assert to `Depth32Float` |
| `apps/mclone-native-client/src/xr_clear_smoke.rs:90` `XR_DEPTH_FORMAT` | literal `Depth24Plus` → `Depth32Float` (drives the OpenXR depth swapchain) |
| `apps/mclone-android-xr-client/src/lib.rs:145` `XR_DEPTH_FORMAT` | literal `Depth24Plus` → `Depth32Float` (drives the OpenXR depth swapchain) |
| `mclone-app-runtime/src/frame_render.rs:245-248` runtime assert; `:1705` test uses `Depth32Float` | follow the const automatically — verify still consistent (the `:1705` test becomes redundant with the new default) |

Follow automatically (read `DEPTH_FORMAT`, no literal edit): pipeline/texture
formats at `chunk.rs:1822`,`:1864`,`:1956`,`:2270`; `far_lod.rs:180`;
`entity.rs:753`; `selection_outline.rs:197`,`:524`; and the graphics-adapter
asserts `xr_clear_smoke/graphics_vulkan.rs:63`, `…/graphics_metal.rs:244`,
`android-xr-client/src/graphics_vulkan.rs:106` (they compare against
`mclone_render::chunk::DEPTH_FORMAT`).

### 2. Depth compare — `LessEqual` → `GreaterEqual` (6 pipelines)

`chunk.rs:1958` (per-eye textured); `chunk.rs:2272` (multiview textured);
`far_lod.rs:182`; `entity.rs:755`; `selection_outline.rs:199` (per-eye);
`selection_outline.rs:526` (multiview). These are the only `CompareFunction`
depth states in the tree.

### 3. Depth clear value — `1.0` → `0.0` (5 clear sites)

| Site | Note |
| --- | --- |
| `chunk.rs:373` `ChunkRenderTarget::clear_depth` default | per-eye main pass |
| `chunk.rs:2499` `ChunkMultiviewRenderTarget::clear_depth` default | multiview main pass |
| `far_lod.rs:259` `LoadOp::Clear(1.0)` | early background pass; owns its own depth clear |
| `mclone-xr-host/src/lib.rs:1582` `LoadOp::Clear(1.0)` | XR eye pass |
| `apps/mclone-native-client/src/headless.rs:427` `LoadOp::Clear(1.0)` | headless capture pass |

Load-not-clear depth passes need **no** clear change — they inherit the buffer the
clearing pass wrote and only need their compare flip (§2): entity actors
(`entity.rs:415`, `LoadOp::Load`), selection outline (`selection_outline.rs:280`,
`:336`), and the late translucent chunk passes (the `with_loaded_depth` path,
`chunk.rs:2359`/`:3311` via `depth_load_op()`).

### 4. Projection — forward (near→0, far→1) → reversed (near→1, far→0)

- Desktop `Mat4::perspective_rh` builders: `chunk.rs:98`, `:231`, `:312`, **and
  the test helper `:4714`** (the original draft listed only the first three).
  Swap near/far, or use an infinite-far reversed perspective.
- XR `xr_fov_to_projection_rh` (`mclone-xr-host/src/lib.rs:1673`): apply the
  reversed mapping in the matrix entries at `:1696` (`-far/(far-near)`) and
  `:1699` (`-(near*far)/(far-near)`).
- XR projection test `mclone-xr-host/src/lib.rs:1749-1752` compares
  `xr_fov_to_projection_rh` against `Mat4::perspective_rh(…)`; it must be updated
  to the reversed reference (both flipped) so it still passes.

### 5. Verified no-change (do **not** touch)

- **Sky is exempt.** `sky_render.rs` pipelines use `depth_stencil: None`
  (`:155`, `:657`) and passes use `depth_stencil_attachment: None` (`:385`,
  `:482`). The sky dome is drawn color-only *before* the chunk pass clears depth
  and never depth-tests, so it does **not** rely on `depth == 1.0`. This corrects
  the original draft, which listed the sky far-plane convention as a required
  change and a risk — reversed-Z leaves it untouched.
- No-depth passes (also exempt): GUI (`gui.rs`, all `depth_stencil_attachment:
  None`), screen effects (`screen_effect.rs`), `gpu_timestamps.rs`, and the web
  canvas present pass (`web_canvas.rs:2470`).

## Slices

### Slice 0 — Baseline capture

Capture a high-altitude look-down over snow / lily-pad terrain on the current
build so before/after can be compared. See Validation for the capture note.

### Slices 1 + 2 — Reversed-Z + Depth32Float, all depth pipelines (done 2026-07-08)

Slices 1 and 2 **landed together in one atomic commit**. They cannot be
separated: the projection matrix and the depth buffer are shared across chunk
terrain, far-LOD, entity actors, and the selection outline, so reversing chunk
while leaving the others on `LessEqual` would invert their occlusion against the
reversed buffer (the "every depth site must flip together" guardrail). A
chunk-only intermediate is a visibly broken frame, so the whole flip is one
change.

Implemented:

- **Format** (§1): `DEPTH_FORMAT` and `DEFAULT_RENDER_DEPTH_FORMAT` →
  `Depth32Float`; `color_profile` default-config assert updated; both
  `XR_DEPTH_FORMAT` swapchain consts (`xr_clear_smoke.rs`, `android-xr-client`)
  → `Depth32Float`.
- **Compare** (§2): all 6 pipelines `LessEqual` → `GreaterEqual` (chunk ×2,
  far-LOD, entity, selection outline ×2).
- **Clear** (§3): all 5 depth clears → `0.0` via the new
  `chunk::REVERSED_Z_DEPTH_CLEAR` const (chunk per-eye + multiview defaults,
  far-LOD, XR eye pass, native-client headless pass).
- **Projection** (§4): new shared `chunk::reversed_z_perspective_rh` applies a
  `REVERSE_Z` remap (clip `z' = w - z`) to glam's `perspective_rh` at all four
  desktop builders (incl. the `:4714` test helper); the XR
  `xr_fov_to_projection_rh` matrix carries the reversed z/w-row entries directly,
  and its unit test cross-checks against `REVERSE_Z * perspective_rh`.
- **Sky** (§5): untouched, as the audit prescribed.

Validated on desktop: `pnpm native:desktop-offscreen:smoke` renders correct
occlusion, entities, water/leaf translucency, and sky (screenshot reviewed);
`mclone-render` (126) and `mclone-xr-host` (3) unit tests pass.

There was no interim-mitigation slice: Option 1 is the only approach, so nothing
was staged behind a temporary hack and nothing needs reverting.

### Remaining — XR in-headset validation

Not yet done (requires a Windows/Quest host per the validation matrix): confirm
the fix in-headset over snow / lily-pad terrain, both eyes, and confirm the
OpenXR runtime allocates a 32-bit float depth swapchain (see Risks). The desktop
lane above is the no-regression gate; the altitude flicker itself needs the
elevated look-down pose noted under Validation.

## Validation

- **Primary (manual acceptable):** high above snow-layer / lily-pad terrain,
  look down, before vs after. Current build flickers; reversed-Z build is stable.
  The issue was originally reported by manual Quest observation, so manual
  verification in-headset (or a live desktop session flown high) is the pragmatic
  gate. The flicker is temporal, so a still A/B is weaker than sweeping the camera
  — capture a short clip or several frames if automating.
- Headless capture caveat: the offscreen `--screenshot` host frames near the
  surface, not a high look-down, so it does not naturally reproduce this. An
  elevated look-down pose (extend the scripted-camera path used in 156's evidence,
  or a live session) is needed to capture it; do not expect the standard
  `native:desktop-offscreen:smoke` framing to show it.
- **No-regression:** run the standard sweep after each slice, both paths, and
  confirm no depth ordering regressions (terrain occlusion, transparency,
  selection outline, and sky still showing behind terrain — see the §5 note that
  sky carries no depth):

```
pnpm native:desktop-offscreen:smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
# XR (Windows host): pnpm native:xr:windows:smoke:connected
```

Per native-validation policy, look at every screenshot before moving on. If a
headless capture fails for lack of a GPU adapter, rerun with elevation.

## Acceptance criteria

- Thin-decoration z-fighting no longer visible from high altitude over snow /
  lily-pad terrain (manual or captured A/B).
- No depth-ordering regressions near spawn: terrain occlusion, translucency,
  selection outline, entities all render correctly, and sky still shows behind
  terrain (sky carries no depth attachment).
- Both per-eye and multiview paths carry the fix; each eye keeps its own
  view/projection.
- Existing desktop + XR smokes remain green; no measurable frame-time regression.

## Risks / watch-items

- **Every depth site must flip together.** A mixed forward/reversed pipeline
  (one pass `LessEqual` against a reversed buffer) inverts occlusion. The audit
  above is the checklist; none of the crate's WGSL shaders read/write
  `@builtin(position).z` or compare against `1.0`/`0.0` (verified — the only
  `@builtin(position)` uses are vertex outputs), so this is entirely a
  Rust-side pipeline/clear/projection change.
- **Sky is exempt (verified):** the sky pass owns no depth attachment (audit §5),
  so there is nothing to move to the reversed convention. Do not "fix" it.
- **XR projection matrix** is runtime-provided asymmetric FOV via
  `xr_fov_to_projection_rh`; apply the reversed mapping there too, not just the
  glam desktop builder, update its unit test, and keep per-eye data independent
  (guardrail).
- **OpenXR depth swapchain format:** the two `XR_DEPTH_FORMAT` consts drive the
  depth swapchain the runtime allocates. The Quest/OpenXR runtime must advertise
  a 32-bit float depth swapchain (`D32_SFLOAT`); confirm at session init and keep
  a fallback in mind if a runtime only offers 24-bit. This is the one genuinely
  runtime-dependent piece of the change.
- **`Depth32Float` support / cost:** universally available on desktop and Quest
  as a *render* depth format; 32-bit depth is slightly more bandwidth than 24-bit
  but negligible here. The `Depth32Float` test path already present at
  `frame_render.rs:1705` is the migration reference.
- **Do not conflate with 156.** Far-from-origin collapse is a separate fix; keep
  validation of the two apart so neither is mistaken for the other.
