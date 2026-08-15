# Procedural Horizon Surface Appearance

Topic: `procedural-horizon-surface-appearance`

Status: the material, texture, color, hydrology, and approximate-light pipeline
is documented. Tactical
[`276`](../tactical/276-procedural-horizon-surface-appearance.md) now makes the
original Mclone profile use its sampled biome recipe for grass tint, paints
interpolated wetland-pool water as well as river corridors, and derives all far
water color from ground-relative depth. Native Vulkan, synthetic per-eye
stereo, and headed browser/WebGPU pixels pass with shader tests, the complete
shared workspace check, and matched release performance receipts on
2026-07-28. Pack-native colormaps, footprint-aware material filtering,
decoration-lake summaries, flat water geometry, stored lighting, and GPU
timestamp instrumentation remain separate work.

Tactical
[`304`](../tactical/304-lod-frontier-and-near-field-voxel-convergence.md)
implements the near-field convergence step with the exact-frontier correction.
The spacing-one level now presents flat tops and cardinal risers with
worldgen-owned side strata, direction-specific active-pack faces, pack-native
grass tint, exact face shade, and shared sky-darken/lightmap inputs. Farther
rings remain smooth. Opaque procedural water remains the single visible water
owner through exact-painted chunks and uses one treatment across the near/far
material transition. Independent water geometry and decoration-lake summaries
remain separate work; subjective Human Review of the new terrain character is
pending.

Planned Tactical
[`309`](../tactical/309-procedural-horizon-lighting-and-seam-convergence.md)
owns the resulting appearance closeout as a phase-gated campaign. Its first
priority is the captured night failure: exact terrain darkens while much of
the smooth horizon, procedural water, and procedural vegetation remain near
daylight color. It then separately converges the voxel/smooth and exact/voxel
borders, with diagnostic albedo, environmental light, geometric shade, AO,
water-depth, and texture inputs plus Human Review after every phase. Seasonal
solar-path policy and independent water geometry remain outside that scope.

## Scope

This topic owns how the procedural-horizon surface looks after worldgen has
answered a coarse sample:

- which ground or water material represents the sample;
- how that material selects an existing block texture;
- how biome color, surface color, and texture detail combine;
- how coarse hydrology survives between sample vertices;
- how the heightfield is lit; and
- how appearance quality is measured without silently increasing the fixed LOD
  budget.

The toroidal residency, seam, exact/procedural handoff, and platform-hosting
contracts remain in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). Natural tree
appearance remains in [`lod-native-vegetation.md`](lod-native-vegetation.md).
Exact chunk rendering is not redefined here.

## Current Pipeline

The horizon does not need a new painted megatexture for ordinary ground. It
already has a material-texture pipeline:

```text
worldgen point/footprint query
        |
        v
visible block material + biome/hydrology semantics
        |
        v
fixed 32-float TerrainPreviewSample
        |
        +--> block-state id --> active-pack top/side faces + tint flags
        |
        +--> surface recipe --> worldgen-owned side/subsurface/body strata
        |
        v
spacing-one voxel shell / stitched smooth farther rings
        |
        +--> near: exact-style face shade + daylight + atlas texel
        |
        +--> far: approximate slope light + reduced atlas detail
        |
        v
analytic inland-water overlay --> fog/target transfer --> render target
```

The efficient default is therefore to improve classification and the compact
semantic fields, then shade them through the existing atlas. A separately
painted texture becomes useful only for information that cannot be represented
well by one material id plus interpolated fields, such as a future footprint
material mixture or a baked regional albedo cache.

### Surface classification

`mclone-worldgen` owns ground truth. For the original profile,
`mclone_overworld_macro_surface_top_material` and
`mclone_overworld_preview_visible_material` classify each requested point as a
raw block material such as grass, coarse dirt, sand, gravel, stone, snow, or
water. The visible-material query accounts for the final surface-quality stage;
it is not a renderer-authored color guess.

`TerrainPreviewSample` packs 32 `f32` values, including:

- surface and display height;
- macro and visible material ids;
- temperature, moisture, and other climate/terrain fields;
- signed river distance, visible river half-width, and channel influence;
- wetland and wetland-pool influence; and
- the original-profile biome recipe or the vanilla biome id.

