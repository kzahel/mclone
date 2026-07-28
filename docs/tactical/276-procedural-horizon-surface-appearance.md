# Tactical 276: Procedural Horizon Surface Appearance

Status: complete 2026-07-28, including native Vulkan, synthetic per-eye
stereo, and headed browser/WebGPU pixel acceptance plus matched performance.

Topic: `procedural-horizon-surface-appearance`

## Originating Request

The most visible remaining LOD problem is that the horizon does not reflect
grass-biome colors well and misses some lake/pond water. Determine whether the
already-visible dirt, stone, and snow prove an existing texture/color system;
document how LOD texture, color, and lighting actually work; improve the shared
surface so it reflects the ground more faithfully; and measure performance.

## Objective

Improve the existing fixed-budget procedural-horizon presentation without
creating a second terrain identity or an unbounded painted-texture cache:

- retain the ordinary block-atlas texture path;
- consume the biome and wetland semantics already present in each sample;
- make water depth independent of fragment-device coordinates;
- keep exact worldgen and the packed sample layout unchanged;
- validate shader contracts and inspect real rendered pixels; and
- compare matched release CPU, streaming/frame, and resident-memory receipts.

The living contract and current-system explanation belong in
[`../topics/procedural-horizon-surface-appearance.md`](../topics/procedural-horizon-surface-appearance.md).

## Initial Finding

The premise was correct: an implementation already existed.

1. Worldgen classifies a raw visible block material at each point.
2. The preview sample carries that id plus climate, biome, river, wetland, and
   pool fields.
3. Scene composition maps the raw id to the normal block atlas.
4. The horizon renderer builds mips, samples the sprite in world space, and
   reduces detail with projected block size.
5. A stitched heightfield normal feeds fixed directional-plus-ambient light.

Dirt, stone, snow, sand, gravel, and coarse dirt therefore already used real
texture sprites and material base colors. Grass looked too uniform because the
Mclone branch ignored the explicit biome recipe and rebuilt a generic
temperature/moisture gradient. Rivers had a fragment overlay, but wetland-pool
influence was diagnostic-only. The river overlay also derived water depth from
`input.position.z`, which is fragment depth rather than terrain height.

Decoration-only `ConfiguredFeature::Lake` records are not present in the direct
procedural source. No shader can reconstruct those features from absent input.
This tactical improves procedural rivers and wetland pools; decoration-lake
summaries remain explicitly deferred.

## Scope and Ownership

Shared owners only:

- surface/biome/hydrology semantics: `mclone-worldgen`;
- atlas and render bindings: `mclone-scene` and `mclone-terrain-view`;
- appearance implementation: the shared terrain-view WGSL;
- platform apps: no visual policy changes.

No sample field, GPU buffer, texture allocation, cache, worldgen output,
persistence identity, exact mesh, or product preference changes are in scope.

## Slice 0: Baseline

Before changing pixels:

```sh
pnpm native:worldgen:macro-perf --iterations 5 \
  --warmup-iterations 1 \
  --only preview-surface-65k --only preview-cover-65k \
  --output /tmp/mclone-t276-before-macro.json

pnpm native:world-explorer:smoke \
  /tmp/mclone-t276-before-streaming

native/target/release/mclone-world-explorer \
  --capture /tmp/mclone-t276-before-wetland-map.png \
  --composition horizon --seed -98765 \
  --center-x -400 --center-z 1152 \
  --blocks-across 2048 --view map
```

The baseline used commit
`65d8f61267cae5492f5c303c875c034d1e4edde4`, a release build, Linux x86_64,
20 logical CPUs, and the AMD Radeon 890M Vulkan adapter. Receipts honestly mark
the pre-existing working copy dirty.

## Slice 1: Ground Color

The render shader now maps all eight original-profile biome recipes to the
existing reference-shaped grass palette. The vanilla profile keeps its sampled
vanilla biome id. This removes only the generic Mclone dry/wet/cold fallback;
material textures, lighting, target-color transfer, and non-grass material
colors remain unchanged.

A focused source-contract test pins every recipe mapping and prevents the old
generic palette from silently returning.

## Slice 2: Inland Water

The production Mclone fragment path now:

- anti-aliases the interpolated wetland-pool influence at the worldgen `0.55`
  threshold;
- takes the maximum of river and pool coverage;
- uses one shared ground-relative water-color function for sampled and analytic
  water;
- carries sampled ground/bed height instead of unrelated continentalness in the
  render varying; and
- suppresses the analytic overlay when the triangle's selected material is
  already water.

The first post-change capture exposed the last rule as necessary: correcting
water depth made an old river-distance line across the ocean brighter. The
water-material mask removed it while preserving interpolated inland water. This
was caught at the first drawable milestone, before the broader benchmark run.

This remains a color overlay on the stitched terrain surface. Independent flat
water geometry and decoration-lake summaries are later contracts.

## Slice 3: Objective Evidence

### Shader and unit validation

Completed:

```sh
cargo test --manifest-path native/Cargo.toml \
  -p mclone-terrain-view \
  horizon_shader_uses_biome_ground_color_and_interpolated_pool_water --lib

cargo test --manifest-path native/Cargo.toml \
  -p mclone-terrain-view compute_and_render_shaders_validate --lib
```

