# 156: Camera-Relative Chunk Rendering (world-distance float precision)

Status: proposed 2026-07-08. Standalone fix for the far-from-origin float
precision degradation diagnosed in
[`../issues/001-thin-decoration-z-fighting-at-altitude.md`](../issues/001-thin-decoration-z-fighting-at-altitude.md)
(§3, "absolute-world-space f32 rendering") and Option 4 of that issue.

Workstream: native Rust rendering (`mclone-mesh`, `mclone-render`,
`mclone-render-session`), applied to both the per-eye and XR multiview paths.

## Goal

Terrain and world geometry are currently transformed in **absolute world space,
entirely in f32**. As the player walks far from the world origin (order 10^6
blocks), the large-magnitude vertex coordinates lose f32 precision, geometry
snaps to a coarse grid, and depths collapse — surfaces shimmer / z-fight and the
world visibly degrades.

This tactical makes chunk (and secondary world) rendering **camera-relative**:
vertices are stored section-local, the big `world − camera` subtraction is done
once per section in f64 on the CPU, and the GPU only ever sees small-magnitude
coordinates. This is what vanilla Minecraft does (per-section model-view offset
of `chunkOrigin − cameraPos`), and it keeps f32 precision constant regardless of
how far the player travels.

### Why it degrades (confirmed by source inspection)

- Vertices are stored as absolute world coords in f32:
  `world_x as f32 + corner[..]` — `native/crates/mclone-mesh/src/builder.rs:657`
  (corners from `textured_face_corners`, `builder.rs:1160`). Vertex format is
  `Float32x3` (`native/crates/mclone-render/src/chunk.rs:2234`; struct
  `native/crates/mclone-mesh/src/data.rs:55`).
- The camera is folded into an f32 `view_projection` and **never subtracted from
  vertices on the CPU** — `chunk.rs:230` (`view_projection = projection * view`,
  `view` = inverse of `from_rotation_translation(orientation, eye)`).
- The f64 sim camera (`Vec3d`) is truncated to f32 *before* the matrix is built:
  `glam_vec3_from_vec3d` (`native/crates/mclone-render-session/src/mesh_inputs.rs:209`).
- The shader just does `view_projection * vec4(position, 1.0)`
  (`native/crates/mclone-render/src/shaders/chunk_textured.wgsl:105`), with
  `world_position` used **only** for fog distance (`chunk_textured.wgsl:120`).

At X ≈ 1,000,000 the f32 ULP is ~0.06 block — right at the 1/16 sub-block grid,
so sub-block geometry collapses and the GPU matrix multiply of a ~1e6 vertex by a
~-1e6 translation is catastrophic-cancellation territory.

## Scope and non-goals

**In scope:** make world geometry rendering camera-relative so f32 precision is
constant with distance from origin.

- Slice 1 (primary): chunk terrain (`chunk_textured` + `chunk_textured_multiview`).
- Slice 2: the other pipelines that submit absolute-world vertices —
  selection outline (`selection_outline.rs`), far LOD (`far_lod.rs`), entity
  actors (`entity_actor` + `entity_actor_multiview`). Mechanical repeats of the
  same pattern; needed for a *fully* clean world far out.
- Slice 3 (optional follow-up): shrink the chunk vertex position to packed
  local ints now that coordinates are bounded.

**Explicit non-goal — this does NOT fix the near-spawn altitude z-fighting on
thin decorations (lily pad / snow).** That symptom is depth-*buffer* precision
(24-bit forward-Z, `near = 0.05`), tracked as
[`../issues/001-thin-decoration-z-fighting-at-altitude.md`](../issues/001-thin-decoration-z-fighting-at-altitude.md)
Option 1 (reversed-Z + `Depth32Float`). Near spawn the vertex coordinates are
already f32-precise, so camera-relative rendering leaves that case unchanged. Do
not expect the high-altitude-over-snow flicker *at spawn* to disappear from this
work; it is a separate fix. (Far from origin, this tactical does clean up the
distance-driven shimmer, which is a different mechanism.)

## Related docs

