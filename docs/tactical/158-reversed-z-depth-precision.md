# 158: Reversed-Z Depth Precision (thin-decoration z-fighting at altitude)

Status: proposed 2026-07-08. Standalone fix for the depth-buffer precision
z-fighting diagnosed in
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

## Fix options

### Option A — Reversed-Z + `Depth32Float` (recommended)

Map near→1, far→0 with a float depth buffer:

- Depth format `Depth24Plus` → `Depth32Float` (`chunk.rs:33`, `color_profile.rs:4`,
  and the runtime assert at `frame_render.rs:245`).
- Projection: reversed form (swap near/far, or an infinite-far reversed
  perspective). Applies to the desktop `perspective_rh` builders (`chunk.rs:98`,
  `:231`, `:312`) and the XR `xr_fov_to_projection_rh` matrix
  (`mclone-xr-host/src/lib.rs:1673`, near/far mapping at `:1690`).
- Depth clear value `1.0` → `0.0` at every depth attachment clear site.
- `CompareFunction::LessEqual` → `GreaterEqual` at every depth-stencil state
  (`chunk.rs:1958`,`:2272`; `far_lod.rs:182`; `entity.rs:755`;
  `selection_outline.rs:199`,`:526`).
- Sky / any far-plane geometry that relies on `depth == 1.0` must move to the
  reversed convention.

Reversed-Z's precision boost comes largely from the float buffer's exponent
distribution; `Depth32Float` + reversed is the standard pairing. Pros: durable,
near-uniform precision; kills the whole class. Cons: touches every depth pipeline
and both render paths; needs headless + Quest validation.

### Option B — Push the near plane out (cheap interim)

Raise `near` from `0.05` toward `0.2`–`0.5`. Because `near` dominates the
precision term, this alone buys ~4–10× granularity for a near-one-line change
(`camera.rs:1291`, `XR_NEAR` in `mclone-xr-scene/src/lib.rs:135`). Cons: risks
clipping blocks / hands very close to the face, especially in VR — a tradeoff,
not a clean win. Usable as an interim or in combination with A.

### Option C — Per-decoration depth epsilon (surgical band-aid)

Mirror the existing `LIQUID_EPSILON` (`mclone-mesh/src/builder.rs:679`) for thin
top-surface decorations: nudge the decoration's top face up a hair in
`textured_face_corners` / `add_textured_face` (`builder.rs:1160`, `:633`). Pros:
cheapest, most localized, no pipeline/XR churn. Cons: band-aid — only the touched
blocks, doesn't fix the general precision limit, and diverges mesh geometry from
the vanilla model.

## Recommendation

Land **Option A** as the durable fix. Options B and C are viable interim
mitigations if a full depth-format change is deferred; B is the fastest
temporary relief.

## Slices

### Slice 0 — Baseline capture

Capture a high-altitude look-down over snow / lily-pad terrain on the current
build so before/after can be compared. See Validation for the capture note.

### Slice 1 — Reversed-Z + Depth32Float (Option A), chunk terrain

- Depth format + clear value + compare flip across the chunk pipelines.
- Reversed projection for desktop `perspective_rh` builders and the XR
  `xr_fov_to_projection_rh` matrix.
- Sky/far-plane depth convention updated.
- Validate: high-altitude before/after over snow, both per-eye and multiview;
  each eye keeps its own view/projection (XR guardrail).

### Slice 2 — Remaining depth pipelines

Flip far LOD (`far_lod.rs`), entity actors (`entity.rs`), and selection outline
(`selection_outline.rs`) to the reversed convention so they depth-test correctly
against the chunk buffer. Outline write is already disabled; only its compare
needs the flip.

### Slice 3 — (optional) Retire interim mitigations

If Option B/C were applied as interim relief, revert them once A lands (restore
`near = 0.05`, drop any decoration epsilon) so VR near-clipping and vanilla mesh
geometry are preserved.

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
  selection outline, sky at the far plane):

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
  selection outline, entities, and sky at the far plane all render correctly.
- Both per-eye and multiview paths carry the fix; each eye keeps its own
  view/projection.
- Existing desktop + XR smokes remain green; no measurable frame-time regression.

## Risks / watch-items

- **Every depth site must flip together.** A mixed forward/reversed pipeline
  (one pass `LessEqual` against a reversed buffer) inverts occlusion. Audit all
  depth-stencil states, clear values, and any shader that reads/writes
  `@builtin(position).z` or compares against `1.0`/`0.0`.
- **Sky / far-plane geometry** that depends on `depth == 1.0` (far) must move to
  `0.0`.
- **XR projection matrix** is runtime-provided asymmetric FOV via
  `xr_fov_to_projection_rh`; apply the reversed mapping there too, not just the
  glam desktop builder, and keep per-eye data independent (guardrail).
- **`Depth32Float` support / cost:** universally available on desktop and Quest;
  32-bit depth is slightly more bandwidth than 24-bit but negligible here. Keep
  the `Depth32Float` test path already present at `frame_render.rs:1705` in mind
  as the migration reference.
- **Do not conflate with 156.** Far-from-origin collapse is a separate fix; keep
  validation of the two apart so neither is mistaken for the other.
