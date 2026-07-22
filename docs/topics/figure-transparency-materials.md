# Figure Transparency Materials

Topic: `figure-transparency-materials`

Status: accepted for implementation on 2026-07-22. The first production slice
extends the completed binary-cutout contract with explicit material modes,
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

## Deferred Direction

Revisit weighted blended OIT only when overlapping blended actors or layered
transparent surfaces visibly defeat the character depth-prepass contract. OIT
must then be designed once for browser WebGPU, native, Android, per-eye stereo,
and multiview; it is not a desktop-only quality toggle.
