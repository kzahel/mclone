# Tactical 252: Procedural Horizon Seams And Transition Admission

Status: proposed; diagnosed and deferred 2026-07-26.

Topic: `procedural-horizon-clipmap`

Parent directions:

- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns ring seams, toroidal residency, requested/committed origins, and
  bounded frame admission; and
- [`249`](249-cross-platform-procedural-horizon-proof.md) completed the first
  native/browser clipmap proof and explicitly deferred retained committed
  origins and seam hardening.

## Objective

Remove two independently diagnosed defects without trading the fixed-work
clipmap for frame-time spikes:

1. persistent tile and fine/coarse lighting discontinuities; and
2. a one-frame coarse-terrain flash when several aligned levels advance
   together.

This document records the diagnosis and likely solution boundaries. Runtime
implementation is intentionally deferred.

## Reported Evidence

Interactive review at an 80-block view showed stable rectangular and diagonal
shading boundaries after all ten levels had returned to `160/160` ready slots,
zero pending refills, and 160 draws. These are steady-state shading defects,
not uncovered holes.

The same review reported an occasional single-frame terrain, lighting, or
camera-like jump while crossing patch boundaries. The clearest occurrence was
near center Z `2048`. Frames captured immediately afterward were fully ready,
so the transient itself was not present in the still images.

## Diagnosis: Edge Normals Lack A Neighbor Halo

Each 64-cell tile stores the 65-by-65 vertices it draws. The render shader
estimates a vertex normal from neighboring height samples, but clamps the
lookup to the local tile at its four edges. Two tiles can therefore share the
same world-space edge height while deriving different one-sided slopes and
lighting for that edge.

This is not propagated Minecraft block-light disagreement. It is preview
directional lighting calculated from tile-local height derivatives.

Fine/coarse boundaries add a second disagreement: the two levels approximate
the surface at different sample spacing. Spatial alignment and fragment
ownership avoid most holes, but they do not weld normals or reconcile the
lighting footprint. Explicit skirts are also still deferred; skirts can hide
positional cracks but cannot repair mismatched surface normals.

Likely implementation:

- evaluate a one-sample world-space halo around each tile, such as 67-by-67
  samples for a drawn 65-by-65 vertex grid;
- calculate every drawn edge normal from the same absolute neighbor samples;
- keep halo samples out of the drawn topology and account for their fixed
  memory and compute cost; and
- define a separate fine/coarse normal policy, potentially a narrow
  transition collar or footprint-aware normal blend.

Per-fragment geometric lighting is a useful diagnostic comparator, but it
would trade the current smooth surface for face-level shading and is not by
itself the preferred final fix.

## Diagnosis: New Origins Are Exposed Before They Are Complete

The finest tile footprint is 64 blocks and every coarser level is
power-of-two aligned. Crossing from Z `2047` to `2048` advances six levels at
once. A single-axis shift requests four entering tiles per level:

```text
six advancing levels * four entering tiles = 24 refills
current per-frame GPU dispatch budget       = 16 refills
```

The refill queue is coarse-first. `set_view` immediately assigns the reused
toroidal slots to the requested tiles and marks them unready. Rendering then
omits any level without all 16 assigned tiles ready. At this boundary the
first frame completes the coarser 16 refills while the two finest levels
remain incomplete. A coarser level removes its inner hole and temporarily
fills the center. The remaining eight refills finish on the next frame and
the fine levels reappear.

That fallback changes sampled geometry, material decisions, normals, and
vegetation together. It can resemble a camera-height jump, but the Horizon
camera target remains fixed at the overworld sea-level datum.

Likely implementation:

- retain distinct requested and committed origins per level;
- generate an entering row/column into bounded staging or guard slots without
  invalidating the currently committed coverage;
- switch the level and its parent/child hole ownership atomically only when
  the replacement strip is drawable; and
- preserve bounded per-frame work and explicit stale-request cancellation.

Increasing the dispatch budget or merely prioritizing fine tiles may conceal
selected crossings, but it introduces device-dependent spikes or holes
between noncontiguous ready levels. Neither is the durable XR-safe solution.

## Acceptance For A Later Implementation

- Same-LOD borders have matching height-derived normals under a diagnostic
  seam visualization and no visible rectangular lighting grid.
- Fine/coarse boundaries remain covered and have an explicit, inspected
  normal and skirt/collar policy.
- Single-axis, diagonal, negative-coordinate, and teleport movement never
  exposes a partially admitted origin.
- Power-of-two coincidence boundaries, including `2048` and world zero, keep
  continuous fine coverage under an intentionally constrained refill budget.
- Diagnostics distinguish requested, staged, and committed origins and report
  atomic level admissions.
- Native, headed-browser, Android, and XR-relevant frame admission retain
  bounded work; no acceptance relies on raising the dispatch budget until the
  defect disappears.
