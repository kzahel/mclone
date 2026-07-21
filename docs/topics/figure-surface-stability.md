# Figure Surface Stability

Topic: `figure-surface-stability`

Status: the Batch 23 pigeon coplanarity defect is corrected and visually
approved. A catalog-wide sampled-pose surface linter is accepted as the
recommended prevention mechanism but is deliberately not implemented yet.

## Scope

This topic owns depth instability caused by independently authored figure
parts presenting the same or nearly the same visible surface. It covers the
Asset Lab source model, animated-pose detection, authoring guidance, and a
future validation gate. General texture filtering, renderer depth precision,
and mesh compilation remain separate concerns except where they help
distinguish the source of a visible artifact.

The broader canonical box-only policy and figure compiler direction remain in
[`compiled-figure-rendering.md`](compiled-figure-rendering.md).

## Observed Defect And Accepted Correction

The Batch 23 rock pigeon exposed intermittent neck flicker during its flight
loop. The source had two relevant surface relationships:

- `neck` and `head` were both `0.50` units wide and centered on the same local
  X coordinate. Their volumes overlap at the joint, so their east faces both
  lay at `x = 0.25` and their west faces both lay at `x = -0.25`. These were
  same-facing, coplanar, overlapping exterior surfaces and were the actual
  visible depth conflict.
- The neck and breast north faces overlapped in projection with only `0.02`
  figure units of rest-pose depth separation. This was not the exact side-face
  conflict, but the animated neck made it an avoidable near-coplanar risk.

The approved source correction in
[`pigeon/figure.ts`](../../tools/asset-lab/examples/pigeon/figure.ts) narrows
the neck from `0.50` to `0.44`, leaving the `0.50` head with a deliberate
`0.03` step on each side. It also moves the breast back enough to increase the
neck-to-breast north-face gap from `0.02` to `0.06`. The silhouette remains
the same at review scale.

The corrected clean multi-angle sheet and four-cycle video live under
`/tmp/mclone-asset-lab/batch-23`. An enlarged neck contact sheet sampled the
whole loop. The user confirmed on 2026-07-21 that the z-fighting was gone.
Typechecking, the Asset Lab tests, the box-only boundary, first-party drift
check, and repository diff check pass.

## Current Rendering Facts

Asset Lab face textures do not create a second decal plane. Each texture is
assigned directly to the corresponding `BoxGeometry` face material in
[`scene.ts`](../../tools/asset-lab/src/scene.ts). The pigeon defect therefore
originated in two part surfaces, not in a textured overlay.

The current structural validation in
[`dsl.ts`](../../tools/asset-lab/src/dsl.ts) checks references, finite
transforms, primitive dimensions, clips, textures, and the canonical box-only
policy. It does not compare world-space surfaces between parts. Ordinary part
interpenetration is common and necessary at joints, so a naive bounding-box
overlap prohibition would reject most useful rigs without identifying the
actual depth hazard.

## Authoring Contract

- Intentional volume overlap at joints is allowed.
- Exposed, same-facing surfaces from different parts must not be exactly
  coplanar where their projected areas overlap.
- Near-coplanar layers should use a deliberate step whose margin exceeds the
  part's animated sweep plus a scale-relative safety tolerance.
- Surface markings belong in box-face textures rather than thin geometry laid
  flush against another face.
- Do not solve ordinary part conflicts with global depth bias,
  `polygonOffset`, disabled depth writes, or draw-order assumptions. Those
  mechanisms can hide one view while producing penetration or ordering errors
  from another.
- When a surface relationship is intentionally exceptional, document the
  exact part/face pair and reason instead of exempting an entire figure or all
  parent-child pairs.

## Proposed Sampled-Pose Surface Linter

Add a pure authoring-side check, initially exposed as a reporting command such
as `pnpm asset-lab:surface-check`, with this pipeline:

1. Load every canonical box-only figure.
2. Evaluate the rest pose and each clip at its key times, midpoints, and a
   bounded uniform sample cadence sufficient to catch macro-generated motion.
3. Transform the six face quads of every box into figure/world space.
4. Compare faces from different parts for parallel orientation, signed plane
   separation, and two-dimensional polygon overlap on the shared plane.
5. Report the figure, clip, sample time, both part/face names, plane gap, and
   overlap ratio.
6. Compare adjacent samples and report depth-order reversals or a swept face
   crossing even when no individual sample lands on exact equality.

The initial classifications should be:

- **error:** same-facing coplanar faces with meaningful area overlap;
- **error:** overlapping faces whose signed ordering crosses during a clip;
- **warning:** same-facing near-coplanar faces inside a scale-relative guard
  band;
- **diagnostic only:** opposite-facing shared boundaries or tiny edge contact.

Thresholds should scale with the complete figure bounds. A reasonable starting
point for catalog measurement is an exact-plane epsilon near `1e-5` of the
figure diagonal, a near-plane band near one percent of the diagonal, and a
minimum overlap near one percent of the smaller face. These are starting
values, not a frozen contract; the first full-catalog report should calibrate
them against reviewed figures.

Do not ignore parent-child pairs automatically: the pigeon defect occurred at
an attached joint, and other articulated defects can do the same. Suppressions
must be narrow, named, and justified.

## Rollout And Acceptance

1. Land the checker as a non-failing report and inventory the canonical
   catalog. Keep legacy curved comparisons out of the initial gate.
2. Review reported pairs with a debug sheet mode that highlights both face
   quads and labels their parts.
3. Fix confirmed defects and record narrow suppressions for intentional cases.
4. Ratchet CI to reject new exact/crossing defects, then promote the
   near-coplanar class once the baseline is understood.

The future checker is accepted when it catches a fixture shaped like the
original pigeon head/neck pair, ignores ordinary intersecting joint volumes
without coincident exposed planes, reports an animated crossing, and scans all
canonical examples deterministically.

## Separate Texture-Shimmer Follow-Up

If a reported visual flicker remains after the surface check is clean, inspect
texture minification separately. Asset Lab currently uses nearest filtering;
small pixel textures seen at oblique angles can shimmer without any duplicate
geometry. Mipmapped nearest minification and anisotropy are possible remedies,
but they must be reviewed for pixel-art softness and are not substitutes for
eliminating coplanar faces.

## Recommended Next Direction

Keep the checker deferred during the current three-at-a-time content workflow.
When surface cleanup becomes the selected slice, implement the reporting pass,
run it across the complete canonical catalog, and use the findings to choose
thresholds before making it a required gate.
