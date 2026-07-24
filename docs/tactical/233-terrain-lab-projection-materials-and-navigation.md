# Terrain Lab Projection, Materials, And Navigation

Status: completed with local and hosted desktop/mobile validation 2026-07-24.

Topic: `gpu-procedural-terrain`

## Objective

Make the canonical, CPU LOD, and GPU LOD panes visually comparable rather
than merely coordinate-locked:

- one physical block-to-screen camera contract must govern every pane;
- procedural terrain must carry production surface-material identity and
  sample the first-party block atlas instead of inventing climate colors;
- exact and procedural texture minification must use an explicit mip/filter
  policy suitable for the Lab's oblique overview; and
- map and 3D views must support direct drag and keyboard panning.

## Originating Direction

The first canonical workspace proved exact chunks and broad LOD together, but
review exposed three misleading presentation gaps:

1. canonical terrain uses a perspective block-space camera while LOD uses a
   normalized, height-exaggerated shader projection;
2. the LOD payload has height and climate fields but no surface block, causing
   sand and other production materials to render as generic grass-like color;
3. the shared chunk atlas has vanilla-style nearest mip selection, which
   aliases strongly in the Lab's oblique miniature view and appears
   effectively unmipmapped.

The workspace also lacks discoverable 3D panning. Right-drag should pan in
both views, and arrow keys should move the shared center without requiring the
control rail.

## Product Contract

### Shared Projection

All panes use the same camera model for a given:

- center X/Z;
- horizontal footprint in blocks;
- logical pane pixel aspect;
- map/3D mode; and
- yaw and pitch.

The LOD renderer must stop applying an implicit viewport-dependent height
exaggeration. If exaggeration is added later, it is one explicit shared
parameter and affects exact and procedural terrain equally.

Map mode is a top-down physical projection. Three-dimensional mode is a
perspective projection aimed at the common sea-level-centered target. The
procedural shader may use center-relative coordinates for precision while the
canonical renderer uses absolute mesh coordinates; their projected X/Y
contract must still agree.

### Surface Materials And Textures

The production first-party surface rule remains the semantic owner. Terrain
preview samples gain a stable macro material identity sufficient to
distinguish:

- water;
- grass;
- sand;
- gravel;
- coarse dirt;
- stone; and
- snow-covered ground.

CPU reference generation derives this identity from production landform and
surface rules. GPU evaluation implements the same preview material contract
from its resident fields and reports material agreement separately from height
agreement. Material disagreement must not be hidden by color similarity.

The procedural renderer loads the same first-party authored and generated
fallback packs as canonical terrain. Near LOD levels use world-anchored block
texture UVs. Coarser levels select appropriate mip detail and may blend toward
material-average color when a repeated block texture would alias below a
pixel. A coarse sample never labels sand as grass merely because both are
land.

This slice textures the existing height field. It does not claim to reproduce
vertical block faces, caves, trees, structures, or final feature geometry in
the LOD panes.

### Mip And Filter Policy

Clay remains owned by final wetland/watercourse surface generation and is not
invented in the uncarved macro-base payload.

The shared chunk renderer retains its vanilla-compatible default sampler.
Terrain Lab opts into an overview sampler that:

- uploads the existing bounded block-atlas mip chain;
- uses linear minification and linear mip interpolation;
- keeps nearest magnification so close pixels remain block-like; and
- remains within the baseline WebGPU feature set; anisotropy is deferred
  because this slice preserves nearest magnification and requests no optional
  sampler feature.

The procedural atlas follows the same Lab policy. Diagnostics expose the
allocated mip count and filter mode so the result is verifiable rather than
inferred from appearance.

### Navigation

- Left drag in 3D orbits.
- Right drag pans in map and 3D.
- Shift+left and middle drag remain pan aliases.
- Left drag in map continues to pan.
- Arrow keys pan in both modes while a Terrain Lab stage is focused or
  hovered.
- Keyboard panning ignores editable controls and uses a bounded fraction of
  the current footprint per keypress.
- Panning updates the shared URL center and therefore every visible pane.

The stage suppresses the context menu for right-drag and exposes a focus ring
and accessible keyboard instructions.

## Shared Ownership

- `mclone-worldgen` owns surface material classification.
- `mclone-terrain-view` owns preview material packing/comparison and the
  shared projection parameters consumed by Rust and WGSL.
- `mclone-render` owns configurable atlas upload/filter policy.
- `mclone-terrain-lab` owns browser surface and atlas assembly.
- `tools/terrain-lab` owns browser pointer/keyboard translation and labels.

TypeScript must not recreate material rules or projection math.

## Implementation Order

1. Lock this contract and topic status.
2. Add shared physical projection data and use it in both renderers.
3. Add right-drag and arrow-key navigation with state and browser tests.
4. Export production preview material classification and pack it into CPU
   reference samples.
5. Implement the matching GPU material evaluator and comparison facts.
6. Upload a first-party procedural atlas and sample it with world-space UVs.
7. Add the Lab overview mip/filter policy to exact and procedural atlases.
8. Inspect close, oblique, beach, mountain, map, desktop, and phone captures.
9. Perform a targeted Terrain Lab deployment and record hosted receipts.

