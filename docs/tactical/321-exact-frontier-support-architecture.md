# Tactical 321: Exact Frontier Support Architecture

Status: Human Reviews A1, A2, and B accepted 2026-08-20. Phase 1 diagnostic
frontier planning is complete at revisions `328757c3` through `cd9706e3`.
Human Review A2 selected bounded Hybrid D. Phase 2's isolated topology proof
is complete at revisions `0a119f30` through `76dda6e9`. Human Review B
accepted its topology and pixels. Phase 3 ordinary-product admission and its
static/streaming evidence are complete; Human Review C is ready.

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

| Preset | Logical clipmap tiles | Fixed bytes | Decimal MB | Binary MiB |
|---|---:|---:|---:|---:|
| Low | 96 | 81,633,320 | 81.6 | 77.9 |
| Medium | 128 | 107,421,472 | 107.4 | 102.4 |
| High | 160 | 133,209,624 | 133.2 | 127.0 |

Each level also owns seven staging resources so movement can retain its prior
committed presentation. One fully provisioned terrain resource is 560,612
bytes, or 0.56 decimal MB / 0.535 MiB, before dynamic vegetation.
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
extent instead of 4-by-4. Even retaining the now-insufficient seven staging
slots would add 172.7 decimal MB / 164.7 MiB before vegetation. Preserving the
current atomic diagonal-movement contract needs up to 35 entering-tile slots
for that extent, raising the added fixed cost to at least 188.4 decimal MB /
179.6 MiB before vegetation.
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

## Phase 0 Architecture Audit

This section is the Human Review A1 checkpoint. It records the code as it
exists before a diagnostic frontier API or any pixel behavior changes. The
audit confirms the missing invariant in the problem statement and broadens it:
the current renderer cannot prove a complete ownership handoff because exact
admission, clipmap admission, connector construction, water composition, and
vegetation composition are related by convention rather than one prepared
composition decision.

### Current ownership and frame flow

| Fact or operation | Current producer | Current consumer / consequence |
|---|---|---|
| Render-session resident columns | `mclone-app-runtime` | `mclone-scene` filters them by exact distance and neighbor readiness. |
| Candidate exact set | `mclone-scene`, using `terrain_exact_player_connected_chunks` | The focus-containing four-connected component becomes the proposed exact draw and mask set. |
| Exact source and generation | `TerrainPreparedExactFrame` in `mclone-terrain-view` | Couples the exact mask, appearance field, and boundary profile, but not a procedural presentation. |
| Appearance transition | `composition.rs`, prepared when exact coverage changes | A 32-block field converges color and lighting inputs; it does not provide geometry support. |
| Exposed boundary heights | `mclone-scene`, by scanning live exact columns | Installed into the prepared exact frame after the initial coverage snapshot. |
| Requested clipmap | `TerrainClipmap` from camera focus and quality preset | Produces regular level tile requests and rectangular finer-level holes. |
| Committed clipmap | `TerrainHorizonAdmission` | Each changed level swaps only after all of its staged tiles are ready; levels can swap independently. |
| Clipmap GPU resources | `viewport_renderer.rs` | Sixteen logical and seven staging resources are provisioned per level. |
| Procedural discard | Procedural fragment shader | Discards every procedural surface class whose horizontal location is exact-painted. |
| Exact connector | `viewport_renderer.rs` | Re-derives instances late and only from committed tiles whose sample spacing is one. |
| Water ownership | Procedural terrain pass plus later exact translucent pass | Procedural water is discarded inside exact coverage; water boundary facts are skipped by the solid connector without a separate closure certificate. |
| Proxy vegetation | Terrain View vegetation admission and renderer | Uses exact coverage and committed procedural state, but has no frontier-support generation to follow. |
| Frame settlement | `TerrainViewportRenderer::target_ready` | Checks clipmap, GPU work, and vegetation settlement, but not exact-edge support. |

The actual draw order is also part of the contract. The frame clears sky,
draws exact opaque and cutout chunks into reversed-Z depth, draws the masked
procedural backdrop, then later draws actors and exact translucent terrain,
including exact water. A crack is therefore not simply a connector draw bug:
the sky becomes visible whenever the exact mask removes procedural geometry
and no certified geometry path closes the resulting height disagreement.

