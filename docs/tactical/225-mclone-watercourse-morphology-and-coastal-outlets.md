# Mclone Watercourse Morphology And Coastal Outlets

Status: active 2026-07-23; design and interactive-review findings recorded,
implementation pending.

Topic: `mclone-overworld-generation`

## Objective

Correct the first interactive-review defects shared by Mclone's sea-level
major rivers, bounded valley streams, their receiving coasts, and the
surrounding surface language:

- a major-river bed may be deeper than the first ocean shelf, because river
  carving stops exactly at the land/ocean boundary while bathymetry begins at
  only two blocks of depth;
- the resulting underwater sill makes the river-to-ocean transition appear
  to change water color. The water tint is continuous; skylight attenuation
  and ambient shading expose the discontinuous floor;
- the major-river centerline, width, depth, and bank cross-section change too
  slowly and coherently, producing long parallel terraces and mirrored banks;
- the accepted bounded stream has an intentionally simple constant ordinary
  half-width and two-block bed depth, so repeated reaches can also look
  mechanically cut;
- steep terrain switches directly from grass soil to exposed stone at one
  slope/exposure threshold, drawing a stark contour across valley walls; and
- channels and banks lack the sparse rocks, substrate patches, point bars,
  pools, riffles, ledges, and other local modifications that make a simple
  heightfield read as a place rather than a formula.

The desired result keeps the accepted calm, peaceful stream family and
hydraulic fixed-point proof. It adds coherent morphology and surface detail;
it does not reopen arbitrary sloped source water or claim a global drainage
network.

## Originating Interactive Review

The 2026-07-23 review of seed `8675309` found:

1. At a coastal steppe outlet, the sea initially becomes shallower than the
   incoming river. The current major-river depth is normally three to four
   blocks, while bathymetry starts at two. The deep, darker river floor meets
   a raised, brighter shelf.
2. Rivers wind pleasantly at broad scale but look perfectly smooth-carved.
   Water width and depth remain too uniform; both banks repeat the same
   parallel offset; beds lack deeper pools, shallow riffles, obstructions, and
   local shelves.
3. The grass/dirt-to-stone transition near water and on valley walls is too
   abrupt. A binary surface recipe makes a single threshold legible as a long
   line.
4. Boulders, gravel/clay/sand patches, talus, and hydrology-aware local
   modifications are conspicuously absent.

The first water-color diagnosis incorrectly attributed the boundary to far
LOD. The follow-up side view proved that the boundary is physical floor
geometry made visible by lighting. This tactical records the corrected
diagnosis.

## Reference Vocabulary

Minecraft Java 1.17.1 remains useful for pipeline nouns and proven bounded
feature shapes:

- a surface builder selects the top/filler language after terrain;
- `DISK_SAND`, `DISK_CLAY`, and `DISK_GRAVEL` add coherent soft-material
  patches rather than independent per-block speckle;
- `BlockBlobFeature`, used by `FOREST_ROCK`, builds a small irregular rock
  from a bounded sequence of overlapping ellipsoids; and
- local modifications and underground ores run as separate feature steps
  rather than being hidden inside macro terrain.

Vanilla's river biome layer is not a morphology implementation to port.
Vanilla also uses simple surface thresholds, but its density terrain and local
features make those thresholds less dominant. Mclone should reuse the
surface/disk/blob vocabulary through its existing feature executor while
owning original placement, morphology, palettes, and seed domains.

Primary source:

- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/BaseDiskFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/BlockBlobFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/surfacebuilders/MountainSurfaceBuilder.java`

## Hydraulic And Performance Invariants

- Ordinary ocean and major-river water remains one flat Y63 source plane.
- Every bounded stream reach remains one constant integer source plane.
- Width, bed, bank, and substrate may vary; an ordinary water surface may not
  acquire a pointwise slope.
- Immediate dry containment beside a source plane may never be roughened
  below that plane. Deeper beds and submerged obstacles are safe candidates;
  exposed islands and split flow require the same fixed-point proof as other
  water stencils.
- Production does not run fluid simulation, connected-component search,
  erosion, or unbounded upstream/downstream tracing.
- Every new point field has fixed work, absolute-coordinate determinism, an
  independent seed domain, and exact plane/cylinder periodic behavior.
- The bounded stream planner may precompute a fixed number of morphology
  knots in its immutable plan. Per-column realization must remain bounded by
  the existing clipped structure references and cache.
- Authoritative chunks and synthetic far LOD consume the same surface and
  stream plans.
- Sparse rocks and material patches use the existing feature dependency
  footprint unless a measured spill radius requires a documented plan
  change.
- The internal, unshipped `mclone-overworld-v1` profile may change in place
  under the compatibility safety ledger. Update intentional fingerprints and
  disposable internal worlds; do not modify reference-locked `overworld`.

## Morphology Vocabulary

One profile-owned sample should express coherent watercourse morphology
without forcing major rivers and bounded streams to share route ownership:

```text
channel axis
  -> axis displacement / bend curvature
  -> bankfull half-width
  -> low-flow half-width
  -> thalweg lateral offset
  -> bed depth
  -> pool / riffle phase
  -> left/right bank character
  -> point-bar / cut-bank influence
  -> substrate family
  -> local-feature eligibility