The current material id is flat-interpolated across each drawn triangle. That
avoids inventing nonsensical fractional block ids, but it also means a coarse
triangle has one atlas sprite. Continuous fields such as river distance,
wetland-pool influence, light, and world position interpolate normally.

### Near voxel shell and side strata

Only the spacing-one level changes topology. Each one-block cell has a rounded
integer-height flat top plus reserved north, south, east, and west risers;
unexposed risers become degenerate triangles. The coarser levels remain the
stitched smooth heightfield.

Worldgen resolves each visible material and surface recipe to a compact
`top/upper-side/subsurface-depth/body` profile shared with generated WGSL.
Grass uses grass top, grass-block side, three dirt blocks, then stone. Sand,
gravel, clay, snow, water, and exposed-stone recipes select their own bounded
profiles. This is approximate untouched-natural-terrain semantics, not a
renderer-authored raw-id strata table or a cache of canonical columns.

### Texture selection and filtering

The scene maps each raw material id through the same baked asset catalog used
by ordinary block meshes. A compact uniform table distinguishes upward and
cardinal sprite rectangles plus grass-tint flags; it replaces the earlier
single `gui_icon_uv` representative. The renderer builds the atlas mip chain
once. Its sampler uses nearest magnification and linear minification/mipmap
filtering.

The fragment shader repeats the selected block sprite in world space with
`fract(world_xz)` and samples it with explicit gradients. Solid faces in the
spacing-one shell use exact-strength atlas texels. Their material character
transitions over the outer 32 cells toward the smooth far presentation, whose
texture contribution falls from `0.82` near one block per pixel toward `0.42`
at eight or more blocks per pixel. Water uses the same far-style treatment on
both sides of that transition so open ocean does not reveal the finest ring as
a square.

This path is fixed-resident and shared by native, browser, flat Android, and the
normal per-eye XR renderer. It does not allocate or upload a per-view painted
terrain texture.

### Material and biome color

The base color remains material-driven. Grass is special because Minecraft
textures are intentionally tintable. The original-profile renderer now maps
its sampled biome recipes into reference-shaped grass palette ids:

| Mclone recipe | Grass palette id |
|---|---:|
| ocean | `0` |
| shore | `16` |
| river/wetland | `7` |
| snowy alpine | `13` |
| cool wet conifer | `5` |
| warm dry steppe | `35` |
| temperate woodland | `4` |
| temperate meadow | `1` |

This replaces the prior generic dry/wet/cold gradient, which discarded the
already-computed biome recipe and made large grass regions less recognizable.
The vanilla LOD profile continues to use its direct sampled biome id.

The near-shell grass tint samples the active pack's `grass.png` colormap
through the exact mesh catalog for the selected biome id. It is still one
representative biome sample rather than vanilla's 5x5 neighborhood blend; the
far smooth path retains its cheaper reference-shaped palette.

### Lighting

The horizon is a stitched heightfield. For each vertex, the render shader
samples left/right/north/south heights from the same stitched geometry function
used for position. Coarse outer-footprint normals blend a wider derivative;
the normal halo supplies real samples beyond tile boundaries. These rules stop
lighting seams from revealing clipmap tiles.

Smooth levels retain one normalized directional term plus ambient bias,
clamped to `0.34..1.05`. The spacing-one shell instead assumes exposed full
sky/zero block light, uses the same sky-darken-dependent lightmap curve as exact
chunks, and applies the ordinary top/east-west/north-south face shades of
`1.0`, `0.6`, and `0.8`. The final target-color transfer and fog remain shared
with the other WGPU paths.

The near shell still does not carry stored per-column light, shadow maps,
weather attenuation, water specular, or reflections. Those effects require
compact world semantics and shared mono/per-eye/multiview-aware render
contracts rather than renderer-local guesses.

### Water and small inland features

There are two complementary water paths:

1. A sampled point whose visible material is water uses the ordinary water
   material/base-water shading at its display height.
2. Signed river distance and wetland-pool influence interpolate across the
   triangle, then an anti-aliased fragment mask paints water between coarse
   vertices.

