# Issue 001 — Z-fighting on thin surface decorations (lily pad, snow layer) when viewed from high altitude

Status: open (investigated, no fix landed)
Reported on: Android XR / Quest; likely also reproducible on desktop at sufficient altitude
Severity: visual correctness (not a crash / not a perf regression)

## Summary

Thin block-model decorations that rest a small distance above another surface —
**lily pads** (`0.0156` block above the cell above water) and **snow layers**
(`0.125` block above the block beneath) — flicker / z-fight against the surface
they sit on when the camera is far away, i.e. when the player is **very high up
looking down**. The two surfaces alternate per-pixel/per-frame as if they were
coplanar.

The root cause is **depth-buffer precision starvation at large view distance**,
not literal coplanar geometry. The decoration-to-surface vertical gap is real but
tiny (~0.125 block), and beyond a few hundred blocks of view distance the 24-bit
forward-Z depth buffer can no longer resolve a gap that small, so the surfaces
quantize to the same depth intermittently and fight.

This is altitude-correlated precisely because a truly coplanar bug would fight at
ground level too. It shows up on Quest first because continuous head motion makes
the flicker far more perceptible (and mobile depth is often effectively lower
precision), but the same precision curve applies to desktop.

## Symptom

- Fly/teleport high above terrain and look down.
- Lily pads on water and snow layers on stone/dirt/grass shimmer, with the
  decoration surface and the surface below alternating.
- Worsens with altitude (view distance). Not visible up close.

## Mechanism (why it happens)

### 1. The geometry gaps are small but nonzero (NOT coplanar)

The engine is fully block-model-JSON-driven — geometry for these blocks comes
verbatim from the vanilla 1.17.1 model files, and vertex Y is
`world_y + face.from|to[1] / 16` (`native/crates/mclone-mesh/src/builder.rs:657`,
corners from `textured_face_corners` at `builder.rs:1160`).

- **Lily pad** — `reference/minecraft-1.17.1/src/assets/minecraft/models/block/lily_pad.json`
  is a zero-thickness horizontal quad at `y = 0.25px = 0.0156` block, placed in
  the cell **above** the water block. Native water renders its top surface at
  `MAX_FLUID_HEIGHT = 8/9 ≈ 0.888` block (`builder.rs:678`, vanilla-correct).
  Net vertical gap lily-pad-top → water-top ≈ **0.128 block**.
- **Snow layer** — `.../models/block/snow_height2.json` is a 2px slab; its top
  face is **0.125 block** above the top face of the supporting block. Layer N →
  top at `2*N px`.

Backface culling is enabled on the chunk pipeline
(`native/crates/mclone-render/src/chunk.rs:1952` `cull_mode: Some(Face::Back)`),
so this is **not** the lily pad's own up/down faces self-fighting — from above
the down-face is culled. It is each decoration's top face fighting the surface
~0.125 block below it.

### 2. The depth buffer cannot resolve ~0.125 block at altitude

Confirmed render configuration (shared by desktop and XR/multiview paths):

| Property | Value | Location |
| --- | --- | --- |
| Depth format | `Depth24Plus` (24-bit) | `native/crates/mclone-render/src/chunk.rs:33`; mirror `color_profile.rs:4` |
| Depth compare | `LessEqual`, forward-Z (no reversed-Z anywhere) | `chunk.rs:1958`, `:2272`; `far_lod.rs:182`; `entity.rs:755` |
| Near plane | `0.05` | desktop `mclone-render-session/src/camera.rs:1291`; XR `mclone-xr-scene/src/lib.rs:135` (`XR_NEAR`) |
| Far plane | XR `700.0` fixed; desktop `700 + render_distance*128` (up to ~2748) | `mclone-xr-scene/src/lib.rs:136` (`XR_FAR`); `camera.rs:1292` |
| Depth bias | none — `bias: Default::default()` everywhere | `chunk.rs:1960`, `:2274`; `far_lod.rs:184`; `entity.rs:757`; `selection_outline.rs:201`,`:528` |
| Mesher anti-z-fight epsilon | `LIQUID_EPSILON = 0.001`, **fluids only** | `builder.rs:679` (not applied to cutout/solid decorations) |

For a forward-Z 24-bit buffer, world-space depth granularity at view distance `z`
is approximately:

```
Δz ≈ z² · (far − near) / (near · far) / 2²⁴
```

Because `near = 0.05` dominates the `(far−near)/(near·far)` term (≈ 20 for both
XR and desktop far values), this reduces to roughly `Δz ≈ 1.2e-6 · z²`:

| View distance (blocks) | Depth granularity | vs. 0.125-block decoration gap |
| --- | --- | --- |
| 240 | ~0.07 block | OK (marginal) |
| ~340 | ~0.14 block | swallows the gap → flicker begins |
| 500 | ~0.30 block | fights badly |

Key consequence: the **far plane barely matters** — `near = 0.05` is the
dominant term — so desktop has essentially the same precision curve as XR and
would show the same flicker at sufficient altitude. Quest just makes it far more
visible due to head motion.

### 3. Separate (latent) precision issue: absolute-world-space f32 rendering

The user's original hypothesis was "we don't apply proper float scaling to the
world." That instinct points at a **real but separate** bug:

Geometry is transformed in **absolute world space, entirely in f32**:

- Vertices stored as absolute world coords: `world_x as f32 + corner`
  (`builder.rs:657`), vertex format `Float32x3` (`chunk.rs:2234`,
  struct `mclone-mesh/src/data.rs:55`).
- The camera is folded into an f32 `view_projection` and **never subtracted from
  vertices on the CPU** (`chunk.rs:230`: `view_projection = projection * view`,
  `view = Mat4::from_rotation_translation(orientation, eye).inverse()`).
