# Tactical 276: LOD Water-Surface Presentation Unification

Status: **complete 2026-08-11**

Topic:

- `procedural-horizon-clipmap`

## Instruction Synthesis

Correct the composed World Explorer presentation defects visible after the
below-sea material fix: river ribbons remain visible across ocean surfaces,
rivers use a different blue from adjacent ponds, and river/pond junctions read
as independent layers. Keep this slice presentation-only, record the decision
before implementation, validate rendered output, and commit the completed
correction.

## Diagnosis

The shared procedural terrain shader currently applies two different water
contracts:

- ordinary ocean, pond, and watercourse samples use the material water color,
  derived from solid-surface depth below sea level; and
- an analytic river-distance overlay paints a separate blue after material
  texturing, regardless of whether the fragment still represents a physical
  continental channel.

The overlay reads planned signed distance and width, so it survives across the
coast where the same hydrology continues only as a submerged ocean-floor
outlet. It also derives its darkening from fragment depth instead of terrain
bed depth. The visible defects therefore share one presentation cause; they do
not require a sea-level clamp, pond rewrite, cave work, or changed worldgen.

## Objective

Make every procedural water surface obey one shared presentation contract
while retaining the analytic river information needed to keep narrow physical
channels legible at coarse clipmap spacing.

```text
physical water/material sample
  + analytic channel edge coverage
  + continental physical-channel ownership
  -> one bed-depth water color
```

## Scope

- Extract the existing procedural water color into a reusable shader helper.
- Carry interpolated solid-surface height to the fragment stage so coverage
  correction uses terrain depth rather than clip-space depth.
- Gate analytic river coverage by physical channel influence and continental
  ownership.
- Blend the accepted coverage toward the same water color used by ocean,
  pond, and directly sampled river materials.
- Preserve forest-cover modulation and the shared mono/stereo/multiview shader
  path.
- Add source-lock tests for the ownership and shared-color invariants.

## Non-goals

- Do not change terrain heights, sea level, water fill, channel routing,
  submerged outlet morphology, pond placement, or river/pond topology.
- Do not hide submerged outlets by flattening the ocean floor. They remain
  valid bathymetry and simply cease to own a distinct surface tint.
- Do not attempt local junction widening, wetland blending, sediment banks,
  flow direction, water animation, transparency, or exact-renderer changes.
- Do not introduce aquifers, cavifiers, caves, or disabled Caves & Cliffs Part
  1 systems.

## Presentation Contract

1. Water is one material family. Ocean, pond, and river fragments use the same
   bed-depth color function.
2. Analytic river distance may improve sub-cell edge coverage only where
   interpolated physical channel influence is nonzero.
3. A fragment on the ocean side of the continental boundary receives no river
   surface overlay. The submerged outlet may still lower the ocean floor.
4. Water shading must not depend on fragment/clip-space depth or camera angle.
5. Forest-cover modulation remains independent of whether river coverage is
   present.

## Acceptance

- The shared render WGSL parses and validates for vertex and fragment entry
  points.
- Tests lock physical-channel and continental gating, shared water-color use,
  and removal of the camera-depth river color.
- Native World Explorer composed output is captured at the reported
  river/ocean/pond review site and visually inspected.
- Rivers no longer continue as dark stripes across ocean surfaces.
- River and pond water at a junction no longer separate solely because the
  former uses a private RGB palette.
- Native and browser builds retain the same shared shader contract.

## Deferred Follow-up

After this correction, any remaining awkward river/pond junction silhouette is
an honest geometry/topology issue rather than a color-layer artifact. Review it
separately before choosing among local junction dilation, bank softening,
wetland semantics, or a hydrology rewrite.

## Execution Record

The shared WGSL renderer now exposes one `water_surface_color(surface_y)`
function for directly sampled material water and analytic channel coverage.
The vertex stage carries solid-surface Y to the fragment stage, and the
fragment stage applies the same material-atlas treatment when coarse channel
coverage must replace a non-water base fragment. The former private river RGB
palette and clip-space-depth darkening are gone.

Analytic coverage is multiplied by interpolated physical channel influence and
hard-gated to positive continentalness. This retains the signed-distance edge
aid on sampled continental channels while preventing the submerged outlet from
claiming ocean-surface color. Forest-cover modulation remains in its original
stage and is not conditional on nonzero river alpha.

Validation completed on 2026-08-11:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view`:
  85 passed, one native-adapter test ignored by declaration;
- Naga vertex/fragment parsing and validation passed inside that suite;
- `cargo check --manifest-path native/Cargo.toml -p
  mclone-world-explorer --lib --target wasm32-unknown-unknown` passed; and
- a 1,024-by-576 native Metal composed capture at seed `8675309`, center
  `(-1536, 2032)`, 512 blocks across, and exact radius 2 reached all 160
  procedural slots and all 25 exact chunks. Visual comparison against the
  pre-correction capture confirms that the ocean ribbons are absent while
  continental rivers remain visible and use the same water family at pond
  junctions.

No CPU/GPU terrain evaluation, sea-level, exact-world, persistence, or
hydrology output changed. Remaining junction-shape concerns stay deferred as
geometry/topology review rather than being hidden inside this presentation
fix.
