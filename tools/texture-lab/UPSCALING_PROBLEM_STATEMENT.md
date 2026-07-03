# Texture Upscaling Problem Statement

## Context

The texture lab can now generate believable original textures that match a
local vanilla reference at the same effective resolution. The recent stone pass
is a useful example: an LLM-guided 16x16 structure mask, informed by extracted
reference features, produced a stone tile that is visually close to vanilla
when reviewed at vanilla scale.

That success exposed a resolution problem. The accepted stone source is
declared as a 32x32 texture, but its dominant layer is a 16x16 mask upscaled
with nearest sampling. The remaining 32x32 procedural layers are intentionally
very low opacity, so the output behaves like a good 16x16 texture wearing a
32x32 size. It matches the analyzer well because the analyzer compares the
candidate to the vanilla reference after downsampling to the shared 16x16 grid.

This is not necessarily wrong. Some textures may be better authored as true
16x16 assets. But if a texture is declared 32x32, we want the native pixels to
carry useful material detail instead of just duplicating each 16x16 pixel into a
flat 2x2 block.

## Problem

How can the lab produce 32x32 source art that:

- preserves the 16x16 macro character of a good vanilla-scale texture
- adds meaningful native 32x32 material detail
- remains deterministic, tile-safe, palette-disciplined, and reviewable as
  source
- avoids metric-gaming patterns such as checkerboards, salt-and-pepper noise,
  diagonal shimmer, or generic dithering

The core tension is that the current feature comparison intentionally removes
native 32x32 detail by downsampling to 16x16. That makes it a good test for
"does this still read like vanilla at vanilla scale?" but a poor test for "did
we actually use the extra resolution well?"

## Desired Invariants

A 32x32 upscale/detail pass should satisfy two classes of invariants.

### Macro Fidelity

Downsample the 32x32 candidate back to 16x16. The downsampled result should stay
close to the accepted 16x16 macro plan and to the vanilla reference feature
profile:

- tone distribution and dominant color share
- row/column banding and anisotropy
- grain direction and strength
- dark/light blob size and count
- repetition peak
- seam continuity
- mip/downsample readability

This constraint prevents the upscale from changing the material identity.

### Native Detail

Compare the 32x32 candidate to a dumb nearest-neighbor 2x upscale of its own
16x16 downsample. The result should be measurably different, but in a
material-shaped way:

- enough non-flat 2x2 cells to avoid fake 32x32 resolution
- detail aligned with the material's language
- no sparkle pixels or high-frequency glitter
- no checkerboard, uniform jitter, or random noise as a substitute for detail
- no new seam defects or stamped macro motifs

This constraint prevents the upscale from being only four identical pixels per
macro cell.

## Candidate Two-Pass Workflow

1. **Macro pass**

   Author a compact 16x16 material plan. Use the current analyzer loop to match
   extracted vanilla-scale features. This is where the LLM or human can already
   do useful work: choose tone placement, broad strokes, blob size, seams, and
   material direction.

2. **Upscale/detail pass**

   Expand the 16x16 plan to a 32x32 texture through a deterministic constrained
   generator. The generator should add sub-cell detail while preserving the
   16x16 downsample. The LLM should guide rules and review artifacts, not hand
   paint all 1024 cells.

3. **Rank variants**

   Generate multiple deterministic variants, then score them with both macro
   fidelity and native-detail constraints. Keep the best candidates for visual
   review.

4. **Human/LLM review**

   Review the sheet for material plausibility. The metrics should reject obvious
   cheats, but they will not know whether a texture reads as stone, grass, dirt,
   bark, or noise.

## Constrained Upscaler Sketch

The upscaler can treat the 16x16 macro map as a material plan.

For each macro pixel, expand to a 2x2 cell. The default cell is flat:

```text
b b
b b
```

Then apply local, seeded modifications based on neighboring macro tones:

- swap one subpixel to an adjacent tone while preserving average luminance
- chip the edge where light meets mid
- notch the edge where pit meets mid or base
- break long horizontal runs into tiny shelves
- suppress isolated one-pixel noise
- keep changes periodic across tile boundaries

For stone, valid micro-detail might be:

```text
h b
h h
```

or:

```text
m h
m b
```

Those cells add native 32x32 structure while keeping the macro tone close.

## Material-Specific Presets

The native-detail rule set must be material-specific. A generic "add variance"
rule will be easy to game and will often look worse.

Possible presets:

- **stone:** short horizontal chips, shallow shelves, dark boundary notches,
  low sparkle, weak color variance