The current immutable exact generation is useful and should be retained. Its
limit is that its identity ends at exact-derived fields. It does not name the
committed clipmap tile set against which those fields were classified. A
clipmap level can consequently commit a new presentation while the same exact
generation and its renderer-derived connector remain active.

### Spatial and format capacity ledger

The base terrain grid contains 64 cells per tile. Level `L` has sample spacing
`2^L`, a tile footprint of `64 * 2^L` blocks, and four tiles per axis. Its full
rectangular extent is therefore `256 * 2^L` blocks. Quality presets change the
number of levels, not the four-by-four finest extent or its spacing.

For an ordinary square exact radius `r`, let `C` be the focus chunk coordinate
on one axis and `q = C mod 4` its phase within a spacing-one tile. The exact
interval is `[C-r, C+r+1)` chunks. The level-zero gaps beyond the exact
interval are:

```text
negative-side gap = (q - r + 8) * 16 blocks
positive-side gap = (7 - q - r) * 16 blocks
```

The asymmetric formulas follow from centering the four-tile clipmap on the
64-block tile containing the focus, rather than centering it on the exact
window. Across all four chunk phases on both axes:

- radius 4 is the largest exact square guaranteed to remain inside level zero;
- radius 3 is the largest guaranteed to retain at least one spacing-one block
  outside every exact edge for a direct connector; and
- radius 2 is the largest guaranteed to retain the complete 32-block
  appearance/support halo inside level zero.

More generally, level `L` is guaranteed to leave an immediately exterior
procedural column on every side only when `r <= 4 * 2^L - 1`. At equality with
`r = 4 * 2^L`, one phase can place the exact edge exactly on that level's outer
edge, making the next coarser level the direct neighbor.

| Exact radius | Square chunks | Width | Coarsest direct neighbor required by worst phase | Existing format |
|---:|---:|---:|---:|---|
| 2 | 25 | 80 blocks | spacing 1 | valid; full 32-block level-zero halo |
| 5 | 121 | 176 blocks | spacing 2 | valid |
| 8 | 289 | 272 blocks | spacing 4 | valid; the sampled origin met spacing 2 |
| 13 | 729 | 432 blocks | spacing 4 | valid |
| 31 | 3,969 | 1,008 blocks | spacing 8 | maximum complete odd-width square |
| 32 | 4,225 | 1,040 blocks | spacing 16 | invalid before frontier planning |

The `r = 8` receipt with zero connector segments is therefore expected, not
an isolated alignment accident. Other phase combinations can make its direct
neighbor even coarser than the measured center-zero scene.

The existing exact-derived GPU formats share a nominal 64-chunk maximum but
reach it through different representations:

| Representation | Fixed capacity | Radius-31 demand | Radius-32 demand |
|---|---:|---:|---:|
| Painted mask | 64 by 64 bits / 512 bytes | 63 by 63 chunks | 65 by 65; exceeds capacity |
| Boundary profile | 1,024 by 1,024 `f32` / 4,194,304 bytes | 1,008 by 1,008 blocks | 1,040 by 1,040; exceeds capacity |
| Transition field | 272 by 272 bytes at 4 blocks/texel | 268 by 268 texels with halo | 276 by 276 with halo; exceeds capacity |

Desktop currently advertises radius 32, while the XR settings boundary
advertises at most 16. The product contract must either change the exact
formats and prove their new bounds or change the shared legal maximum. A
silent clamp is not an acceptable resolution, and Phase 1 diagnostics must
report this case as format-invalid rather than misclassifying it as a normal
unsupported edge.

Fixed terrain residency consists of 23 resources per level: 16 logical tiles
plus seven staging slots. Each resource consumes 560,612 bytes: 540,800 for
the semantic samples, 19,044 for normal-height data, and 768 for two aligned
uniform allocations. The exact mask, maximum transition field, maximum
boundary profile, and exact uniform add 4,268,864 fixed bytes. This produces
the Low, Medium, and High totals in Starting Evidence and explains why a
complete enlarged finest square is not a neutral fix. The seven-slot staging
bound is itself derived from the maximum entering row plus column of a
four-by-four grid. An 18-by-18 grid needs up to 35 staging slots to preserve
the same atomic diagonal shift, so rectangular growth scales both logical and
staging residency.

The current CPU and upload work also has shape-sensitive worst cases:

- transition preparation is proportional to the exact bounding rectangle;
- boundary fact sampling is proportional to exposed block edges and may scan
  downward as far as 1,024 blocks per edge;