Commit every coherent slice with `Topic: gpu-procedural-terrain`.

## Implementation Outcome

The slice landed as commits `e7c723a8`, `5634e15a`, `8cfc7642`,
`17d72ecd`, `ce1d4e52`, `9664ec87`, and `a0621c8e`.

- `mclone-terrain-view` now owns one perspective/top-down projection
  descriptor. Exact chunks consume it through `ChunkCamera`; LOD WGSL consumes
  the same eye, target, up, FOV, near/far, and logical-pane aspect values.
  The old viewport-dependent LOD height exaggeration is gone.
- `mclone-worldgen` exports a spacing-stable macro top-material classifier for
  water, sand, grass, snow, exposed stone, gravel, and coarse dirt. The CPU
  preview packs the raw block ID and the GPU evaluator ports the same
  classifier under evaluator revision `gpu-preview-a3`.
- CPU/GPU comparison reports material agreement independently of height and
  ocean agreement.
- The procedural renderer loads the authored/generated first-party packs,
  maps raw surface block IDs to atlas sprites, repeats detail in world block
  coordinates, and modulates the semantic material color so incomplete
  fallback art cannot turn every surface purple.
- Procedural and exact overview paths allocate five atlas mip levels. The Lab
  uses nearest magnification plus linear minification and linear mip
  interpolation. The normal chunk renderer retains nearest/nearest as its
  default.
- Right mouse, Shift+left, and middle mouse pan in 3D. Left mouse remains
  orbit. Arrow keys pan a focused pane in map or 3D, and the stage exposes a
  visible focus ring.

The material contract intentionally describes the uncarved broad LOD surface.
Final rivers, wetlands, clay beds, and planned streams remain visible only in
the exact pane until their structured overlay is ported.

## Validation

- canonical and LOD projections place fixed world landmarks at matching
  normalized screen positions;
- changing pane layout does not change block scale inside a logical pane;
- CPU preview material matches direct production classification at pinned
  beach, ocean, grass, eroded, alpine, and exposed-stone sites;
- GPU material agreement is measured over complete target footprints;
- sand, gravel, grass, stone, snow, and water visibly use distinct
  first-party textures;
- mip count/filter policy is unit-tested and reported by the browser;
- oblique exact terrain no longer exhibits the previous high-frequency
  shimmer in inspected captures;
- right-drag and arrow keys pan both map and 3D views without orbiting,
  scrolling the page, or editing focused inputs; and
- local and hosted headed-Wayland desktop/phone WebGPU smokes remain clean.

## Local Evidence

`cargo test -p mclone-worldgen macro_surface_materials_preserve_distinct_lod_regions --lib`
pins seven representative macro materials. `mclone-terrain-view` shader/unit
tests pass, the Terrain Lab Wasm target checks, and
`terrain_overview_sampling_does_not_change_vanilla_defaults` proves the Lab
filter opt-in does not change normal chunk sampling.

Headed-Wayland desktop and Pixel smokes both completed the exact/CPU/GPU
workspace, orbit, right-button desktop pan, focused arrow pan, map zoom,
65.5 km field check, and cold independent scheduler race. Both reported:

- zero CPU/GPU base-height mean/P95 error at the 512-block review;
- 100% ocean agreement;
- 100% macro-material agreement;
- about `2e-8` mean continentalness error; and
- `5 mip · trilinear minification` in browser diagnostics.

Inspected desktop and phone captures show the exact patch at its truthful
bounded radius and matching physical scale, with independently published LOD
panes using material-preserving atlas detail. Generated purple/pink fallback
art remains visible in the exact patch where first-party curation is absent.

## Hosted Receipt

A clean detached build at product commit `7a538223` was uploaded only under
`/terrain/`; the Cloudflare Worker and the rest of the aggregate deployment
were unchanged. Hashed assets were uploaded before HTML and downloaded back
from R2 for byte-for-byte SHA-256 verification. Production now serves:

- JavaScript `index-DC0tTMgU.js`;
- canonical Worker `canonical-worker-C3EyhCMt.js`;
- stylesheet `index-BUU9cbcd.css`; and
- Wasm `mclone_terrain_lab_bg-CEXO1oC3.wasm`.

Hosted headed-Wayland desktop and Pixel smokes passed against
`https://mclone.kzahel.com/terrain/`. They exercised exact completion, shared
orbit, desktop right-button pan, focused arrow pan, map/65.5 km navigation,
and the cold independent scheduler race. Both retained zero review-site base
mean/P95 error, 100% ocean agreement, 100% material agreement, and about
`2e-8` continentalness error.

The hosted desktop cold race published GPU target plus readback in 847.6 ms
and CPU target in 14,134.1 ms. Pixel measured 1,123.8 ms and 12,758.1 ms.
These are end-to-end fixed-workload boundaries under host load, not pure GPU
execution timings.

## Non-Goals

- authoritative lighting;
- final-feature geometry in LOD;
- provenance-perfect feature toggles;
- truthful area-averaged far material coverage beyond the first filtered
  material representation;
- replacing in-game Far LOD; or
- changing the game's default vanilla-compatible block sampler.
