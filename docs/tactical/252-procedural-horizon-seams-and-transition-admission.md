# Tactical 252: Procedural Horizon Seams And Transition Admission

Status: active 2026-07-26. The original diagnosis predates Tacticals
253–256; desktop and browser review after the shared vegetation-worker
cutover confirmed the transition defect on both executors. Slices 0–2 are
complete and Slice 3 is next.

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

This document records the diagnosis, the selected implementation boundary,
and the cross-host acceptance evidence.

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

Tactical
[`256`](256-shared-horizon-vegetation-worker-topology.md) subsequently moved
tree-record compilation behind the same shared coordinator on a native thread
and browser Worker. Post-cutover review showed a second expression of the same
premature-admission defect on both hosts: `set_view` immediately clears tree
buffers in reassigned toroidal slots, while replacement products arrive
asynchronously. A level can therefore be terrain-drawable for one frame
without a complete vegetation representation.

Likely implementation:

- retain distinct requested and committed origins per level;
- generate an entering row/column into bounded staging or guard slots without
  invalidating the currently committed coverage;
- switch the level and its parent/child hole ownership atomically only when
  the replacement strip is drawable; and
- preserve bounded per-frame work and explicit stale-request cancellation.

Terrain and vegetation do not need one monolithic commit barrier. Terrain
should atomically switch from its previous complete origin to its next
complete origin. Vegetation may retain the previous valid parent/committed
representation across that terrain switch until the replacement child
products are drawable, then switch without an uncovered frame. It must never
draw old-source vegetation after a seed, profile, topology, content-stage, or
compiler-source change.

Increasing the dispatch budget or merely prioritizing fine tiles may conceal
selected crossings, but it introduces device-dependent spikes or holes
between noncontiguous ready levels. Neither is the durable XR-safe solution.

## Implementation Order

### Slice 0: executable contract and status repair

Status: complete 2026-07-26.

1. Correct the Tactical 253/256 completion status in the tactical index.
2. Activate this tactical under the existing
   `procedural-horizon-clipmap` topic.
3. Refine the original terrain-only staging direction with the now-proven
   asynchronous vegetation readiness and source-lifetime contract.
4. Keep platform executors, forest algorithms, and representation thresholds
   unchanged.

### Slice 1: requested, staged, and committed admission

Status: complete 2026-07-26.

1. Give every level explicit requested and committed origins.
2. Preserve currently drawable slot resources while entering strips are
   generated into bounded staging resources.
3. Commit a level and its fine/coarse hole ownership atomically.
4. Retain a valid vegetation representation until replacement products are
   drawable, with source reset as the only immediate invalidation.
5. Add deterministic constrained-budget tests at ordinary, aligned,
   negative, diagonal, and teleport transitions.

The implementation adds seven fixed guard resources to each four-by-four
level. That is exactly the largest entering set for a one-cell diagonal move:
four row tiles plus three non-duplicate column tiles. Ten levels therefore
retain `160` logical resources and add `70` staging resources, for `230`
bounded terrain/vegetation resource slots. Ordinary movement advances each
requested origin by at most one tile per frame-state update; a true long jump
uses the explicit rebase path. The default global rebase threshold is `4,096`
blocks.

Terrain and vegetation expose separate complete presentations over those
resources. A terrain level commits its new origin only after all 16 logical
tiles are drawable. Vegetation retains the previous complete level until
every replacement product for that level is drawable, then switches the
whole level without an uncovered frame. A source reset remains the only
immediate invalidation and removes both presentations before accepting new
source work.

The fixed allocation rose from `86,553,600` to `124,457,600` bytes:
`37,904,000` bytes, or `43.8%`, for bounded staging resources and the
separate vegetation presentation uniform. No transition increases that
allocation.

Pure admission tests cover partial aligned transitions, diagonal guard
capacity, retained vegetation, and teleport reuse. Native/offscreen smoke at
the `2047` to `2048` coincidence boundary retained `160` ready logical tiles,
ten committed terrain levels, and three committed vegetation levels on every
movement frame. The headed desktop-browser lane passed the same continuous
coverage assertions through held motion, Worker termination/restart, negative
coordinates, and a million-block teleport. Its held-motion frames kept the
previous `2,609`-tree presentation until the replacement forest committed.

### Slice 2: shared normal halo and fine/coarse seam policy

Status: complete 2026-07-26.

1. Evaluate one world-space sample beyond every drawn tile edge.
2. Derive same-LOD edge normals from identical absolute neighbor samples.
3. Keep halo values out of drawn topology and report their fixed cost.
4. Select and prove a bounded fine/coarse normal policy without alpha
   crossfades or device-dependent work spikes.

The horizon now stores a fixed two-sample halo around each tile: `69x69`
samples for the unchanged `65x65` drawn vertices and `64x64` cells. The first
halo sample makes both sides of a same-LOD tile border use the same absolute
central-difference inputs. The second implements the fine/coarse policy: over
the outer two-cell collar of every fine level except the coarsest ring, the
normal footprint widens smoothly from one to two fine samples. At the shared
edge that is exactly the adjacent coarse level's one-sample world footprint.
The shader changes neither topology nor fragment ownership and uses no alpha
crossfade.

Each resource stores `536` additional samples, a `12.7%` sample/dispatch
increase over the drawn grid and `68,608` bytes per resource. Across the 230
fixed resources this is `15,779,840` bytes. Fixed residency therefore rose
from the Slice 1 value of `124,457,600` to `140,237,440` bytes and remains
constant across every transition.

The shader-layout tests prove the fixed `65`/`69` topology split, outer-edge
selection, and identical fine-wide/coarse-narrow world footprints. All 49
terrain-view tests and the native/Wasm Explorer tests pass. The aligned
native/offscreen smoke completed with the new allocation and visually
inspected 3D, movement, negative, teleport, map, and orbit captures. In the
same local lane, process time to the first complete target rose from
`354.9 ms` to `399.3 ms`; movement mean/P95 encode time rose from
`2.52/4.19 ms` to `3.31/5.68 ms`.

The headed Wayland desktop-browser lane also passed initial, pointer,
shift-pan, two-contact, forced Worker restart plus held motion, negative,
teleport, and shutdown checks. Its inspected movement capture retained a
coherent forest and surface; all checkpoints reported the exact
`15,779,840`-byte halo. The Wasm artifact is `1,708,486` bytes, `4,777` bytes
larger than Slice 1.

### Slice 3: cross-host closeout

1. Add requested/staged/committed transition diagnostics.
2. Exercise the constrained aligned-boundary transition on native and
   browser, including vegetation-worker delay.
3. Inspect stationary, movement, zoom, negative, diagonal, and teleport
   pixels.
4. Record memory/work deltas and close this tactical without starting game
   scene integration.

## Acceptance

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
- Desktop and browser retain continuous forest presentation through the same
  transition sequence despite their different executor latency.
- Old-source vegetation is removed immediately; same-source parent or
  committed vegetation remains until replacement records are drawable.
- Native, headed-browser, Android, and XR-relevant frame admission retain
  bounded work; no acceptance relies on raising the dispatch budget until the
  defect disappears.