- boundary upload is a dense bounding-rectangle upload, reaching 4,064,256
  bytes for a radius-31 square whenever coverage generation changes; and
- a compact radius-31 square has 4,032 block-edge connector segments, but a
  loose 4,096-chunk adversarial bound is 262,144 segments, about 3 MiB of
  12-byte instances and 1.57 million submitted vertices.

Those are capacities, not a prediction that ordinary play reaches every
bound. They establish why Phase 1 must measure preparation and generation
churn as well as steady draw time.

The periodic-cylinder case exposes a second independent format assumption.
Connectivity uses topology-aware neighbors, but exact mask packing and dense
boundary bounds currently use canonical numeric minima and maxima. A small
locally connected region crossing the canonical wrap can therefore appear
nearly a full topology period wide. Frontier preparation must operate in one
observer-local lifted coordinate frame, retain the canonical identity needed
for sampling, and reject ambiguous lifts explicitly. Merely enlarging the
64-chunk formats would not repair this case.

### Late renderer assumptions to extract

| Current late assumption | Required prepared composition fact |
|---|---|
| Every connector-relevant edge intersects a spacing-one tile. | Bordering committed level and sample spacing for every exposed exact segment. |
| Calling a clipmap level committed is sufficient for exact composition. | A presentation identity covering all levels and support resources named by the plan. |
| An exact generation may commit independently of clipmap movement. | One atomic exact/procedural ownership epoch or an explicit safe retention transition. |
| One rectangular finer-level hole describes all fine/coarse ownership. | Explicit suppression and outer-closure topology for any sparse fine support. |
| All levels share one `tiles_per_axis`. | A separately bounded support pool if the selected design needs non-regular extent. |
| The appearance field implies a usable transition. | Separate appearance convergence from geometry closure and certify both. |
| Skipping water connector instances is harmless. | An explicit exact-water, procedural-water, coast, or unsupported boundary classification. |
| Vegetation can follow exact coverage alone. | Vegetation ownership keyed to the same committed composition epoch. |
| Visible connector count represents frontier completeness. | Visibility-independent totals for classified, closed, fallback, and unsupported segments. |
| Canonical min/max bounds are locally compact. | A topology-aware local coordinate lift shared by mask, boundary, support, and connector facts. |
| `target_ready` means the composed view is settled. | Certificate readiness and any retained/rejected generation included in settlement. |
| Boundary preparation is a small incidental cost. | CPU, dense upload, segment, and generation-churn receipts exposed separately. |

The regular clipmap's rectangular inner holes and its fine/coarse edge
geometry remain valid for complete nested rectangles. They do not define the
holes, bays, self-near boundaries, or outer edge of a sparse fine-support
belt. Candidate B or D therefore needs a new explicit ownership topology; it
cannot reuse the existing inner rectangle by changing tile admission alone.

### Renderer-neutral frontier plan and certificate

Phase 1 should introduce a diagnostic-only plan with the following semantic
contract. Names and storage layout remain implementation details until A1 is
accepted.

```text
TerrainCompositionEpoch {
    source identity,
    exact coverage generation,
    committed procedural presentation identity,
    topology and local coordinate lift,
}

TerrainFrontierPlan {
    epoch,
    exact coverage and compact exposed boundary segments,
    per-segment solid / water / unsupported-volume classification,
    per-segment bordering procedural level, spacing, and readiness,
    desired fine-support keys,
    proposed closure decision for every segment,
    coarse suppression and outer-closure facts,
    bounded CPU / memory / upload / draw receipt,
    state: complete | pending | capacity-rejected | format-invalid,
}

TerrainCompositionCertificate {
    epoch,
    immutable exact draw and discard snapshot,
    immutable committed procedural and support snapshot,
    exactly one accepted closure path for every exposed segment,
    vegetation and water ownership decisions,
    validated bounded-cost receipt,
}
```

The plan inputs are one prepared exact frame, one whole committed procedural
presentation, topology information, the selected frontier policy and caps,
and the prior complete certificate if it is still drawable. Requested or
partially staged tiles are evidence of future readiness, never valid closure
inputs. Boundary segments are the proof unit because an irregular connected
chunk set can meet different clipmap levels along one side and can contain
holes or concave bays.

A complete certificate must establish all of these invariants:

1. The exact draw, procedural discard, appearance field, boundary facts,
   water decision, vegetation decision, and support resources name one epoch.
2. Every exposed segment has exactly one accepted closure path and references
   committed resources; no segment is silently omitted because of spacing,
   visibility, material, or view.
3. Every horizontal location has one geometry owner. Any support layer also
   carries an explicit suppression region beneath it and an outer handoff to
   the regular clipmap.
4. All referenced resources fit the selected fixed capacities. Allocation or
   submission cannot expand those bounds after certification.
5. Mono, both ordinary XR eyes, and full-frame multiview consume the same
   immutable certificate. Per-view culling may remove invisible work but may
   not choose a different topology.

The plan is reusable while all epoch fields remain unchanged. Camera motion
inside the same exact generation and committed clipmap presentation does not
invalidate it. Exact coverage change, source reset, topology-lift change,
support-pool commit, or any referenced clipmap-level commit creates a new
epoch. Stale planning work is coalesced by desired epoch and may never publish
after a newer epoch becomes current.

### Admission and failure lifecycle

The composition owner needs an explicit two-input commit rather than relying
on draw order:

```text
candidate exact + committed procedural presentation
                         |
                         v
                 build frontier plan
             / complete       pending \
            v                           v
  atomically publish             prepare bounded support
  new certificate               and retain old certificate
                                           |
                                           v
                                  re-plan after commit
```

Retention is legal only while every exact column named by the prior
certificate is still drawable and every procedural resource it names remains
committed. The clipmap admission owner must likewise delay or atomically pair
a level swap which would invalidate that certificate. Independent level
atomicity is insufficient once an exact edge depends on a particular level.

Exact eviction is different from exact growth. If streaming removes a column
used by the prior certificate, retaining that exact generation would itself
create a hole. The safe transition is an atomically certified smaller exact
subset, or empty exact coverage with the already committed procedural owner,
before the unavailable exact draw is removed. Source reset invalidates both
old exact and old procedural identities and cannot use retention across the
reset.

`capacity-rejected`, `format-invalid`, and unsupported volumetric boundaries
are ordinary planned outcomes, not renderer warnings. The final policy may
select a resolution-aware closure, reduce exact admission to a certifiable
focus-connected subset, or retreat to procedural ownership. Which degradation
is product-correct remains deliberately open until the A2 measurements and
human selection. It may never publish an incomplete certificate, allocate
without a cap, or leave the previous mask active after its exact draw becomes
unavailable.

### A1 conclusions and open decisions

Phase 0 recommends accepting these architecture conclusions before adding
instrumentation:

- the defect is a missing cross-system composition invariant, not merely an
  undersized finest ring or a spacing-one shader filter;
- exact-edge boundary segments, classified against one committed procedural
  presentation, are the correct proof unit;
- `mclone-terrain-view` should own the plan and certificate, while
  `mclone-scene` supplies authoritative exact readiness and orchestrates
  certificate admission;
- the renderer should consume certified immutable facts and retain only
  visibility, resource upload, culling, and submission mechanics;
- legal radius 32 and periodic topology lifting are existing composition
  contract defects which must be resolved along with frontier support; and
- candidate A, B, C, or D and the exhaustion fallback remain open until Phase
  1 produces objective distribution, cost, and pixel evidence.

Human Review A1 may adjust that division of ownership, the certificate proof
unit, format-scope requirements, or the retention model. Accepting A1
authorizes diagnostic planning only; it does not select the eventual geometry
candidate.

## Phase 1 Diagnostic Results

Phase 1 adds a renderer-neutral `TerrainFrontierPlan` in
`mclone-terrain-view`. It consumes one exact coverage and boundary generation,
one complete committed clipmap presentation, the horizontal topology, and an
observer-local coordinate lift. Its directed block-edge segments retain
canonical exact coordinates, lifted coordinates, edge direction, solid/water/
missing-profile classification, bordering procedural level and spacing,
procedural owner count, and current closure. The plan also derives a stable
presentation identity, format-capacity receipt, hypothetical fine-support
keys, and costs for Candidates A, B, and C.

The live renderer caches this diagnostic plan by exact generation and
committed presentation. It invalidates on an exact, boundary, topology, or
relevant periodic-observer change. Planning failure produces an explicit
`invalid` receipt and increments a failure counter; it does not fail or gate
ordinary rendering. `target_ready`, clipmap admission, exact admission,
procedural discard, connectors, water, vegetation, and draw geometry are
unchanged in this phase.