```

Useful concrete facts include:

- `width_scale`: correlated widening and narrowing, not per-column jitter;
- `bed_depth_offset`: bounded deeper pools and shallower riffles;
- `thalweg_offset`: the deepest path may move toward the outside of a bend;
- `left_bank_roughness` and `right_bank_roughness`: independently correlated
  shoulder response;
- `pool_influence` and `riffle_influence`: alternating meso-scale reaches;
- `point_bar_influence` and `cut_bank_influence`: curvature-aware asymmetry;
- `edge_roughness`: small coherent shoreline displacement; and
- `substrate_patch`: gravel, sand, clay, coarse dirt, stone, or ordinary soil.

Variation is hierarchical:

| Scale | Role |
|---|---|
| 64-256 blocks | accepted route, broad meander, valley relationship |
| 16-64 blocks | width, pools/riffles, thalweg migration, asymmetric banks |
| 3-12 blocks | edge roughness, ledges, substrate patches, sparse rocks |

White noise at each block is explicitly rejected. It would create camouflage,
one-block bays, incoherent banks, and a harder closure problem without making
the river more natural.

## Slice 1: Coastal Receiving Profile

- Continue the major-river thalweg beneath the first ocean shelf as a
  submerged outlet/estuary carve.
- At each ocean channel column, select the deeper of ordinary bathymetry and
  the fading outlet floor. Never replace a deeper natural shelf or basin with
  a shallower channel.
- Preserve ocean biome and ocean-floor surface language for the submerged
  outlet; a geometric receiving channel is not an infinite river biome.
- Widen or fan the outlet modestly while fading its incision as ordinary
  bathymetry becomes deeper.
- Tighten the generic inner-shelf progression after a narrow shallow coastal
  margin. Preserve occasional tidal shallows as classified variation rather
  than making every coast a broad two-block-deep plate.
- Add a pinned seed-`8675309` coast-to-river transect. The centerline floor
  may not rise from the final river column into its receiving ocean before
  ordinary bathymetry is at least as deep.

The outlet fade may use ocean-interior/continentalness as a fixed-work
distance proxy. It must not trace the contour to discover a coastline.

## Slice 2: Major-River Morphology

- Retain the accepted 768/192-block broad path family.
- Add smaller correlated morphology fields instead of simply increasing
  high-frequency centerline weight.
- Vary ordinary half-width by roughly 20-30 percent over meso-scale runs.
- Alternate pools and riffles. Pools may be one or two blocks deeper and
  somewhat wider; riffles may be one block shallower and narrower while
  retaining submerged support.
- Use local centerline curvature to deepen/steepen the outside bend and place
  a shallower point bar on the inside bend.
- Add at most a small, smooth axis displacement so the shoreline does not
  remain a perfect offset of the broad zero contour.
- Break the repeated bank terrace with independent left/right shoulder
  response and bounded one- to two-block residual relief.
- Clamp the first dry ring beside Y63 source water to Y63 or higher, and keep
  the existing closure audit authoritative.

The major river still does not gain true source/sink identity. Curvature and
local provisional flow orient a cross-section; they do not claim downstream
network semantics.

## Slice 3: Bounded-Stream Morphology

- Preserve structure placement, route search, monotonic reach levels, typed
  pieces, and the accepted 48-96-block peaceful family.
- Derive deterministic morphology from structure identity and route station.
- Replace the constant ordinary 2.25-block half-width with a bounded,
  interpolated set of width knots.
- Replace the constant two-block calm bed with classified shallow, ordinary,
  and pool depths.
- Keep transition throats, falling columns, lips, and receiving pools exact;
  ordinary morphology may not erode a validated drop stencil.
- Allow gentle thalweg and bank asymmetry while retaining the existing
  no-required-fill route acceptance.
- Expose the same facts to authoritative chunks, review maps, and synthetic
  LOD.

## Slice 4: Surface Transitions

Replace the binary grass-soil/exposed-stone edge with a confidence band and
coherent patch selection:

```text
grass soil
  -> eroded grass / coarse dirt
  -> gravel / talus / mixed stone
  -> exposed stone