- The simulation camera is f64 (`Vec3d`) but is truncated to f32 *before* the
  view matrix is built (`glam_vec3_from_vec3d`,
  `mclone-render-session/src/mesh_inputs.rs:209`).
- The shader just does `view_projection * vec4(position, 1.0)`
  (`native/crates/mclone-render/src/shaders/chunk_textured.wgsl:105`); `camera_position`
  is used only for fog distance, not position rebasing. No per-chunk offset
  uniform or push constant exists.

**Important scoping:** this bites with **horizontal distance from the world
origin**, not pure altitude. At X ≈ 1,000,000 the f32 ULP is ~0.06 block (≈ the
1/16 sub-block grid), so sub-block geometry collapses. At Y ≈ 300 near spawn, f32
is fine (ULP ~3e-5). So this is a genuine latent bug worth fixing, but it is
**not** the primary driver of the altitude-triggered flicker reported here. If
the player flies both high **and** far out, the two effects stack.

## Reference: what vanilla Minecraft 1.17.1 does

1. **Camera-relative rendering.** Vanilla translates every chunk section to be
   relative to the camera before rendering (model-view offset of
   `chunkOrigin − cameraPos`), keeping vertex magnitudes small and f32-precise.
   This engine does the opposite (see §3). This is the mitigation the user's
   "float scaling" intuition refers to.
2. **Backface culling + geometry sitting proud** of the surface below — this
   engine already matches (`cull_mode: Some(Back)`; decoration models carry the
   same `from`/`to` offsets).
3. **Honest caveat:** vanilla 1.17 also uses a 24-bit forward-Z depth buffer with
   `near ≈ 0.05`, so vanilla is **not** immune to this precision limit. It mostly
   avoids the reported symptom because (a) the 1.17 world height cap bounds how
   far down you can look, and (b) camera-relative rendering prevents the
   horizontal precision collapse. An XR clone where the player can get much
   higher exposes the depth-precision limit more directly.

Relevant vanilla sources:
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/WaterlilyBlock.java`
  (collision `Block.box(1,0,1,15,1.5,15)`; render offset lives only in the model JSON).
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/SnowLayerBlock.java`
  (`SHAPE_BY_LAYER`, height `2*layers` px).
- `.../assets/minecraft/models/block/lily_pad.json`, `.../snow_height2.json`.

## Implementation options

Ranked. Not yet implemented — this doc is the problem statement only.

### Option 1 — Reversed-Z + `Depth32Float` (recommended, durable fix)

Swap to a reversed-Z projection with a floating-point depth buffer:
- Depth format `Depth24Plus` → `Depth32Float` (`chunk.rs:33`, `color_profile.rs:4`).
- Projection: map near→1, far→0 (swap near/far or use an infinite-far reversed
  form). Applies to both the desktop `Mat4::perspective_rh` builder (`chunk.rs:98`,
  `:231`, `:312`) and the XR `xr_fov_to_projection_rh` matrix in
  `mclone-xr-host/src/lib.rs:1673`.
- Depth clear value `1.0` → `0.0`; `CompareFunction::LessEqual` →
  `GreaterEqual` at every depth-stencil site.
- **XR guardrail:** must be applied to *both* the per-eye and the multiview paths,
  and each eye must keep its own projection (see `docs/platforms.md` /
  CLAUDE.md "XR render-path guardrail").

Pros: near-uniform depth precision across the whole range; essentially eliminates
this whole class of z-fighting; standard modern approach. Cons: touches every
depth pipeline + both render paths; needs headless + Quest validation.

### Option 2 — Push the near plane out (cheap partial mitigation)

Raise `near` from `0.05` toward `0.2`–`0.5`. Because `near` dominates the
precision term, this yields ~4–10× granularity improvement for a near-one-line
change (`camera.rs:1291`, `XR_NEAR` in `mclone-xr-scene/src/lib.rs:135`).
Cons: risks clipping blocks/hands very close to the face, especially in VR — a
tradeoff, not a clean win. Could be combined with Option 1 or used as an interim.

### Option 3 — Per-decoration depth epsilon (surgical band-aid)

Mirror the existing `LIQUID_EPSILON` (`builder.rs:679`) for thin top-surface
decorations: nudge the decoration's top face up by a small epsilon in
`textured_face_corners` / `add_textured_face` (`builder.rs:1160`, `:633`).
Pros: cheapest, most localized, no pipeline/XR churn. Cons: a band-aid — only
helps the specific blocks touched, doesn't fix the general precision limit, and
diverges the mesh from vanilla model geometry.

### Option 4 — Camera-relative meshing/rendering (fixes the §3 latent bug)

Subtract the camera or chunk origin from vertices (in f64 on the CPU), upload
small offsets, and keep only rotation/projection in the GPU matrix — matching
vanilla. Fixes the absolute-world f32 precision loss (the user's "float scaling"
hypothesis), which is **orthogonal** to the altitude flicker. Larger change;
warrants its own tactical. Worth doing regardless for correctness far from spawn.

## Recommendation

- Land **Option 1** (reversed-Z + `Depth32Float`) as the durable fix for the
  reported altitude z-fighting.
- Track **Option 4** (camera-relative rendering) as a separate tactical for the
  horizontal-distance precision bug — it does not fix this symptom on its own but
  is a real correctness gap.
- Options 2 and 3 are viable interim mitigations if a full depth-format change is
  deferred.

## Investigation notes

- Depth/projection config, geometry meshing, and coordinate-space handling were
  confirmed by direct source inspection (file:line references throughout).
- No changes were made during investigation.