The new `frontier-support` presentation leaves exact terrain natural, renders
unrelated procedural terrain near-black, marks spacing-one procedural
frontier terrain green, and marks spacing-two-or-coarser frontier terrain
magenta. Both ordinary and full-frame multiview shaders validate with the new
selector. The capture lane requires natural and diagnostic variants to have
identical settled semantic state after normalizing process-local generation
and timing values.

### Objective matrix

The focused terrain-view suite now contains 133 tests, of which 132 pass and
one native-GPU preview remains intentionally ignored without an adapter. The
frontier subset proves:

- all 16 two-axis chunk phases at exact radius 2 and a 16-phase sweep for
  radii 2, 5, 8, and 13;
- radius-31 candidate bounds and explicit radius-32 mask, boundary, and
  transition rejection before dense snapshot packing;
- L, staircase, ring-with-hole, corridor/comb, growth, eviction, disconnected
  island rejection, missing profile, and water classifications;
- negative coordinates, plane ownership, and compact observer-local lifting
  across a cylinder seam;
- stable presentation identity for sub-tile motion and a changed identity at
  a 64-block clipmap rebase;
- distinct exact generations and source-reset identity; and
- deterministic sparse-pool exhaustion, including a legal 63-by-63-chunk
  comb which requests 308 additional fine tiles, admits the 128-tile
  diagnostic cap, and rejects 180.

Existing clipmap and composition tests continue to cover chunk motion,
teleport, delayed readiness, exact connected admission, atomic growth and
eviction, toroidal slot reuse, and synthetic multiview ownership. Because the
plan has no view input, mono, per-eye, and multiview submission consume one
immutable diagnostic classification rather than preparing separate topology.

### Matched capture evidence

The final native campaign was generated at revision `e21e93d9` with seed
`12345`, focus chunk `(0, 0)`, frozen noon, the Original pack, vanilla color
profile, High distant terrain, and 1,024-by-576 output. All four captures
settled on the first attempt. The complete machine-readable
[receipt](/tmp/mclone-t321-phase1/receipt.json) and its natural and diagnostic
PNGs are in the [review directory](/tmp/mclone-t321-phase1) on the review
host.

| Exact radius and view | Exposed edges | Bordering spacing | Existing closure | Boundary prep | Frontier prep |
|---|---:|---:|---:|---:|---:|
| RD2 low | 320 | 320 at spacing 1 | 277 solid connectors; 43 water edges uncertified | 424–475 us | 51–52 us |
| RD8 elevated | 1,088 | 1,088 at spacing 2 | 0 connectors; 896 solid and 192 water edges unclosed | 1,371–1,460 us | 244–264 us |

Boundary preparation is now measured independently from the existing
transition-field preparation. The same captures measured 30 us for the RD2
transition and 180–196 us for RD8, versus the larger live-column scan and
dense boundary packing costs above. RD2 uploaded a 25,600-byte boundary
rectangle containing 316 valid columns; RD8 uploaded 295,936 bytes containing
1,084 valid columns. The four duplicated directed corner edges explain why
frontier edge counts exceed unique boundary columns by four.

Inspected diagnostic pixels agree with the receipts. RD2 shows a green land
frontier, while RD8 shows a continuous magenta land frontier. The corresponding
natural captures preserve the existing product pixels and expose the same
high-distance risk without any diagnostic geometry affecting them. Water is
not silently counted as closed in either case; its separate unresolved count
is an important input to the selected topology proof.

### Candidate cost ledger

Candidate costs describe requirements, not allocations made by Phase 1:

| Fixture | A: complete finest extent | B: sparse 32-block belt | C: resolution-aware solid connector |
|---|---:|---:|---:|
| RD2 capture | no added resources | no added tiles | 277 segments / 3,324 bytes; 43 water edges unresolved |
| RD8 capture | 6x6 logical extent plus 11 staging; 13.45 MB added | 32 desired, 12 resident, 20 added; 11.21 MB required | 896 segments / 10,752 bytes / 5,376 vertices; 192 water edges unresolved |
| Compact RD31 square | 18x18 logical extent plus 35 staging; 188.37 MB added | 128 desired and added; 71.76 MB required, exactly the diagnostic cap | 4,032 segments / 48,384 bytes / 24,192 vertices |
| Legal 63x63 comb | same 18x18 extent and 188.37 MB added | 324 desired, 16 resident, 308 additional; 180 rejected by the cap | 63,552 segments / 762,624 bytes / 381,312 vertices |