The second path is what prevents a narrow river or procedural wetland pool from
vanishing merely because no coarse vertex lands at its center. The pool uses
the worldgen threshold `0.55`; river width expands to at least a pixel-aware
minimum. Both paths now call the same ground-relative depth function, using the
sampled ground/bed height rather than fragment-device depth. The analytic
overlay is disabled where the triangle's selected material is already water,
so river fields do not draw lines across open ocean.

This improves the continuous hydrology already present in the original Mclone
profile. It cannot reconstruct decoration-only `ConfiguredFeature::Lake`
instances that are absent from the procedural source. Supporting those ponds
requires a deterministic lake summary/record in worldgen or an exact-feature
handoff; it is not a texture-painting problem.

The analytic mask currently colors the terrain heightfield; it does not create
a separate flat surface at sea/pool level. A later geometry slice should compare
an independent water sheet against the current inexpensive color overlay.

In exact/procedural composition, the opaque procedural water surface remains
visible through exact-painted chunks while solid procedural faces are removed.
This is deliberate single-visible-owner arbitration: translucent exact water
otherwise produces a stable dark square because the compact exact snapshot
does not carry the water-column compositing depth needed by the opaque horizon.
The accepted close and elevated captures show the composed water matching the
horizon-only presentation without horizontal z-fighting. A future translucent
water contract must replace this rule explicitly rather than draw both water
surfaces.

## Decisions and Invariants

1. Worldgen owns material, biome, and water semantics; the renderer owns their
   bounded presentation.
2. Reuse the block atlas and its mip chain before introducing a generated
   megatexture.
3. Keep discrete material ids flat and use continuous semantic masks for narrow
   features that must survive between samples.
4. Do not change terrain identity or exact world output to improve LOD color.
5. Do not add camera-dependent sampling to worldgen. Screen-space filtering is
   presentation-only.
6. Preserve the fixed clipmap allocation unless a measured, documented change
   explicitly replaces that contract.
7. A surface change needs both shader validation and inspected pixels. Color
   screenshots alone are not performance evidence.
8. Mono, normal per-eye XR, and full-frame multiview are generated from the
   same appearance source. Appearance work does not introduce a per-eye-only
   alternate implementation.

## Performance Contract and Evidence

### Tactical 304 near-shell cost

The spacing-one shell generates topology from `vertex_index`; it adds no
resident vertex buffer and does not change the fixed 160 terrain / 70 staging
slot allocation. The active-pack face/tint uniform is 16 KiB, 12 KiB larger
than the former one-face UV table. Final native acceptance reports
`128,941,304` fixed resident bytes and zero pending work.

Sixteen finest tiles now reserve 30 vertex invocations per cell while the
other 144 tiles retain six. The worst-case terrain total is therefore
`5,505,024` versus `3,932,160`, a bounded 40% increase. Accepted native
movement captures report `4,709,496..5,294,208` submitted terrain-plus-tree
vertices after tile culling. This slice does not claim a GPU-time result;
portable timestamp evidence remains future work. Exact command lines and
inspected pixel paths are recorded in Tactical 304.

### Tactical 276 baseline

All measurements below are release builds on Linux x86_64, 20 logical CPUs,
commit `65d8f61267cae5492f5c303c875c034d1e4edde4`, with the already-dirty working
copy recorded by both receipts. Five measured macro iterations followed one
warmup. Raw artifacts live in `/tmp` and are not repository inputs.

The implementation changes fragment arithmetic only. It adds no terrain
sample, no packed field, no texture, no bind group, and no fixed allocation.
Matched CPU generation/packing confirms identical checksums, work counts, and
`540,800` packed bytes:

| 65,536-block preview | Before gen | After gen | Before pack | After pack |
|---|---:|---:|---:|---:|
| surface, 4,225 terrain evaluations | 3.403 ms | 4.033 ms | 0.231 ms | 0.264 ms |
| cover, 21,125 terrain evaluations | 15.946 ms | 16.002 ms | 0.214 ms | 0.208 ms |

The surface-only median is noisy in this five-sample run; because the changed
WGSL is not executed by the CPU compiler/packer and the checksum/work/bytes are
identical, it is not attributed as shader cost. The cover generation median is
`+0.4%` and packing is `-2.8%`.

