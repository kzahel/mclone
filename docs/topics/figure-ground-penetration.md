# Figure Ground Penetration

Topic: `figure-ground-penetration`

Status: implemented as a required sampled-pose Asset Lab gate for canonical
land figures. Existing catalog debt is retained as 36 ratcheted warnings
across 10 figures; new findings fail, exact source exceptions require reasons,
and stale warnings or exceptions fail. The Batch 32 Chameleon tail defect is
corrected.

## Scope

This topic owns authored box or plane geometry crossing below the figure-space ground
plane during rest, land locomotion, idle, or action clips. It covers pose
sampling, land-clip admission, declared locomotion contacts, exact-part
exceptions, and the existing-warning ratchet.

It does not solve runtime terrain contact, inverse kinematics, foot placement,
or collision. Disconnected components remain owned by
[`figure-geometry-analysis.md`](figure-geometry-analysis.md), while coplanar
and animated surface conflicts remain owned by
[`figure-surface-stability.md`](figure-surface-stability.md).

## Confirmed Defect And Correction

The first Batch 32 Chameleon tail curled downward because each positive local
X rotation accumulated through its six-part hierarchy. The final tail tip
reached approximately `y=-0.436`, visibly passing far below the preview floor.

The corrected hierarchy reverses the curl direction. Its relative rotations
now carry the tail upward and inward into an angular coil while preserving the
same six connected boxes and subtle locomotion sway. The clean creep sheet and
four-cycle video under `/tmp/mclone-asset-lab/batch-32-six` are the review
evidence.

## Implemented Analysis

[`ground-analysis.ts`](../../tools/asset-lab/src/ground-analysis.ts):

1. Admits figures with explicit `metadata.habitats` containing `land`, or for
   older unclassified sources at least one `biped-walk`, `quadruped-walk`, or
   `slither` locomotion clip.
2. Samples rest plus land-locomotion, idle, and action poses at the same keys,
   midpoints, and bounded uniform cadence used by surface analysis.
3. Excludes `swim` and `wing-flap` poses, even for hybrid figures.
4. Reconstructs every sampled box or plane through the complete parent, pivot,
   translation, quaternion-interpolated rotation, and scale hierarchy.
5. Finds the lowest transformed corner of every non-contact primitive.
6. Reports a part when that corner passes more than `0.02` figure units below
   `y=0`.

Parts explicitly named by land locomotion `contacts` are excluded. Those boxes
are load-bearing pads whose separate authoring and procedural contact rules may
place them slightly through the plane. Their ancestors, decorations, tails,
trunks, and every other box remain checked, so declaring one foot cannot hide
an unrelated penetration.

Run the summary directly with:

```sh
pnpm asset-lab:ground:check
```

Add `-- --verbose` to list every retained baseline warning. The gate also runs
during canonical `figure()` construction, catalogue builds, and
`pnpm asset-lab:test`.

## Baseline And Exceptions

The calibrated first inventory found 36 penetrating parts across 10 existing
figures after correcting Chameleon and the new Hamster's slight leg-shaft dip.
Their exact figure/part keys live in
[`ground-baseline.ts`](../../tools/asset-lab/src/ground-baseline.ts). They are
warnings and review debt, not approval that the animation is correct.

The baseline is a ratchet: new parts fail, removed findings make their keys
stale, and identity changes do not inherit old warnings. A truly intentional
relationship uses one exact part and a nonempty reason:

```ts
geometryException({
  rule: "ground-penetration",
  parts: ["burrowing_claw"],
  reason: "This action deliberately pushes the named claw into loose soil.",
});
```

The exception becomes stale when the part no longer penetrates. Whole figures,
multi-part groups, and arbitrary depth allowances cannot be suppressed.

## Validation Evidence

- The original Chameleon reports `tail_tip` at about `y=-0.436`; the corrected
  source reports no ground issue.
- A semantic fixture that penetrates only at an animation key fails and names
  the part, clip, time, depth, and suggested exception syntax.
- Tests prove exact reasoned exceptions round-trip, stale exceptions fail,
  declared locomotion contacts are excluded, explicit land metadata admits
  custom idle/action figures, and swim-only figures are outside the land gate.
- The required scan reports zero failures, zero acknowledged source
  exceptions, and 36 baseline warnings across 185 canonical figures.
- The 31-test semantic suite, connectivity and surface scans, typecheck, and
  production catalogue build pass.

## Limits And Next Direction

The ground plane is the authoring convention `y=0`; this is not a terrain
query. Corner sampling is exact for rigid boxes and planes at each sampled pose but
does not prove that no crossing occurs between samples. A declared contact pad
is exempt as a whole, so its own excessive penetration remains separate debt
for the procedural contact system.

Review the highest-depth baseline entries first: humanoid leg shafts,
Roly-poly roll actions, and unsupported hopping feet. Remove each baseline key
in the same change that corrects its source. If future figures need burrowing
or digging, prefer explicit action semantics before expanding the exception
vocabulary.