The data rules out Candidate A as the shared primary solution: its simple
topology purchases correctness with an unacceptable worst-case fixed-resource
increase. Candidate B preserves the desired spacing-one character and is
reasonable for RD8, but a compact legal RD31 square consumes all 128
hypothetical support slots and a legal comb proves that perimeter support can
degenerate toward area cost. Candidate C is dramatically cheaper and closes
arbitrary solid edges, but deliberately places coarse topology beside exact
blocks and does not yet solve water.

Phase 1 therefore recommends Candidate D: a bounded hybrid. Prefer a sparse
spacing-one belt for the ordinary exact frontier, give it explicit base
suppression and an aligned outer stitch, and use a separately accepted
resolution-aware closure when the fine pool is exhausted. The Phase 2 proof
must include a typed water/coast closure and must retain or reduce exact
admission if neither preferred nor fallback closure can certify every edge.
The support-pool size, fallback pixels, and water topology remain Human Review
A2 decisions; the measurements do not authorize choosing them implicitly.

Human Review A2 accepted bounded Hybrid D on 2026-08-20. Phase 2 is authorized
to prove a sparse spacing-one support belt with explicit base suppression and
outer closure, a resolution-aware exhaustion fallback, and a typed water/coast
closure. That proof remains opt-in and may not change ordinary composition
before Human Review B accepts its topology and pixels.

## Phase 2 Topology Proof Results

The shared renderer now has an explicitly opt-in `frontier-hybrid-proof`
presentation. It consumes the renderer-neutral topology proof and lazily
allocates only its selected spacing-one support tiles. Support compilation is
limited to four dispatches per frame. The coarse suppression table remains
empty until every selected tile is ready; the complete set and its matching
suppression set then publish together. Leaving the diagnostic immediately
drops the support resources and proof connector buffers. Natural composition
never allocates a support tile or proof connector and retains its existing
geometry and draw path.

Every exact segment is encoded into one buffer owned by either its active
spacing-one tile or the one committed coarse tile selected by the fallback.
Solid and water are separate flags. Preferred closures use the fine triangle;
fallback closures evaluate the actual bordering coarse triangle rather than
pretending that a spacing-one endpoint exists. Every exposed outer edge of a
selected support tile receives one block-segmented vertical skirt owned by
that tile. Base horizontal fragments beneath the complete support set are
discarded, so exact, support, and base retain one horizontal owner.

The proof is deliberately not a live admission implementation. It rebuilds
from one settled committed clipmap presentation, has no retention or
coalescing lifecycle, does not yet coordinate support vegetation, and is not
available in ordinary product composition. A fixed 512-byte suppression-key
buffer is present in the exact bind group on all paths; natural frames report
zero dynamic proof resources. Phase 3 remains responsible for generation-
coherent exact/support/base commits if Human Review B accepts these pixels.

### Objective and pixel evidence

The terrain-view suite now has 140 tests: 139 pass and one native-adapter test
remains intentionally ignored. The topology and renderer tests cover solid,
water, missing profile, finite world boundary, periodic lift, negative L/ring/
comb shapes, pool exhaustion, one-owner connector assignment, outer closure,
shader parsing, and mono/multiview shader construction. Shared terrain-view
and scene owners also check for `wasm32-unknown-unknown`; the native client
checks on its native target.

The matched seed-12345 review campaign is in
[`/tmp/mclone-t321-phase2`](/tmp/mclone-t321-phase2) with its machine-readable
[`receipt.json`](/tmp/mclone-t321-phase2/receipt.json). All captures use the
same frozen noon, Original pack, vanilla color profile, High distant-terrain
preset, focus chunk `(0, 0)`, and 1,024-by-576 camera. Natural and proof
variants retain identical settled clipmap, exact, and vegetation state after
normalizing proof work and process-local generations.