Both tests pass. The second test parses and validates the composed WGSL.

### Matched macro performance

```sh
pnpm native:worldgen:macro-perf --iterations 5 \
  --warmup-iterations 1 \
  --only preview-surface-65k --only preview-cover-65k \
  --output /tmp/mclone-t276-after-macro.json
```

| Workload | Before gen | After gen | Before pack | After pack |
|---|---:|---:|---:|---:|
| surface 65k | 3.403 ms | 4.033 ms | 0.231 ms | 0.264 ms |
| cover 65k | 15.946 ms | 16.002 ms | 0.214 ms | 0.208 ms |

The before/after checksums, terrain-evaluation counts, and `540,800` output
bytes are identical. The changed fragment shader is not part of this CPU path;
the mixed medians are recorded as run noise rather than attributed speedup or
regression.

### Matched rendered streaming performance

```sh
pnpm native:world-explorer:smoke \
  /tmp/mclone-t276-after-streaming
```

Both runs complete 30 rebases and 842 refills with 160 ready slots and zero
pending work:

| Host path | Metric | Before | After |
|---|---|---:|---:|
| window | movement mean / P95 | 1.857 / 2.625 ms | 2.243 / 4.365 ms |
| window | settling mean / P95 | 2.888 / 4.876 ms | 2.769 / 4.002 ms |
| offscreen | movement mean / P95 | 2.537 / 4.780 ms | 2.295 / 3.679 ms |
| offscreen | settling mean / P95 | 3.885 / 5.096 ms | 2.996 / 4.356 ms |

The 32-frame window movement sample regresses, while both full settling paths
and offscreen movement improve. Treat the mixed timing as ordinary run
variation. Fixed resident bytes remain exactly `128,867,704`; final resident
bytes remain `128,885,848`; peak bytes differ by less than 4 KiB. The slice adds
no CPU work, packed bytes, upload, or resident allocation.

A future instrumentation tactical should isolate fragment GPU time with
portable timestamp queries. The current smoke measures whole rendered frames,
which is the established end-to-end performance gate but not a shader
microbenchmark.

### Pixel review

Accepted native Vulkan comparison:

```sh
native/target/release/mclone-world-explorer \
  --capture /tmp/mclone-t276-after-wetland-map-v2.png \
  --composition horizon --seed -98765 \
  --center-x -400 --center-z 1152 \
  --blocks-across 2048 --view map
```

The inspected post-change image has stronger region-appropriate grass color,
continuous narrow inland water, retained snow/stone/coast/texture/lighting
readability, and no river overlay across open ocean. The scripted smoke also
captures matched 3D/orbit views for final cross-checking.

The complete `mclone-terrain-view` library suite passes: 85 tests pass and
one native-adapter-only test remains intentionally ignored. The workspace
compile gate (`cargo check --manifest-path native/Cargo.toml --workspace`) also
passes; warnings belong to pre-existing dirty work outside this slice.

The standalone Explorer WebGPU semantic smoke reached ten levels, 160 tiles,
target-ready composition, and clean Worker shutdown. Its four Playwright
canvas screenshots were nevertheless solid white, so they were rejected as
pixel evidence. Two dedicated Explorer composition attempts were then
terminated by the host.

The established full-game quality capture supplied the valid browser boundary:

```sh
pnpm native:web:app-smoke -- \
  --generation-profile mclone-overworld-v1 \
  --terrain-presentation composed --terrain-composition-probe \
  --screenshot-eye 8,105,8 --screenshot-target 8,72,-300
```

The headed Wayland capture `/tmp/mclone-native-web-app-canvas.png` was
inspected. It shows coherent biome-colored terrain, rivers/open water, atlas
detail, directional lighting, and no new ring seam. The receipt reports 49
exact columns, ten LOD levels, 160 tiles, 809 trees, target-ready true, 48/48
vegetation jobs, and 28,464 distinct interior colors.

The composed synthetic-stereo capture
`/tmp/mclone-t276-xr-composed.png` was also inspected. Both 640x640 eyes show
valid procedural background with 227,366 differing eye pixels; no per-eye
shader or projection discrepancy is visible.

## Acceptance

- [x] The current LOD texture/color/lighting system is documented.
- [x] Original-profile grass consumes the sampled biome recipe.
- [x] Interpolated wetland pools can remain visible between coarse vertices.
- [x] Far water color uses terrain height rather than fragment depth.
- [x] Open-ocean material is not overpainted by inland river fields.
- [x] No sample-layout, work-count, texture, or fixed-residency increase lands.
- [x] Focused shader contracts and WGSL validation pass.
- [x] Matched release CPU, frame, and memory evidence is recorded.
- [x] Native pixels are captured and visually inspected.
- [x] Headed browser/WebGPU pixels and affected build boundaries pass.

## Explicit Follow-ups

- active-pack grass/foliage colormaps and cheaper neighborhood blending;
- footprint-aware material mixtures at coarse LOD;
- independent flat water geometry and shoreline behavior;
- deterministic decoration-lake summaries or an exact-only decision;
- time-of-day/stored-light/shadow/water response; and
- portable fragment-GPU timing.