```

- Strongly steep/exposed columns remain reliably stone.
- Clearly sheltered columns remain reliably grass.
- The transition band uses low-frequency absolute-coordinate patch noise so
  stone fingers descend and soil pockets survive on ledges.
- Outside bends and cliff toes may bias toward stone/gravel; inside bars and
  low coastal banks may bias toward gravel/sand.
- Avoid isolated checkerboard pixels and long single-material contour lines.
- Vegetation continues to consume the final substrate so trees and flowers do
  not grow through exposed talus or riverbed rocks.

This is profile-owned surface policy. Do not generalize a cross-profile
surface DSL without a second concrete caller.

## Slice 5: Sparse Local Modifications

- Add a Mclone-owned small rock/blob feature using the shared placed-feature
  executor and vanilla `BlockBlobFeature` as shape vocabulary.
- Prefer one to three overlapping, partially buried lobes with a one- to
  four-block footprint.
- Select ordinary stone, cobble/mossy cobble where available, or a restrained
  mixed palette based on climate and moisture.
- Place rocks sparsely at cliff toes, cut banks, gravel bars, and selected
  submerged beds. Keep the accepted peaceful stream readable rather than
  filling every reach with obstacles.
- Add coherent gravel, sand, and clay patches using disk-like bounded
  replacement with hydrology-aware target predicates.
- Keep fallen logs, roots, rich aquatic plants, mist, sound, and functional
  rapids as later content unless the first pixel pass demonstrates that one
  is required to make the morphology legible.

Small rocks are local modifications, not structure starts. Larger tors,
arches, sea stacks, and overhangs remain owned by the separate volumetric
geology campaign.

## Validation

### Determinism and topology

- focused field and morphology unit tests;
- intentional Mclone field/decoration fingerprints;
- adjacent-chunk equality;
- exact 6,144-block plane/cylinder field seam;
- bounded-stream request-order, partition, and cache independence;
- synthetic far-LOD agreement at major-river, stream, and outlet columns.

### Hydraulic safety

- existing multi-seed major-river closure audits;
- complete planned-stream route closure;
- seed-`8675309` river-to-ocean outlet closure and non-rising-floor transect;
- authoritative wake of every water block at representative ordinary river,
  outlet, stream, pool/riffle, and boulder-adjacent sites;
- zero initial scheduled-fluid work outside intentional baked fall states.

### Visual review

Capture fully warmed render-distance-16 cards and full-size views from above
terrain:

- seed `8675309` coastal-steppe river-to-ocean outlet;
- one mountain-valley major river showing multiple width/depth changes;
- the accepted seed `-98765` bounded stream, proving the peaceful family was
  enriched rather than replaced;
- an outside bend with cut bank and inside point bar;
- a grass/coarse-dirt/gravel/stone transition;
- one sparse rock/substrate-feature site; and
- a negative/control site without excessive visual bustle.

Inspect the first drawable outlet, morphology, surface, and rock milestones
before proceeding to the next layer.

### Performance

- compare field-only and surface throughput before/after;
- alternate release cold and warm generation at one ordinary control, one
  major-river/outlet site, and the accepted planned-stream hotspot;
- investigate a greater than 25-percent cold-generation regression and block
  a twofold regression without an explicit product decision;
- run the existing 3,600-frame RD10 movement soak through mixed river/stream
  terrain;
- record generation, render, publication, and all fluid queue maxima/final
  states.

## Commit Plan

1. Record this tactical and update the durable topic.
2. Land coastal outlet and inner-shelf continuity with objective transects.
3. Land shared morphology vocabulary and major-river variation.
4. Land bounded-stream variation without changing structure ownership.
5. Land mixed surface transitions.
6. Land sparse local rocks/material patches.
7. Record complete deterministic, hydraulic, performance, and pixel evidence.

Use `Topic: mclone-overworld-generation` on every implementation-series
commit.

## Stop Conditions

Pause for human direction if:

- outlet continuity requires a global coast walk or unbounded drainage graph;
- coherent variation cannot preserve source-water closure without runtime
  settlement;
- a plausible surface transition requires a materially different block
  palette or texture direction;
- rocks present a choice between a sparse peaceful language and a dense
  dramatic language after both have inspected pixels;
- the accepted planned stream can only become varied by weakening its
  monotonic reach or no-fill proof; or
- cold generation crosses the twofold performance blocker.

Otherwise continue through the complete visual review point.
