# Figure Surface Stability

Topic: `figure-surface-stability`

Status: implemented as a required sampled-pose Asset Lab gate for new
same-facing coplanar overlaps and insufficient animated head-socket margins.
The remaining catalog inventory is retained as 146 ratcheted warnings across
51 figures; new findings fail, exact source exceptions require reasons, and
stale warnings or exceptions fail. The confirmed Pigeon, Wild Turkey, Spotted
Hyena, and animated head/neck defects are corrected.

## Scope

This topic owns depth instability caused by independently authored figure
parts presenting the same visible plane. It covers transformed box faces,
sampled animation poses, authoring exceptions, the existing-warning ratchet,
and visual evidence. General texture filtering, renderer depth precision, and
mesh compilation remain separate concerns.

The broader canonical box-only policy remains in
[`compiled-figure-rendering.md`](compiled-figure-rendering.md). Disconnected
part components are checked separately by
[`figure-geometry-analysis.md`](figure-geometry-analysis.md).

## Confirmed Defects And Corrections

### Rock Pigeon

The Batch 23 Rock Pigeon had a `0.50`-wide neck and head centered on the same
local X coordinate. Their overlapping east and west faces were coplanar and
used visibly different surfaces. Narrowing the neck to `0.44` left a deliberate
`0.03` step per side. Moving the breast increased a separate near-plane margin
from `0.02` to `0.06`. The user confirmed the flicker was gone on 2026-07-21.

### Wild Turkey

The Wild Turkey's seven display feathers overlapped in the fan plane. All
north faces shared one depth and all south faces shared another while adjacent
feathers alternated bronze materials. Every adjacent pair therefore competed
over a meaningful projected area.

The corrected fan uses symmetric depth layers from `0.09` at the outer
feathers to `0.135` at the center. Adjacent feather faces retain the intended
overlap and silhouette but no longer share a plane. A clean strut sheet and
four-cycle video under `/tmp/mclone-asset-lab/surface-gate` show a stable fan.

### Spotted Hyena

The Spotted Hyena's head and neck were both `0.58` units wide. Their east and
west faces overlapped by about 15.9 percent and used different coat materials
throughout the lope. Narrowing the neck to `0.52` leaves a deliberate `0.03`
step per side. The clean sheet and four-cycle video under the same review
directory show the head-neck transition without depth interference.

### Animated Head Sockets

The Donkey exposed a broader animation defect: its neck and head were both
`0.42` units wide. Head pitch kept the two differently colored side surfaces
at, or close enough to, the same depth that the visible surface periodically
swapped. A frame could look correct while playback popped.

The new articulated seam rule found the same unsafe construction in 18
figures: American Bison, Bear, Camel, Chimpanzee, Cow, Deer, Donkey, Gemsbok
Oryx, Giraffe, Gorilla, Jaguar, Llama, Mallard Duck, Moose, Owl, Polar Bear,
Robin, and Toucan. Each head now has at least a `0.04` lateral step per side
over its neck, throat, or body socket. Clean sampled animation sheets for all
18 are under `/tmp/mclone-asset-lab/articulated-head-seams`.

## Implemented Analysis

[`surface-analysis.ts`](../../tools/asset-lab/src/surface-analysis.ts) performs
a pure authoring-side scan:

1. Evaluate the rest pose and every clip at authored key times, adjacent key
   midpoints, and a bounded uniform cadence of at most 24 intervals.
2. Reconstruct each part's exact preview hierarchy, pivot compensation,
   additive translation, quaternion-interpolated rotation, and scale.
3. Transform all six box faces to figure space.
4. Compare faces from different parts when they are same-facing within a
   `0.99999` normal dot threshold and have different effective material or
   texture appearances.
5. For the general rule, require a plane gap within `1e-5` of the sampled
   figure diagonal. For an animated `head` connected through `neck`,
   `neck_*`, `throat`, or `body`, require paired lateral faces to retain
   `max(0.02, figure diagonal * 0.0075)` separation.
6. Require at least one percent overlap relative to the smaller projected
   face, discard overlap regions immediately covered by a third box, then
   group all sampled occurrences by the exact part/face pair.

The animated rule requires both lateral faces to enter the guard over the
sampled motion. That distinguishes an undersized socket from a
deliberate one-sided contour. Its semantic scope also avoids treating every
tapered segment chain as a head joint. The appearance check avoids reporting
depth ties that cannot produce a visible color difference. The
immediate-coverage check removes common joint faces buried inside a larger
box. Neither check replaces rendered review.

