# Tactical 321: Exact Frontier Support Architecture

Status: active architecture and diagnostic phase 2026-08-20; no
pixel-changing frontier implementation is authorized before Human Review A

Topic: `procedural-horizon-clipmap`

## Instruction Synthesis

Take an architectural step back before applying another local LOD seam fix.
The procedural horizon has progressed through several useful iterations:
fixed toroidal rings, atomic strip admission, exact-painted ownership,
connected exact admission, appearance convergence, direct smooth topology,
and an exact-profile connector. The result is converging toward the desired
system, but those improvements have also accumulated assumptions which are
not yet expressed as one complete composition contract.

Investigate and document the whole system before changing its pixels. In
particular, determine how the regular distance-based clipmap should support a
focus-connected but potentially irregular exact region whose perimeter can
extend beyond the fixed finest ring. Establish bounded performance and
adversarial-shape behavior, add a proof that every admitted exact edge has a
valid procedural neighbor, compare architectural remedies, and require human
acceptance before selecting or implementing one.

When the design has solidified, consolidate the durable result into
[`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md).
That living topic must become the detailed current-system description; this
tactical remains the bounded investigation, decision, implementation, and
evidence record.

## Problem Statement

The current composed terrain combines two independent spatial systems:

```text
camera focus                         ready exact render columns
     |                                         |
     v                                         v
fixed nested toroidal rings          focus-connected exact component
     |                                         |
     |                              mask + appearance + boundary facts
     |                                         |
     +-------------------+---------------------+
                         v
                 late render composition
          procedural discard + edge connector
```

The procedural clipmap remains regular and distance-based. Every level owns a
four-by-four grid of 64-cell tiles, with power-of-two sample spacing and a
rectangular central hole occupied by the preceding finer level. Requested
movement prepares entering tile strips and commits each complete level
atomically.

Exact admission is deliberately not rectangular. The scene selects the
four-connected ready component containing, or still validly connected to,
the player focus. One immutable generation supplies the exact draw set,
procedural discard, 32-block appearance field, exposed boundary profile,
connector instances, and vegetation ownership. Disconnected ready islands
remain procedural.

Those systems currently meet only while preparing and drawing the frame. The
exact connector is selected and drawn only through sample-spacing-one tiles,
but exact admission does not prove that its entire perimeter remains inside
the finest clipmap extent. The missing invariant is:

> Every exposed edge of an admitted exact generation has one ready,
> generation-coherent procedural neighbor and a complete geometry-closure
> path at the resolution claimed by the transition contract.

Without that proof, a larger exact view can escape level zero, meet a
spacing-two or coarser ring directly, and receive no connector. Height
disagreement then exposes the sky even though exact and procedural horizontal
ownership remain binary.

## Starting Evidence

The current shared preset family does not lower near-field sample spacing.
Low, Medium, and High all use base spacing one and a four-by-four level-zero
grid; they change only outer level count and proxy-vegetation reach. Each
spacing-one tile covers 64 blocks, so the complete fixed level-zero bounds are
256 by 256 blocks.

The grid is snapped around the 64-block tile containing the focus rather than
around the chunk-aligned exact window. Its available margin is therefore
phase-dependent and asymmetric. Increasing exact render distance can exhaust
one side before another even when the exact footprint is narrower than 256
blocks.

A matched settled native diagnostic at seed `12345`, center chunk `(0, 0)`,
and the same low camera observed:

| Exact render distance | Exact chunks | Exact width | Visible connector segments |
|---:|---:|---:|---:|
| 2 | 25 | 80 blocks | 277 |
| 8 | 289 | 272 blocks | 0 |

The render-distance-eight footprint cannot fit inside the 256-block finest
extent. The gentle diagnostic site happened not to expose a large sky wedge,
but the settled receipt proves that the viewed exact frontier had no connector.
A separate elevated render-distance-eight view retained only 64 connector
segments where part of the perimeter still intersected spacing-one tiles,
confirming that position and direction change which edges are protected.

The audit has also identified a capacity mismatch which must be resolved
explicitly. Desktop accepts exact render distance 32, whose complete square is
65 by 65 chunks. `ExactPaintedCoverageSnapshot`, its GPU mask, the transition
field, and boundary profile currently allow a maximum span of 64 chunks per
axis. A fully ready legal view can therefore exceed the composition format
before frontier support is considered.

Current performance evidence establishes a constrained baseline:

| Preset | Logical clipmap tiles | Fixed residency |
|---|---:|---:|
| Low | 96 | approximately 81.6 MiB |
| Medium | 128 | approximately 107.4 MiB |
| High | 160 | approximately 133.2 MiB |

Each level also owns seven staging resources so movement can retain its prior
committed presentation. The measured fixed-byte differences imply roughly
0.56 MiB per fully provisioned terrain resource before dynamic vegetation.
Quest Low is already near its 72 Hz frame target at approximately 12.65 ms
average app work. New frontier support must therefore remain explicitly
bounded and measured; `local` or `perimeter-only` is not by itself a budget.

## Objective

Define and prove one shared exact/procedural composition architecture which:

- retains the regular fixed-budget toroidal clipmap as the distant-terrain
  foundation unless evidence at Human Review A selects a replacement;
- admits no exact generation with an unsupported exposed edge;
- gives the preferred exact handoff a spacing-one smooth procedural neighbor;
- preserves one horizontal geometry owner at every location;
- supports ordinary square coverage and connected irregular coverage,
  including holes and concave boundaries;
- bounds memory, generation, upload, connector, draw, and admission work for
  every legal exact render distance and Distant Terrain preset;
- keeps source identity, coverage generation, terrain, water, vegetation,
  mono, per-eye, and multiview decisions coherent;
- defines deterministic behavior when preferred support exceeds its budget;
  and
- leaves a clear, smaller implementation boundary rather than adding more
  exact-frontier policy directly to the monolithic viewport renderer.

## Binding Architecture Constraints

### Diagnose the composition, not only the visible crack

Do not begin by increasing `tiles_per_axis`, moving the clipmap origin, or
letting coarse tiles emit the current connector. First produce a
renderer-neutral frontier analysis which combines one committed procedural
presentation with one prepared exact generation and accounts for every
exposed boundary edge.

The analysis must distinguish:

- the procedural level and sample spacing bordering each exact edge;
- whether the required procedural samples are committed and drawable;
- whether exact solid, exact water, or an unsupported volumetric silhouette
  owns the boundary fact;
- whether connector geometry covers that edge;
- which preferred fine-support tiles would be needed;
- the inner and outer transition topology of those tiles; and
- the current and worst-case resource cost.

### Introduce a composition certificate

The settled design must expose a typed composition plan or equivalent
certificate before renderer submission. Names are not fixed, but its role is
equivalent to:

```text
TerrainFrontierPlan {
    source_and_generation,
    exact_coverage,
    committed_procedural_coverage,
    boundary_classification,
    desired_fine_support,
    closure_policy,
    readiness,
    bounded_cost_receipt,
}
```

The plan belongs in shared `mclone-terrain-view` ownership. `mclone-scene`
continues to provide authoritative exact readiness and frame orchestration;
apps remain platform adapters. Mono, per-eye, and multiview consume the same
immutable plan rather than preparing per-view frontier geometry.

Exact coverage, procedural discard, connector geometry, and any fine-support
layer must commit coherently. If a new exact generation is ready before its
required support, retain the last complete ownership snapshot or another
explicit complete owner. Never expose an intermediate unsupported edge.

### Preserve bounded degradation

The normal steady-state footprint is expected to be compact and approximately
square, but correctness must not depend on that expectation. A connected
component may still be an L, staircase, ring with a hole, narrow corridor,
comb, or temporarily jagged streaming frontier. A perimeter algorithm can
approach area-proportional work for adversarial connected shapes.

Any selected fine-support pool must therefore have a validated cap, reuse and
hysteresis policy, and deterministic exhaustion behavior. Exhaustion may not
silently allocate, crack, overlap opaque surfaces, or change policy by host.
Platform profiles may choose different default Distant Terrain presets, but a
given preset and render-distance input retain shared semantics.

### Keep ownership binary

The exact footprint continues to discard every procedural horizontal surface,
including water. A fine-support belt, if selected, owns only procedural-side
horizontal locations. Its outer transition must hand ownership back to the
base clipmap through an explicit aligned stitch, skirt, zipper, or other
single-owner topology. Depth order, alpha, dithering, fog, and coincident
overdraw are not substitutes for that ownership decision.

## Candidate Architecture Set

Human Review A selects the implementation direction after diagnostics and
cost evidence. The current recommendation is not binding before that gate.

### A. Expand the complete finest clipmap extent

Give level zero a dynamic or render-distance-derived rectangular extent large
enough to contain exact coverage plus a support halo. This offers simple
ownership and can reuse rectangular fine/coarse stitching, but the current
clipmap and admission pools assume the same tile axis for every level. At
render distance 32, a 32-block halo needs roughly an 18-by-18 spacing-one
extent instead of 4-by-4, adding roughly 170 MiB before staging and vegetation.
This is the simplest correctness model and the weakest cross-platform cost
candidate.

### B. Add a sparse spacing-one frontier-support layer

Keep the base rings unchanged and allocate only spacing-one tiles intersecting
a bounded band around the actual exact perimeter. This preserves the accepted
exact-to-spacing-one character and scales with ordinary perimeter rather than
filled area. It requires a separate bounded admission pool, coarse ownership
cuts beneath the support, outer support-to-clipmap stitching, generation
coalescing, and explicit handling where the band self-intersects around holes
or narrow features.

### C. Make the connector resolution-aware

Generate a connector against whichever procedural level actually borders the
exact edge. This is likely the lowest-residency robust closure, but it places
spacing-two or coarser topology directly beside exact blocks and does not
preserve the preferred near-field character. It also requires the connector
to match the active coarse triangle profile rather than merely evaluating a
spacing-one endpoint. Treat it as a deliberate product candidate or bounded
fallback, not as permission to remove the existing spacing-one contract.

### D. Use a bounded hybrid

Prefer a sparse spacing-one support layer within a measured pool. When an
otherwise legal exact footprint or pathological connected perimeter exceeds
that pool, use one explicitly accepted fallback: resolution-aware closure,
retention of the last fully supported exact generation, or a support-aware
exact admission bound. This candidate currently best preserves ordinary
pixels while keeping worst-case behavior defined, but Human Review A must see
the cost and complexity evidence before selecting it.

Merely shifting the existing four-by-four level-zero origin can improve phase
alignment at moderate distances but cannot contain a footprint wider than 256
blocks. It is a possible optimization inside another candidate, not a complete
architecture.

## Required Diagnostics And Capacity Ledger

Before a pixel-changing candidate, add deterministic diagnostics for:

- total exposed exact block edges;
- exposed edges grouped by bordering procedural sample spacing;
- edges with spacing-one support;
- edges with a connector, water-specific closure, selected fallback, or no
  closure;
- maximum and distribution of exact-to-procedural spacing;
- desired, resident, pending, committed, and rejected fine-support tiles;
- support pool bytes, connector bytes, submitted vertices, dispatches,
  uploads, preparation CPU, and generation churn;
- exact generations delayed, coalesced, retained, or rejected for support;
- legal render-distance width versus mask, boundary, transition, clipmap, and
  support capacities; and
- one diagnostic presentation which marks unsupported or fallback boundary
  segments without relying on a naturally visible sky crack.

Do not infer a bound from one regular square. Record formulas and validated
maximums for the 64-chunk mask limit, legal render distances, topology period,
tile size, halo width, support pool, connector format, and staging policy.

## Adversarial Evidence Matrix

The architecture and selected implementation must cover the cross-product
which exposed the current blind spot, not isolated unit fixtures:

- exact render distances `2`, `5`, `8`, `13`, `31`, and `32` or the final
  explicitly supported maximum;
- all four chunk phases within a 64-block spacing-one tile on both axes;
- exact squares, rectangles, L shapes, staircases, concave bays, holes,
  narrow corridors, bounded combs, growth, eviction, and a suppressed
  disconnected island;
- exact boundaries wholly inside level zero, exactly on its edge, crossing
  into level one, and crossing multiple fine extents;
- flat land, steep snow/stone, coast, exact/procedural water, trees crossing
  ownership, and a known unsupported volumetric silhouette;
- sub-block motion, chunk motion, 64-block clipmap rebases, rapid view churn,
  teleport, source reset, and delayed exact readiness;
- positive and negative coordinates plus plane and cylinder topology seams;
  and
- mono, synthetic stereo, normal per-eye XR, and full-frame multiview.

Tests should generate compact shape/phase fixtures directly. Pixel campaigns
should use a smaller representative matrix selected from those objective
receipts rather than multiplying every case into screenshots.

## Execution Phases And Human Gates

### Phase 0: Architectural ledger and invariant audit

- [ ] Record the current clipmap, admission, exact snapshot, transition,
      connector, water, vegetation, and draw data flow with actual owners.
- [ ] Reconcile every legal render distance with the 64-chunk exact format and
      finest-ring capacity.
- [ ] Identify every late renderer assumption which should become a prepared
      composition fact.
- [ ] Specify the frontier plan/certificate inputs, outputs, lifetime, and
      failure behavior without changing pixels.

Gate -- Human Review A1: accept or adjust the current-system model and the
missing-invariant diagnosis before instrumentation changes its public shape.

### Phase 1: Diagnostic frontier plan

- [ ] Implement renderer-neutral boundary classification and capacity
      receipts without changing ordinary geometry or ownership.
- [ ] Expose unsupported edges, bordering levels, hypothetical candidate
      support sets, and candidate memory/draw estimates.
- [ ] Add the adversarial shape, phase, render-distance, motion, and topology
      objective matrix.
- [ ] Capture matched natural and diagnostic pixels at low and high exact
      render distances and verify the receipts against inspected seams.
- [ ] Measure boundary-profile preparation separately from the existing
      transition-field timing.

Gate -- Human Review A2: review the measured frontier distribution, common
case, worst case, memory/performance projections, and diagnostic pixels.
Select candidate A, B, C, D, or explicitly request further research. No
candidate implementation begins without this decision.

### Phase 2: Selected topology proof

- [ ] Implement the smallest isolated proof of the selected support and outer
      closure topology in shared ownership.
- [ ] Keep the ordinary product path unchanged until exact, support, and base
      ownership assertions pass.
- [ ] Prove solid, water, hole, concave, negative-coordinate, and level-edge
      cases with one complete owner and no unsupported segment.
- [ ] Measure pool bytes, generated tiles, vertices, dispatches, uploads, and
      steady draw cost against the committed baseline.

Gate -- Human Review B: inspect matched exact/frontier pixels and accept the
topology before it becomes the ordinary composition path.

### Phase 3: Generation-coherent live admission

- [ ] Integrate the selected support plan with requested, staged, and
      committed procedural presentations.
- [ ] Gate exact generation changes on a complete composition certificate;
      coalesce stale support work and retain the prior complete owner.
- [ ] Apply the same immutable plan to terrain discard, connector geometry,
      water, vegetation, mono, per-eye, and multiview rendering.
- [ ] Give support exhaustion and legal-capacity mismatch one explicit tested
      policy.

Gate -- Human Review C: accept static and streaming exact-to-LOD transitions
at low and high render distance before broad platform performance work.

### Phase 4: Motion, platform, and performance acceptance

- [ ] Run continuous sub-block, chunk, clipmap-rebase, view-distance change,
      and teleport scenarios without a one-frame crack or duplicate owner.
- [ ] Inspect native, headed WebGPU, flat Android, stereo, and XR pixels.
- [ ] Measure Low/Medium/High steady state and transition bursts on native,
      browser, and physical Quest using warmed pipelines.
- [ ] Confirm Off remains allocation-free and unsupported sources remain Off
      without frontier work.
- [ ] Tune only inside the accepted topology and fixed capacity policy.

Gate -- Human Review D: accept the final pixels, motion behavior, degradation
policy, and platform performance.

### Phase 5: Ownership cleanup and topic consolidation

- [ ] Extract settled frontier planning, support admission, and resource
      ownership from `viewport_renderer.rs` into focused shared modules.
- [ ] Delete rejected candidate code, temporary selectors, diagnostic-only
      allocations, and superseded assumptions.
- [ ] Update this tactical with the chosen architecture, measured budgets,
      revisions, and acceptance evidence.
- [ ] Rewrite the relevant portions of
      [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
      as the canonical detailed system description.

The final topic document must explain, in current rather than historical
terms:

- clipmap geometry, toroidal residency, staging, and preset budgets;
- exact readiness, connected admission, coverage capacity, and source identity;
- one-frame terrain, water, connector, and vegetation ownership;
- frontier planning, preferred support, fallback, and admission certificates;
- movement, generation changes, teleport, failure, and device-loss recovery;
- mono, per-eye, and multiview consumption;
- performance bounds and diagnostics; and
- accepted limitations and deferred volumetric/edit behavior.

Gate -- Human Review E: verify that the topic document can serve as the
standalone current-system reference and that this tactical is only an
execution record.

## Acceptance

- Every admitted exact edge has a diagnostic and enforced closure owner.
- Ordinary exact terrain meets spacing-one smooth terrain before the base
  clipmap becomes coarser, unless Human Review explicitly accepts a named
  bounded fallback.
- No legal render-distance input can exceed an implicit mask, boundary,
  transition, clipmap, or support limit.
- Irregular connected coverage remains correct and its work remains bounded.
- Exact growth and clipmap movement cannot expose a partially prepared
  composition generation.
- Fine support and its outer transition do not overlap base procedural or
  exact horizontal geometry.
- Water and vegetation use the same generation and ownership plan as land.
- Low, Medium, and High retain measured, shared meanings; Off performs no
  horizon or frontier allocation.
- Native, WebGPU, Android, mono, stereo, per-eye XR, and multiview consume the
  shared architecture.
- The settled implementation has focused module ownership and the living topic
  doc contains the complete durable system model.

## Non-Goals

- Reintroducing the retired chunk-based Far LOD or its lifecycle.
- Selecting a general quadtree or unrestricted adaptive terrain system before
  the bounded frontier evidence requires one.
- Hiding unsupported geometry through fog, exact render-distance inflation,
  alpha blending, dither, depth bias, or coincident opaque surfaces.
- Changing Mclone Overworld generation, canonical exact blocks, persistence,
  or gameplay simulation.
- Solving distant edits, arbitrary structures, caves, arches, overhangs, or a
  general sparse volumetric LOD.
- Making an app-local desktop, browser, Android, or XR frontier implementation.
- Treating a larger fixed level-zero square as complete merely because it
  repairs the first reproduced camera.
- Broad renderer cleanup unrelated to extracting the selected frontier owner.

## Code And Documentation Map

- `native/crates/mclone-terrain-view/src/clipmap.rs` -- regular level geometry,
  snapped bounds, toroidal tiles, and inner holes.
- `native/crates/mclone-terrain-view/src/horizon_admission.rs` -- per-level
  requested/staged/committed resources and seven-slot movement guard.
- `native/crates/mclone-terrain-view/src/composition.rs` -- connected exact
  admission helpers, fixed mask, transition field, and boundary profile.
- `native/crates/mclone-terrain-view/src/source.rs` -- immutable prepared exact
  generation.
- `native/crates/mclone-terrain-view/src/engine.rs` -- shared host-neutral
  composition entry point.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` -- current late
  composition, GPU pools, connector filtering, visibility, vegetation, and
  draw submission; target of focused extraction after selection.
- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl`
  -- fine/coarse holes, exact discard, appearance, and connector geometry.
- `native/crates/mclone-scene/src/terrain_view.rs` -- live authoritative ready
  columns, focus-connected exact set, boundary facts, and scene arbitration.
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  -- living canonical system description at completion.
- [`313-direct-exact-to-smooth-horizon-transition.md`](313-direct-exact-to-smooth-horizon-transition.md)
  -- accepted direct-frontier implementation history.
- [`320-cross-platform-lod-quality-presets.md`](320-cross-platform-lod-quality-presets.md)
  -- accepted preset budgets, defaults, and physical Quest evidence.

## Execution Record

Created 2026-08-20 after high exact render distance exposed a remaining sky
crack outside the fixed finest clipmap region. Creation changes documentation
only. Phase 0 begins after review of this tactical.
