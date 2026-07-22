# Figure Transparency Materials

Topic: `figure-transparency-materials`

Status: implemented and validated on 2026-07-22. The production slice extends
the completed binary-cutout contract with explicit material modes,
whole-figure opacity, and dithered versus smoothly blended Ghost proofs. Exact
order-independent transparency remains deferred.

## Scope

This topic owns figure-material alpha above the binary `"transparent"` palette
literal: semantic authoring, validation, prepared-figure compilation, Asset Lab
preview and catalogue behavior, native actor and review rendering, render-pass
partitioning, whole-figure opacity, and cross-view evidence.

The existing binary atlas and alpha-test foundation remains documented in
[`figure-alpha-cutout.md`](figure-alpha-cutout.md). This slice does not add
weighted blended OIT, depth peeling, per-pixel linked lists, refraction, or a
general transparent-world renderer.

## Accepted Semantic Contract

Figure materials gain additive optional fields while schema v1 and old assets
remain valid:

```ts
{
  color: "#88ccff",
  alphaMode: "opaque" | "mask" | "blend" | "additive",
  opacity: 0.0 .. 1.0,
  alphaCutoff: 0.0 .. 1.0,
  alphaCoverage: "threshold" | "dither",
}
```

- Defaults are `opaque`, opacity `1`, cutoff `0.1`, and threshold coverage.
- `alphaCutoff` and `alphaCoverage` are valid only for `mask`.
- Dither is an object/texture-anchored mask technique, not blending. It keeps
  ordinary depth writes and must remain stable under animation and stereo.
- `blend` uses ordinary source-over color with a character depth prepass. It
  gives a stable visible shell and deliberately does not promise visibility of
  every internal or rear surface.
- `additive` is color-only, depth-tested, and depth-write-free. It is intended
  for eyes, runes, and spectral glow rather than smoky opacity.
- Fully transparent atlas texels remain discarded in every mode.
- Palette entries may add `#RRGGBBAA`; the material mode determines whether
  surviving fractional alpha is thresholded, dithered, blended, or additive.
- A part or box face chooses a material. Raw polygon alpha flags are not a
  separate authoring surface.

Whole-figure opacity is per actor instance. Opaque and masked materials convert
that value to stable dithered coverage below one; blended and additive
materials multiply their fragment alpha or energy. This supports fades without
duplicating figure assets.

## Prepared And Render Contract

The renderer-neutral compiler resolves each box face to one prepared pass:
opaque, threshold mask, dither mask, blend, or additive. It preserves exact
face diagnostics while grouping indices into contiguous pass ranges, avoiding
one draw per face.

Rendering order is:

1. opaque and threshold/dither mask with depth writes;
2. blended depth prepass;
3. blended color with source-over composition and no depth writes; and
4. additive color with no depth writes.

The same pass set must exist for prepared review, ordinary actor drawing,
placed drawing, per-eye stereo, and full-frame multiview. A feature unavailable
in one of those paths is a blocker rather than a silent fallback. Blend
ordering is shared between multiview eyes; the character depth prepass is the
bounded initial answer to eye-dependent triangle order.

## Asset Lab Contract

Three.js remains the authoring oracle and consumes the serialized/reparsed
semantic asset. It must expose the same material modes and depth behavior,
including a real depth-only prepass for blended figure surfaces rather than a
preview-only depth-write shortcut.

The catalogue exposes alpha modes in searchable facts. The first visual proof
adds paired Ghost variants using one shared geometry vocabulary:

- a dithered whole-body apparition that demonstrates stable mask coverage; and
- a smooth translucent apparition with additive eyes that demonstrates the
  blend depth prepass and additive tail pass.

Both require clean static sheets, animated MP4 review, a catalogue capture, and
semantic-versus-native prepared comparison.

## Implementation Checkpoints

1. Add and test the semantic material vocabulary, RGBA palettes, and matching
   Three.js materials; commit the authoring boundary independently.
2. Add prepared pass ranges and native pipelines, with compiler and shader
   tests plus mono/multiview comparison evidence; commit the shared runtime
   boundary independently.
3. Add per-instance opacity and paired Ghost proofs, run every affected gate,
   inspect pixels in motion, and commit the visual acceptance closeout.

All three checkpoints are complete. The implementation landed as the following
reviewable series:

- `11a0f565` defines this contract;
- `a7a7d2e6` adds Asset Lab semantics, preview, and catalogue facts;
- `2b08857d` compiles exact faces into prepared alpha pass ranges;
- `de095e15` renders all prepared review paths;
- `355d6299` renders production actors and adds per-instance opacity;
- `1dda5b13` adds both shared-rig Ghost proofs; and
- `8d7d61a8` closes the browser and catalogue sentinels.

## Implemented Result

- Schema-v1 figures retain their old defaults while material validation accepts
  the four modes, fractional opacity, mask cutoff/coverage, and RGBA palette
  entries. Only texture characters that are actually used can imply a mask.
- The v2 compiler preserves exact face diagnostics, adds alpha cutoff to the
  prepared vertex, and groups indices into at most five semantic pass ranges.
- Review and production renderers own six physical pipelines: opaque,
  threshold mask, dither mask, blend depth, blend color, and additive. Ordinary,
  placed, clipped, per-eye, and multiview actor paths share the same ordering.
- `ActorInstance::with_opacity` clamps a per-instance fade. Solid passes turn a
  fractional instance value into depth-writing dither; blend and additive
  passes multiply alpha or energy.
- Asset Lab uses a matching Three.js depth prepass and exposes the effective
  used alpha modes as searchable catalogue facts. `ghost_dither` and
  `ghost_translucent` share one 15-box rig, texture, and two-clip motion set.

## Validation Evidence

The closeout used generated output under `/tmp`; no review captures are tracked
as source assets.

- Asset Lab typecheck and all 28 unit tests pass. Geometry, ground, and surface
  gates report zero failures across 176 canonical figures. Catalogue Playwright
  coverage passes all four desktop/mobile/classification/action tests and shows
  176 figures, 238 clips, and exact `Blend` plus `Additive` facts for the smooth
  Ghost.
- Clean static sheets and 24 FPS MP4s were inspected for both Ghosts. Each
  native animation comparison captured 44 frames with all 43 transitions
  changing, six exact synchronized sample times, four immutable topology
  uploads, and the production selector unchanged. Static and animated native
  panels preserve the semantic silhouettes in front, side, and three-quarter
  review.
- Full `mclone-assets` and `mclone-render` tests passed during the series; the
  final focused prepared-figure test run passes all six shader/layout tests.
  Full native workspace checking and the production remote-player visual smoke
  also passed.
- Headed-Wayland browser WebGPU reports compiler v2, six pipelines, one draw for
  the opaque player fixture, 52,757 figure pixels, 20 distinct figure colors,
  and no page errors. The captured canvas was inspected.
- Desktop synthetic stereo produced distinct, valid per-eye pixels. The flat
  Android debug APK and Android XR release APK both build successfully through
  the repository scripts, covering mobile Vulkan and the packaged multiview
  boundary.

## Deferred Direction

Revisit weighted blended OIT only when overlapping blended actors or layered
transparent surfaces visibly defeat the character depth-prepass contract. OIT
must then be designed once for browser WebGPU, native, Android, per-eye stereo,
and multiview; it is not a desktop-only quality toggle.