Run the required summary directly with:

```sh
pnpm asset-lab:surface:check
```

Add `-- --verbose` to list every retained baseline warning. The check also runs
during canonical `figure()` construction, catalog builds, and
`pnpm asset-lab:test`.

## Gate, Baseline, And Exceptions

The first calibrated pass found 160 exact pairs across 55 existing figures
after fixing the Turkey and Hyena reports. Correcting the animated head sockets
removed 14 of those exact pairs and four entire figure entries, leaving 146
warnings across 51 figures. Those precise keys live in
[`surface-baseline.ts`](../../tools/asset-lab/src/surface-baseline.ts). They are
warnings and review debt, not approval that the geometry is good.

The baseline is a ratchet:

- a new pair is a failing authoring error;
- removing a pair makes its baseline entry stale and fails until the entry is
  removed, so the warning count can only deliberately decrease;
- changing part or face identity does not inherit an old warning;
- a new intentional pair must use a source-level exception with an exact pair
  and a nonempty reason.

Example:

```ts
surfaceException({
  rule: "coplanar-overlap",
  faces: [
    { part: "glass_inset", face: "north" },
    { part: "frame", face: "north" },
  ],
  reason: "The inset is deliberately flush and uses the same final pixels.",
});
```

An intentional animated socket uses the same exact-face contract with
`rule: "articulated-seam-margin"`. Both opposite faces must be acknowledged;
one broad part-level exception cannot suppress the relationship.

Exceptions are order-independent but match only that exact pair. A geometry
change that removes the finding makes the exception stale. Whole figures,
whole parts, and parent-child categories cannot be suppressed.

## Validation Evidence

- The original Turkey source reports all 12 north/south adjacent-feather face
  pairs. The stepped fan reports none.
- The original Hyena source reports both head/neck side pairs through the
  sampled lope. The narrowed neck reports neither.
- A semantic fixture shaped like the original Pigeon joint fails and prints
  the exact two faces plus suggested exception syntax.
- A near-but-not-coplanar animated head fixture with `0.01` side margins fails
  the articulated rule, while a `0.04` margin passes. Exact reasoned animated
  exceptions also round-trip through JSON.
- Tests prove an exact reasoned exception round-trips through JSON, an empty
  reason fails, a stale exception fails, and a defect appearing only at an
  animation key is detected.
- The current required scan reports zero failures, zero acknowledged source
  exceptions, and 146 ratcheted warnings across 158 canonical figures.
- The 23-test semantic suite, connectivity scan, first-party drift check, and
  typecheck pass.

## Authoring Contract

- Intentional volume overlap at joints is allowed.
- Differently rendered, same-facing surfaces must not share a plane where
  their projected areas overlap.
- Use a deliberate width, height, or depth step to establish one visible
  surface instead of relying on draw order.
- Animated heads must retain the articulated margin throughout their sampled
  clip poses; widen the head or narrow its socket instead of accepting a
  momentary depth winner.
- Surface markings belong in box-face textures rather than thin geometry laid
  flush against another face.
- Do not use global depth bias, `polygonOffset`, disabled depth writes, or draw
  order to hide ordinary figure conflicts.
- If a truly exceptional pair is required, name both parts and faces and state
  why the visual result is stable and intentional.

## Limits And Next Direction

The required classes are exact same-facing coplanarity at sampled poses and a
narrow near-plane guard for animated semantic head sockets. The scan does not
yet apply a general near-coplanar rule, infer a swept crossing between samples,
model full camera visibility, or inspect deprecated curved legacy primitives.
Its overlap coverage test samples representative points rather than computing
complete constructive solid geometry.

Use the verbose baseline report and rendered sheets to review high-area or
highly visible pairs in small batches. Fix confirmed problems and delete their
keys from the baseline in the same change. After that debt is understood,
extend the analyzer with a non-failing general near-plane class and depth-order
crossing evidence before deciding whether either class is precise enough to
gate outside the calibrated head-socket case.

## Separate Texture-Shimmer Follow-Up

If visible flicker remains after the surface check is clean, inspect texture
minification separately. Asset Lab currently uses nearest filtering; small
pixel textures viewed obliquely can shimmer without duplicate geometry.
Mipmapped nearest minification and anisotropy are possible remedies, but they
must be reviewed for pixel-art softness and do not substitute for separating
coplanar faces.