- **dirt:** clods, crumb edges, small warm/cool flecks, less directional grain
- **grass:** blade strokes, clump-aligned cuts, tint-safe value variation
- **leaves:** cutout-aware clusters, binary alpha discipline, foliage tint
  compatibility
- **bark:** vertical fibers, ring-aware top faces, axis-aware detail
- **ores:** preserve host-rock texture, add mineral detail only inside ore masks

The LLM can help tune and critique these presets, but the generator should keep
the constraints deterministic and inspectable.

## Suggested Metrics

Add native-resolution metrics in addition to the existing downsampled vanilla
feature comparison:

- **upscale flatness:** distance between the 32x32 candidate and its
  downsample-to-16 then nearest-upscale image
- **2x2 cell variance:** share of cells with meaningful but bounded subpixel
  variation
- **cell average error:** how much each 2x2 cell's average drifts from the macro
  source tone
- **native sparkle:** isolated high-contrast pixels at 32x32
- **native seam continuity:** edge mismatch at 32x32
- **micro repetition peak:** repeated sub-cell motifs that stamp when tiled
- **material directionality:** for stone, mostly horizontal chips; for bark,
  mostly vertical fibers; for dirt, less directional clods

A rough scoring shape:

```text
score =
  macro_downsample_error
+ vanilla_feature_error
+ cell_average_error
+ upscale_flatness_penalty
+ sparkle_penalty
+ seam_penalty
+ repetition_penalty
+ material_style_penalty
```

The score should not maximize detail. It should add enough structured detail
while preserving the accepted macro read.

## Anti-Gaming Risks

Naive metrics can be gamed.

- Flatness penalties can be satisfied with checkerboards.
- Variance penalties can be satisfied with salt-and-pepper noise.
- Directionality can be satisfied with mechanical stripes.
- Repetition checks can be satisfied with random noise that fails visually.

Mitigations:

- require bounded 2x2 average drift
- cap native sparkle and local high-contrast outliers
- measure material-specific direction and connectedness
- score mip/downsample stability
- include visual review sheets as a required decision point
- prefer small curated pattern libraries over unconstrained random jitter

## Other Approaches To Evaluate

- **True 16x16 textures:** allow some vanilla-close materials to stay 16x16
  explicitly. This may be the right answer for stone if mixed texture
  resolutions are acceptable.
- **Manual 32x32 authoring:** ask the LLM or human to author a full 32x32 ASCII
  mask. This is reviewable but laborious, and LLMs may produce inconsistent
  large masks.
- **Image-model upscaling:** use a diffusion or image super-resolution model to
  generate perceptual detail. This may look better but weakens determinism,
  palette discipline, tiling guarantees, legal reviewability, and source
  compactness. This is the approach currently being pursued, with those
  objections addressed by projecting proposals back into frozen, quantized
  mask source — see [`DIFFUSION_UPSCALE_PLAN.md`](DIFFUSION_UPSCALE_PLAN.md).
- **Pattern-library upscaling:** use curated 2x2 or 4x4 replacement patterns per
  material. This is deterministic and reviewable, but risks predictable motifs.
- **Hybrid variant tournament:** generate several constrained upscales, rank by
  metrics, then have a human/LLM choose the best visual candidate and tune the
  preset.

## Review Questions

- Should the pack support explicit 16x16 textures alongside 32x32 textures, or
  should we prefer one pack-wide resolution for visual consistency?
- Which native-detail metric best detects "fake 32x32" without rewarding noise?
- Should the 16x16 macro plan remain checked-in as an authoring layer, with the
  32x32 detail generated deterministically from it?
- How much 2x2 average drift is acceptable before the macro texture changes?
- Which material-specific presets should be implemented first?
- Is image-model upscaling useful as a private reference or variant proposal,
  even if the checked-in source remains deterministic DSL art?

## Initial Experiment

Use stone as the first trial:

1. Keep the current successful 16x16 stone macro mask as the macro plan.
2. Implement a seeded stone upscaler that edits only within 2x2 macro cells and
   along macro tone boundaries.
3. Add native flatness, 2x2 variance, cell average error, native sparkle, and
   native seam metrics.
4. Generate a small tournament of variants.
5. Compare each variant against:
   - the current nearest-upscaled 32x32 stone
   - the 16x16 macro plan after downsample
   - the vanilla reference feature profile
6. Review the best sheet visually before accepting any source change.

The experiment succeeds if the selected 32x32 stone has visible native chips or
shelves, still reads like the current macro stone at 16x16, and does not open
new seam, sparkle, repetition, or material-style defects.