| Case | Support | Exact closure | Outer closure | Added resident bytes | Added submitted vertices |
|---|---:|---:|---:|---:|---:|
| RD2 natural to proof | 0 tiles | 277 solid + 43 water | none | 3,840 | 258 |
| RD8 natural to 32-tile proof | 20 tiles, 8 visible | 896 solid + 192 water | 24 edges / 1,536 segments | 11,243,728 | 202,848 |
| RD8 natural to forced one-tile fallback | 1 tile, offscreen | 64 preferred solid + 832 fallback solid + 192 fallback water | 3 edges / 192 segments | 570,020 | 3,168 |

The normal RD8 proof compiles each of its 20 tiles once, consuming 11,212,240
terrain-resource bytes. Its exact and outer connector buffers consume 31,488
bytes. The forced-fallback selector exists only to expose exhaustion pixels
at the same review site; it changes the proof cap to one without changing the
accepted 32-tile design. Its inspected image uses 1,024 resolution-aware
fallback segments and retains a coherent silhouette and water handoff without
a sky crack or conspicuous wall. RD2 proves that the typed water closure can
replace the solid-only connector without allocating sparse terrain.

These measurements are submission and residency costs, not a claim of final
GPU frame time. Product performance and streaming churn cannot be measured
honestly until Phase 3 owns a persistent admission lifecycle. They are enough
for Human Review B to judge the topology, fallback character, and bounded
steady draw expansion before authorizing that integration.

## Phase 3 Live Admission Results

The accepted hybrid is now the ordinary composition path, including
`Natural`. Each non-empty exact generation and each newly committed clipmap
presentation first receives a complete zero-tile, resolution-aware fallback
certificate in the same render submission. Terrain discard, typed land/water
closures, the regular vegetation owner, mono/per-eye/multiview consumption,
and the immutable procedural presentation therefore agree before pixels are
submitted. The preferred 32-tile spacing-one generation compiles behind that
certificate at four dispatches per frame. Only after every selected tile is
ready do its suppression set, support draws, and connector ownership publish
together. A newer exact or clipmap epoch drops stale pending work while the
new synchronous fallback remains complete.

The support layer deliberately owns no second proxy-vegetation product. The
regular clipmap vegetation presentation remains the one procedural tree
owner across both base and support terrain, while exact-owned complete tree
records continue to be removed by the same exact coverage generation. This
avoids duplicate or disappearing trees during a fine-support promotion.

Desktop's legal radius 32 now fits a shared 65-by-65 exact format. The mask
uses 133 words with a 65-chunk row stride; the transition and boundary
formats derive their 276-texel and 1,040-block maxima from the same bound.
The explicit exhaustion policy is the tested zero-fine-capacity certificate:
every solid and water segment uses its actual bordering coarse triangle,
without allocation, suppression, an unsupported segment, or a sky crack.

The matched product review campaign is in
[`/tmp/mclone-t321-phase3`](/tmp/mclone-t321-phase3) with its machine-readable
[`receipt.json`](/tmp/mclone-t321-phase3/receipt.json). The natural RD2 view
certifies 320 segments using the resident finest ring and allocates no support
tile. The natural RD8 view commits 20 support tiles (11.21 MB), certifies all
1,088 land/water segments, and draws eight visible support tiles. The forced
one-tile RD8 diagnostic certifies the same 1,088 segments with 1,024 fallback
segments. All three settled with no pending vegetation or frontier work and
were inspected without a sky crack.

A command-driven headed traversal report at
[`/tmp/mclone-t321-phase3-motion.json`](/tmp/mclone-t321-phase3-motion.json)
moved the camera 130.68 blocks over 240 presented frames. It crossed three
committed clipmap presentations and 138 exact generations under active
streaming. Every frame carried a complete certificate: 188 frames retained a
synchronous fallback while preferred work coalesced and 52 used a preferred
certificate. There were no skipped or reconfigured surface frames. This is
objective ownership evidence; Human Review C remains the acceptance point for
the inspected static pixels and motion character.

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

- [x] Record the current clipmap, admission, exact snapshot, transition,
      connector, water, vegetation, and draw data flow with actual owners.
- [x] Reconcile every legal render distance with the 64-chunk exact format and
      finest-ring capacity.
- [x] Identify every late renderer assumption which should become a prepared
      composition fact.
- [x] Specify the frontier plan/certificate inputs, outputs, lifetime, and
      failure behavior without changing pixels.

Gate -- Human Review A1: accept or adjust the current-system model and the
missing-invariant diagnosis before instrumentation changes its public shape.