The matched full World Explorer smoke exercises 30 rebases, 842 refills, 32
movement frames, complete settling, exact/procedural composition, and rendered
frames on the same Vulkan adapter:

| Host path | Metric | Before | After |
|---|---|---:|---:|
| window | movement mean / P95 | 1.857 / 2.625 ms | 2.243 / 4.365 ms |
| window | settling mean / P95 | 2.888 / 4.876 ms | 2.769 / 4.002 ms |
| offscreen | movement mean / P95 | 2.537 / 4.780 ms | 2.295 / 3.679 ms |
| offscreen | settling mean / P95 | 3.885 / 5.096 ms | 2.996 / 4.356 ms |

The short window movement sample regressed while both complete settling paths
and the offscreen movement path improved. This mixed result is treated as
run-to-run noise, not a claimed speedup. More importantly, fixed resident bytes
remain exactly `128,867,704`, final resident bytes remain `128,885,848`, and the
same 160 allocation/ready slots settle with zero pending work. Peak resident
bytes changed by less than 4 KiB.

This is sufficient to show no CPU work, upload, or residency expansion. It does
not isolate fragment-GPU duration; a later performance-instrumentation slice
should add portable timestamp queries where supported and a bounded fallback
measurement elsewhere.

## Pixel Evidence

A matched seed `-98765`, center `(-400, 1152)`, 2,048-block map capture was
inspected before and after. The accepted post-change Vulkan image shows:

- clearly separated meadow/woodland/steppe/conifer grass regions;
- continuous narrow inland river and wetland-pool water;
- snow, stone, coast, forest proxy, and texture detail still visible;
- no analytic river line across the open ocean; and
- no newly exposed clipmap seam or lighting grid.

A composed synthetic-stereo capture shows distinct valid pixels for both eyes
and the procedural background. The full-game headed Wayland WebGPU quality
probe also passes with 49 exact columns, ten drawn LOD levels, 160 drawn
tiles, 809 trees, target-ready terrain, and 48/48 vegetation jobs. Its
inspected canvas has 28,464 distinct interior colors, coherent biome/water
regions, and no blank, transparent, or near-black capture failure.

The standalone Explorer WebGPU semantic smoke also passed, but its four
Playwright canvas screenshots were solid white; they are rejected as pixel
evidence. Two dedicated Explorer composition attempts were then terminated
by the host. The successful full-game quality probe uses the same shared
shader and the established explicit one-frame smoke capture, so it is the
accepted browser presentation boundary for this slice.

Commands and exact filenames are recorded in Tactical 276. Temporary captures
remain under `/tmp` by policy.

## Known Gaps and Recommended Next Work

1. Load the active pack's biome colormaps and compare reference 5x5 grass
   blending against a cheaper LOD footprint approximation.
2. Add footprint-aware material coverage so very coarse cells can represent a
   stable mixture or dominant material rather than one flat point id.
3. Give procedural water a separate flat geometry/elevation contract if the
   heightfield overlay produces sloped rivers or ponds at grazing angles.
4. Decide whether decoration lakes receive deterministic multiscale summaries
   or deliberately remain exact-range-only.
5. Add measured time-of-day/sky-light and water-response experiments through
   shared per-view/multiview-aware lighting contracts.
6. Add renderer GPU timing before increasing atlas samples, biome blending, or
   water shading complexity.

## Code and Documentation Map

- `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/surface.rs` —
  original-profile visible surface classification.
- `native/crates/mclone-worldgen/src/terrain_preview.rs` — fixed semantic sample
  and packing contract.
- `native/crates/mclone-scene/src/terrain_view.rs` — block-state material ids to
  shared atlas UVs.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` — atlas mip chain,
  sampler, bindings, and shader contract tests.
- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl` —
  stitched geometry lighting, material/biome color, texture filtering, and
  analytic water presentation.
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) — parent
  residency/composition contract.
- [`../tactical/251-lod-surface-appearance-quality.md`](../tactical/251-lod-surface-appearance-quality.md)
  — earlier basic/inferred material-quality slice.
- [`../tactical/276-procedural-horizon-surface-appearance.md`](../tactical/276-procedural-horizon-surface-appearance.md)
  — this implementation and evidence record.
