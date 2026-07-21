# Figure Geometry Analysis

Topic: `figure-geometry-analysis`

Status: implemented as a required canonical Asset Lab gate. The 155-figure
baseline has zero unacknowledged and zero acknowledged disconnected components.

## Scope

This topic owns conspicuously disconnected box components in canonical Asset
Lab figures and the narrow exception contract for intentionally floating
geometry. Same-facing coplanar surfaces and animated depth instability remain
owned by [`figure-surface-stability.md`](figure-surface-stability.md), while
sampled below-ground geometry is checked by
[`figure-ground-penetration.md`](figure-ground-penetration.md). General
collision, inverse kinematics, runtime physics, and curved legacy primitives
are outside this check.

## Motivation And Corrected Evidence

The Batch 31 Wallaby tail declared `pelvis` as its parent but its rotated root
box began beyond the pelvis surface, leaving a visible gap. A hierarchy link is
an animation relationship, not proof of geometric attachment.

The first full-catalog pass also found two existing kinds of genuine baseline
defect: the approved Kangaroo had the same separated tail-root construction,
both Moose antler palms sat beyond their antler roots, and two Starfish arms
were displaced by applying their radial rest rotation around an uncompensated
joint pivot. Those sources were reattached rather than exempted. Clean sheets
were inspected after each correction.

## Implemented Contract

[`geometry-analysis.ts`](../../tools/asset-lab/src/geometry-analysis.ts)
reconstructs every canonical box's rest-pose world transform with the same
`at + pivot`, Euler rotation, and parent-content hierarchy used by the Three.js
scene. Each box becomes an oriented bounding box. Pairwise boxes count as
connected when they intersect after sharing a total `0.06` figure-unit margin.
The resulting graph is split into components; every component outside the one
with the greatest total box volume is a diagnostic.

Unacknowledged diagnostics fail canonical `figure()` construction, catalogue
builds, and `pnpm asset-lab:test`. The direct catalogue command is:

```sh
pnpm asset-lab:geometry:check
```

An intentional floating component uses the additive semantic contract:

```ts
geometryException({
  rule: "disconnected-component",
  parts: ["halo"],
  reason: "The magical halo intentionally floats above the body.",
});
```

The part set must exactly match the reported component and the reason must be
nonempty. A changed component no longer matches. A reconnected component makes
the exception stale. Both conditions fail the gate, preventing broad or
forgotten suppressions. The standalone report prints acknowledged reasons so
reviewers and future authoring agents see them.

## Evidence And Validation

- The oriented-box pass catches the original Wallaby source as a detached
  three-part tail component with an approximately `0.068`-unit gap. The earlier
  axis-aligned prototype missed it because the empty corner of the rotated
  tail's broad-phase bounds overlapped the pelvis.
- The corrected Wallaby, Kangaroo, Moose, and Starfish pass and retain their
  approved silhouettes in clean review sheets.
- A semantic test proves that an unexplained floating component fails, an
  exact exception with a reason round-trips through JSON, a stale exception
  fails, and an empty reason fails.
- The required scan currently reports zero unacknowledged and zero acknowledged
  components across 155 canonical figures.

## Limits And Next Direction

This is a deterministic rest-pose authoring heuristic. The small margin allows
ordinary seams and near contact; it is not a collision skin or proof of exact
mesh continuity. Oriented-box expansion can still conservatively connect
geometry at corners. The scan deliberately excludes rounded legacy sources and
does not yet evaluate clip poses.

If animation exposes a recurring attachment defect, extend the same component
analysis over bounded clip samples and report the clip/time that first splits a
rest-connected component. Keep that separate from the face-level coplanarity
linter, and add a highlighted debug sheet before expanding the exception
vocabulary.