- [`../issues/001-thin-decoration-z-fighting-at-altitude.md`](../issues/001-thin-decoration-z-fighting-at-altitude.md) — parent issue, §3 + Option 4.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) — shared crate ownership.
- [`../platforms.md`](../platforms.md#validation-policy) — validation gates / offscreen screenshot host.
- CLAUDE.md "XR render-path guardrail" — per-eye vs multiview parity, per-view data invariant.

## Design

Two halves; the CPU does two cheap things, the GPU keeps doing today's per-vertex
math.

1. **Store vertices section-local at mesh-build time.** In `add_textured_face`
   (`builder.rs:633`/`:657`), store `(world_x - section_origin_x) as f32 + corner`
   instead of `world_x as f32 + corner`. Section origin is an exact integer known
   at build time; local coords land in ~`0..RENDER_SECTION_HEIGHT`, where f32 ULP
   is ~1e-6 block regardless of world position. This is the part that actually
   fixes the degradation — the current precision loss is baked in at store time,
   so re-translating the camera alone would not help. Mesh contents become
   invariant to camera position, so **walking does not trigger re-meshing.**
2. **Per drawn section, pass an f32 offset `= section_origin − camera`, computed
   in f64.** Draws are already issued per `RenderSectionKey` with their own vertex
   buffer (`chunk.rs:1687`: `set_vertex_buffer` + `draw_indexed`). Push constants
   are currently unused (`push_constant_ranges: &[]`, `chunk.rs:2032`), so add a
   small push constant (or per-section dynamic uniform) carrying the offset. The
   offset magnitude is bounded by render distance (a few thousand blocks), so its
   f32 ULP (~1e-4) is far below a pixel.
3. **Make the uploaded matrix rotation+projection only** (camera at origin) and
   compute clip space in the shader as
   `view_projection_rot * vec4(local_position + section_offset, 1.0)`. Both
   `local_position` and `section_offset` are small → the sum is small → precise.
4. **Fog / `world_position`:** the fog varying becomes the camera-relative
   position (`local_position + section_offset`); `fog_distance = length(that)`
   and the `camera_position` uniform term drops out. Fog gets simpler and *more*
   precise. Verify no other consumer of the `world_position` varying exists
   (confirmed: fog only, across chunk + entity shaders).

WGSL/GPU has no f64, so the `world − camera` subtraction must be CPU-side — but
it is once per section per frame (a few hundred f64 vec3 subtracts), not
per-vertex. Runtime cost is negligible; this can be a slight net win.

## Slices

### Slice 0 — Baseline capture (before any code change)

Capture reference screenshots so before/after can be compared. See Validation.
Save near-spawn and far-origin captures to `/tmp` and record the paths in this
doc when the slice runs.

#### Baseline evidence — bug reproduced on the current build (captured 2026-07-08)

The far-from-origin precision collapse is confirmed with the offscreen
screenshot host. Key capture gotcha: the plain `--screenshot` (idle) path frames
with a fixed spawn eye height/pitch that misses flat terrain far out (you get an
all-sky frame — e.g. `/tmp/mclone-camrel-farout-before.png`, only 4 drawn
sections). Use `--screenshot-scripted-interaction true` instead — it aims a
top-down close-up at a real surface block near the scene center, so it frames
terrain reliably at any coordinate. Capture recipe:

```
cargo run --manifest-path native/Cargo.toml -p mclone-native-client \
  --bin mclone-native-client -- \
  --screenshot /tmp/mclone-camrel-scripted-<CX>.png \
  --screenshot-scripted-interaction true \
  --width 1280 --height 720 --seed 12345 --chunk-x <CX> --chunk-z 0 \
  --render-distance 3 --day-time 6000 --freeze-time
```

`--chunk-x CX` puts the framed block at world X ≈ `CX*16`. f32 ULP grows with
that magnitude, so the artifact scales with distance:

| Capture (`--chunk-x`) | World X | f32 ULP | Result |
| --- | --- | --- | --- |
| `0` (control) | ~8 | ~1e-6 blk | Clean: undistorted mobs, proper flower/grass crosses, crisp selection outline. `/tmp/mclone-camrel-scripted-0.png` |
| `250000` | ~4,000,000 | ~0.25 blk | Mostly coherent in a still; artifact is sub-block distortion + temporal depth flicker. `/tmp/mclone-camrel-scripted-250000.png` |
| `1000000` | ~16,000,000 | ~1 blk | **Unambiguous:** cross-quad decorations (flowers, grass tufts) sheared into diagonal slivers, flower petals broken/offset, cow warped — all sub-block geometry snapped to the integer grid. `/tmp/mclone-camrel-scripted-1000000.png` |

`--chunk-x 2000000` (X≈32M) is past the ±29,999,984 world border and has no
surface, so ~16M is the practical extreme. Because worldgen is f64/deterministic,
the same terrain shape is generated correctly at every coordinate — the collapse
is purely the render-side f32 vertex/depth transform, which is exactly what this
tactical fixes. These are the **before** images; the matching **after** images
(same commands post-fix) go here when Slice 1 lands, and should match the `0`
control quality at all magnitudes.

Note: a mid-magnitude still (~1M blocks, the originally-suggested `--chunk-x
62500`) shows only subtle/temporal artifacts; use ~16M for a still that reads
clearly. The near-spawn thin-decoration altitude z-fighting from Issue 001 is a
different, depth-buffer symptom and is not what these captures show.

### Slice 1 — Chunk terrain camera-relative

- Mesher: section-local vertex positions (`builder.rs` `add_textured_face`).
- Render: per-section offset push constant + set per draw
  (`chunk.rs` draw loop around `:1687`; pipeline layout `:2032`).
- View matrix: rotation-only variant (`chunk.rs:230` / render-pose build).
- Shaders: `chunk_textured.wgsl` + `chunk_textured_multiview.wgsl` — add offset
  before transform, rework fog to camera-relative.
- Validate: near-spawn no-regression A/B + far-origin fix demonstration; both
  per-eye and multiview.

### Slice 2 — Secondary world pipelines

Apply the same local-vertex + per-draw-offset pattern to
`selection_outline.rs`, `far_lod.rs`, and the entity actor path
(`entity_actor.wgsl` + `entity_actor_multiview.wgsl`). Each is a mechanical
repeat. Entity positions are few and near the player, so lower urgency, but they
still snap far from origin.

### Slice 3 — (optional) Packed local vertex positions

With coordinates bounded to a section, the position attribute can drop from
`Float32x3` to packed 16-bit local ints, shrinking the chunk vertex and upload
bandwidth. Follow-up only; not required for correctness.

## Validation

Two distinct checks. The user has accepted manual verification as a fallback;
the offscreen `--screenshot` host makes automated A/B cheap where it works.

The existing gate pattern (`native:desktop-offscreen:smoke`) is:

```
cargo run --manifest-path native/Cargo.toml -p mclone-native-client \
  --bin mclone-native-client -- --screenshot /tmp/OUT.png \
  --width 2560 --height 1600 --seed 12345 --chunk-x CX --chunk-z CZ \
  --render-distance 2 --day-time 6000 --freeze-time
```

### V1 — No-regression near spawn (world still looks the same)

Same seed/pose at `--chunk-x 0 --chunk-z 0`, before vs after Slice 1. The images
must be pixel-near-identical (fog rework is the only expected source of tiny
differences; confirm none are visible).

- Before: `/tmp/mclone-camrel-spawn-before.png`
- After:  `/tmp/mclone-camrel-spawn-after.png`

### V2 — Fix demonstration far from origin

The visual payoff of *this* tactical appears far out, not at altitude near spawn.
Use the `--screenshot-scripted-interaction true` recipe from the Slice 0 baseline
evidence above (it frames a top-down close-up on a real surface block, reliably,
at any coordinate — the plain idle `--screenshot` path misses flat far terrain).
Cross-quad decorations (flowers, grass tufts) are the most sensitive indicator.

- Before (current build): captured — see the Slice 0 evidence table.
  `/tmp/mclone-camrel-scripted-0.png` (control) vs
  `/tmp/mclone-camrel-scripted-1000000.png` (X≈16M, gross collapse).
- After (post-fix, same commands): all magnitudes should match the `--chunk-x 0`
  control quality. Save as `/tmp/mclone-camrel-scripted-<CX>-after.png`.

Use a large coordinate (`--chunk-x 1000000`, X≈16M, ~1-block ULP) for a still
that reads clearly; mid-magnitudes only shimmer temporally. Manual capture in a
live session is also acceptable.

### V3 — Existing smokes stay green

Run the standard sweep after each slice, both paths:

```
pnpm native:desktop-offscreen:smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
# XR (Windows host): pnpm native:xr:windows:smoke:connected
```

Per the native-validation policy, **look at every screenshot before moving on** —
do not batch. If a headless capture fails for lack of a GPU adapter, rerun with
elevation before treating GPU validation as blocked.

## Acceptance criteria

- V1: near-spawn before/after visually identical (no world-appearance regression).
- V2: far-origin (~1e6 blocks) terrain renders without the distance-driven
  snapping/shimmer present in the current build; matches near-spawn quality.
- V3: existing desktop + XR smokes remain green; no measurable frame-time
  regression (per-section f64 subtract is negligible).
- Both per-eye and multiview paths carry the fix; each eye keeps its own
  view/projection (XR guardrail).

## Precision target (why it works)

- Section-local coords (~0..384) → f32 ULP ~1e-6 block, constant with world
  position.
- Per-section offset bounded by render distance (~few thousand blocks) → f32 ULP
  ~1e-4 block, sub-pixel.
- Net: f32 clip-space precision no longer depends on distance from origin, so the
  order-1e6 degradation is eliminated.

## Risks / watch-items

- **XR parity:** must land per-eye and multiview together; never share mutable
  per-eye uniforms; keep each eye's view/projection independent (guardrail).
- **Fog rework:** the `world_position` varying changes meaning (world → camera-
  relative). Confirm fog is the only consumer and that distances match before/
  after (V1 catches this).
- **Secondary pipelines lag:** if Slice 2 is deferred, selection outline / far
  LOD / entities will still snap far out. Note this honestly rather than implying
  the whole world is fixed after Slice 1.
- **Do not conflate with Issue 001:** near-spawn altitude thin-decoration
  z-fighting is depth-buffer precision, not this. Keep them separate so neither
  fix is mistaken for the other during validation.
