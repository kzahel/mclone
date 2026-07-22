# Figure Card Primitives

Topic: `figure-card-primitives`

Status: implemented and validated on 2026-07-22. The first proof converts the
Maw Orchid's four textured petals from paired thin-box faces to exact two-sided
cards.

## Scope

This topic owns finite, fixed planar surfaces in the figure semantic format,
Asset Lab preview, native prepared-figure compilation, alpha-pass selection,
and authoring analysis. Cards are intended for petals, leaves, fins, wings,
cloth, and similar intentionally flat details.

Cards are not sprites or billboards. They inherit the ordinary part hierarchy,
remain fixed in figure-local space, and never turn toward the camera. This
topic does not add arbitrary polygon meshes, curved geometry, alpha-contour
extrusion, collision surfaces, or a general material flag that disables
back-face culling.

## Semantic Contract

Canonical figure authoring expands from boxes to boxes and cards. The DSL
surface is:

```ts
part("petal", plane({
  parent: "flower",
  size: [0.72, 1.10],
  material: "petal",
  texture: "petal_mask",
  sidedness: "double",
}));
```

The serialized schema-v1 primitive stores `kind: "plane"`, positive `width`
and `height`, and explicit `sidedness: "front" | "double"`. A card lies in
its part's local XY plane with its front normal along local positive Z.
Ordinary part translation, rotation, pivot, hierarchy, and clip transforms
orient and animate it.

`front` emits only the front surface. `double` emits coincident front and back
surfaces with opposite winding and opposite normals. Both surfaces use the
part's material and texture; the back is the physical reverse view of the
same image. Separate back materials or textures are deferred until an authored
asset demonstrates that need.

## Render Contract

Two-sided cards compile to opposing geometry instead of disabling culling:

- the native prepared renderer retains its ordinary back-face-culling
  pipelines;
- front and back each have four vertices so their normals remain correct;
- each emitted side has its own exact diagnostic draw range;
- both sides resolve through the existing opaque, threshold-mask,
  dither-mask, blend, and additive pass buckets; and
- mono, per-eye stereo, placed actors, and multiview consume the same prepared
  vertices and ranges without a card-specific render path.

The Three.js preview uses matching finite plane geometry and explicit front or
double sidedness, including the existing blend depth prepass. This avoids the
oblique-angle parallax and hollow gap of two textured faces separated by a
thin box.

## Analysis Contract

- Geometry connectivity treats a card as an analysis-only epsilon-thick
  oriented bound. This does not give the card physical thickness at render
  time.
- Ground analysis samples all four transformed corners in every applicable
  pose.
- Surface analysis exposes exact `front` and `back` face identities and checks
  card/card and card/box coplanar overlaps. A card's own coincident opposing
  sides are intentional and are never compared with one another.
- The articulated head-socket rule remains box-specific.

## First Proof And Acceptance

The Maw Orchid's four petal boxes are the first proof. Their cutout artwork
must have one exact location with no front/back depth gap, remain readable
from both sides during Tendril Watch and Snap Trap, and stay visually stable
from front, side, and three-quarter views.

Acceptance requires semantic round-trip tests, compiler topology and pass
tests, card-aware analyzer fixtures, inspected Asset Lab sheets and videos,
an inspected semantic-versus-native prepared comparison, and the affected
shared/browser build and render gates.

## Acceptance Evidence

- All 185 canonical figures round-trip through the box-and-card validator.
  The 31 TypeScript tests include explicit-sidedness serialization,
  card/box coplanar overlap, and card-corner ground penetration fixtures.
- The geometry, ground, and surface gates report no unacknowledged failures.
  Maw Orchid contributes four planes while retaining its existing hierarchy
  and Tendril Watch and Snap Trap clips.
- Native compiler tests prove front-only and double-sided topology, opposing
  winding and normals, exact face ranges, alpha-pass routing, and rejection of
  omitted sidedness. `mclone-assets` passes 69 tests and `mclone-render` passes
  163 tests with eight capability skips.
- The prepared compiler identity is
  `mclone-prepared-figure-box-card-v3`. The Maw Orchid comparison reports 21
  parts, 17 boxes, four planes, 440 vertices, 660 indices, 110 exact ranges,
  six pipelines, and four immutable uploads.
- Front, side, and three-quarter Tendril Watch and Snap Trap sheets and videos
  were inspected under `/tmp/mclone-living-growths/card-final/`. The card is
  edge-thin from the side, has no hollow oblique gap, and stays readable from
  both faces. The native static and five-key animated comparisons preserve the
  same silhouette and resident topology.
- Asset Lab typechecking, web tests, production web build, catalogue drift,
  and all three asset analysis gates pass. The shared native web build and
  headed-Wayland browser WebGPU prepared-figure smoke pass with valid pixels,
  the v3 compiler, six pipelines, one draw, and no page errors.