### Phase 1: Diagnostic frontier plan

- [x] Implement renderer-neutral boundary classification and capacity
      receipts without changing ordinary geometry or ownership.
- [x] Expose unsupported edges, bordering levels, hypothetical candidate
      support sets, and candidate memory/draw estimates.
- [x] Add the adversarial shape, phase, render-distance, motion, and topology
      objective matrix.
- [x] Capture matched natural and diagnostic pixels at low and high exact
      render distances and verify the receipts against inspected seams.
- [x] Measure boundary-profile preparation separately from the existing
      transition-field timing.

Gate -- Human Review A2: review the measured frontier distribution, common
case, worst case, memory/performance projections, and diagnostic pixels.
Select candidate A, B, C, D, or explicitly request further research. No
candidate implementation begins without this decision.

### Phase 2: Selected topology proof

- [x] Implement the smallest isolated proof of the selected support and outer
      closure topology in shared ownership.
- [x] Keep the ordinary product path unchanged until exact, support, and base
      ownership assertions pass.
- [x] Prove solid, water, hole, concave, negative-coordinate, and level-edge
      cases with one complete owner and no unsupported segment.
- [x] Measure pool bytes, generated tiles, vertices, dispatches, uploads, and
      steady draw cost against the committed baseline.

Gate -- Human Review B: inspect matched exact/frontier pixels and accept the
topology before it becomes the ordinary composition path.

### Phase 3: Generation-coherent live admission

- [x] Integrate the selected support plan with requested, staged, and
      committed procedural presentations.
- [x] Gate exact generation changes on a complete composition certificate;
      coalesce stale support work and retain the prior complete owner.
- [x] Apply the same immutable plan to terrain discard, connector geometry,
      water, vegetation, mono, per-eye, and multiview rendering.
- [x] Give support exhaustion and legal-capacity mismatch one explicit tested
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
only.

Phase 0 completed 2026-08-20 without code or pixel changes. The audit traced
the exact, procedural, connector, water, vegetation, and draw owners; derived
the phase-dependent spacing and legal-render-distance capacity ledger; listed
the renderer assumptions which need prepared ownership; and specified the
inputs, invariants, lifecycle, and failure states of a renderer-neutral
frontier plan and complete composition certificate. Human Review A1 is
accepted. Phase 1 may change diagnostic APIs and presentations but not ordinary
terrain geometry or ownership.

Phase 1 completed 2026-08-20 at revisions `328757c3` through `cd9706e3`.
The shared planner, live cached receipts, separate boundary timing,
adversarial objective matrix, and matched natural/diagnostic capture lane pass.
Inspected pixels and receipts prove the RD2 spacing-one case and the RD8
spacing-two failure without changing ordinary geometry. Candidate cost
evidence recommends bounded Hybrid D, but implementation is stopped at Human
Review A2 as required.

Human Review A2 accepted bounded Hybrid D on 2026-08-20 and authorized the
isolated Phase 2 topology proof. Ordinary geometry remains frozen until Human
Review B.

Phase 2 completed 2026-08-20 at revisions `0a119f30` through `76dda6e9`.
The proof selects at most 32 sparse fine tiles, commits suppression only after
all selected resources are ready, assigns one typed exact curtain to a fine
or coarse owner, and closes every support outer edge with a skirt. Matched
RD2 and RD8 natural/proof pixels plus a forced one-tile RD8 exhaustion view
pass their receipt contract and visual inspection. Ordinary composition still
uses the pre-proof geometry path and reports zero dynamic proof resources.
Human Review B accepted the bounded hybrid topology and its preferred and
forced-fallback pixels on 2026-08-20. Phase 3 is authorized to replace the
diagnostic-only lifecycle with generation-coherent ordinary-product
admission.

Phase 3 completed 2026-08-20 at revisions `0a0f9101` through `b515b5ad`.
All legal exact formats now cover radius 32. Ordinary composition installs a
complete synchronous coarse certificate for each exact/procedural epoch,
coalesces preferred work behind it, and atomically promotes a ready fine
support generation. Shared and per-frame receipts expose that lifecycle.
Matched RD2/RD8 product captures, a forced-exhaustion capture, and a
240-frame continuous traversal pass. Human Review C evidence is ready; the
user's request to proceed end to end authorizes continuing Phase 4 without an
additional implementation pause but does not pre-record visual acceptance.
