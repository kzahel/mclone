# Continental Hydrography

Topic: `continental-hydrography`

Status: **selected product direction as of 2026-08-21. Human Review C of the
detached continental exact/LOD candidate retained the continental scale but
rejected its water and landform realization as final. Tactical
[`326`](../tactical/326-continental-catchment-and-landform-realization.md)
owns the first bounded mountain-to-lake catchment proof. Its directed graph,
shared terrain/water realization, exact lowering, snow/rock summits, ecology
semantics, conservative narrow-water LOD summaries, confluence aprons, and
five deterministic review sites are implemented. Exact agreement now covers
all five sites. Phase 4 owns movement, performance, browser evidence, and the
Human Review D package. Production `mclone-overworld-v1` remains unchanged
until that proof passes Human Review D.**

## Motivation

The detached continental candidate proves that one reconstructible source can
feed exact chunks and every procedural-horizon level, but it does not yet
describe a convincing watershed. Its lake is a thresholded depression with a
nearly uniform rim, its river is inferred from an ecological corridor field,
and its uplands are broad scalar bumps. The result reads as macro allocation
plus noise: water does not explain the valleys, valleys do not organize the
mountains, and riparian ecology is not downstream of actual drainage.

Mclone already contains useful water mechanisms and failure evidence:

- Tactical 220 proved that pointwise sloping water and locally selected
  reach levels can become hydraulically invalid, while a universal sea-level
  river language erases highland water stories.
- Tactical 222 accepted one bounded, monotonic source-to-river stream with
  explicit drops and static closure.
- Tactical 225 established reusable reach morphology: meanders, bankfull and
  low-flow widths, thalwegs, pools, riffles, asymmetric banks, cut banks,
  point bars, substrate, and receiving outlets.
- Tactical 234 established hydrology as a typed content stage and showed how
  narrow channels can publish scale-aware signed summaries to LOD.
- Tacticals 259 and 260 showed that shore families need locally feathered
  transitions and inherited terrain structure rather than broad material
  masks or coast-only noise.
- Tacticals 265, 267, and 270 showed why a hybrid of regional envelopes,
  compact typed skeletons, and analytic realization is promising, while a
  finite raster watershed is not an honest arbitrary-query source for an
  unbounded world.
- The clean-room Streams Reflowing research identified useful semantic facts:
  separate water occupancy from downstream current, make lakes own explicit
  rims and spills, and let synthetic discharge drive morphology without
  requiring arbitrary upstream traversal during a column query.

This topic consolidates those results for the continental direction. It does
not revive any rejected implementation.

## Product Decision

Hydrography is an upstream authored geography. Terrain and hydrography are
planned together; final terrain realizes both; ecology consumes their result.

```text
continental and province envelopes
       -> range fronts, divides, saddles, and protected basins
       -> bounded directed catchment graph
            headwaters -> tributaries -> confluences -> trunk reach
                                                -> lake -> spill/outlet
       -> analytic valley, channel, bank, floodplain, and shoreline profiles
       -> exact blocks and direct procedural-horizon samples
       -> riparian, wetland, crossing, refuge, and migration facts
```

An ecological route may follow, widen, or cross a watercourse. It may not
create the watercourse, choose its direction, or move its sink. Wetland and
riparian suitability derive from reach, floodplain, basin-margin, and water
permanence facts.

## Bounded Generative Catchments

The ordinary product remains an unbounded plane, so Mclone will not claim an
exact global watershed derived from a complete elevation raster. The selected
first mechanism is **generative hydrography**: stable coarser owners construct
bounded feature-owned graphs with explicit maximum influence.

One catchment plan publishes compact facts such as:

- stable catchment, reach, confluence, basin, spill, and outlet identities;
- an explicit directed acyclic reach graph;
- ridge/divide and saddle anchors shared with landform realization;
- source and sink ownership, downstream reach identity, and reach order;
- synthetic contributing area or discharge for width and depth control;
- endpoint bed levels and a monotonic within-reach grade;
- bankfull width, low-flow width, floodplain width, and fixed influence bounds;
- lake boundary/rim intent, floor, water level, lowest spill, outlet reach,
  and open- or closed-basin classification; and
- downstream vectors and permanence without a point query walking the graph.

Queries reconstruct a small canonical owner neighborhood, select the stable
features whose bounds cover the point, and evaluate analytic profiles. Cache
contents, request order, camera scale, clipmap level, and traversal history may
change cost only. They may not change graph identity or geometry.

Window queries retain reconstructed catchments only for the lifetime of that
bounded request. This removes per-lattice-point graph reconstruction without
making cache contents semantic, retaining graphs across unbounded travel, or
changing the cold-reset result.

Plane and cylinder topology remain distinct. Cylinder ownership and geometry
must be periodic at the seam; directed cycles remain invalid. A future finite
world may choose complete analytical hydrology, but that is a different
profile contract rather than a hidden fallback for the unbounded plane.

## Mountain And Valley Co-Authorship

Mountain form cannot be another height multiplier beside the river graph.
Range axes and branches establish persistent divides, peaks, shoulders,
saddles, passes, fronts, foothills, and protected basins. Catchment reaches
then occupy compatible descending valleys and subdivide the mass.

The final surface combines relational profiles:

- range and branch distance establishes coherent mountain mass;
- along-axis envelopes vary summit height and prevent constant ridges;
- saddles lower selected crossings without cutting every crest;
- tributary valleys converge toward trunks and become broader downstream;
- valley heads, hanging tributaries, fans, terraces, and floodplains respond
  to reach order and grade;
- protected basins preserve lake opportunities and explicit spill saddles;
  and
- subordinate noise roughens faces, ledges, banks, and materials without
  selecting another mountain, valley, or shoreline.

Quiet lowlands remain an authored control. Not every surface should acquire a
channel, ridge, or noisy micro-feature.

## Water Surface And Shoreline Contract

Water occupancy and current are separate facts. Static source blocks retain
flat integer surfaces within one reach or lake; explicit drops join different
levels. A downstream vector and synthetic discharge may be exposed now for
future currents and ecology without implementing entity-flow physics in the
first proof.

A lake owns one coherent water level, but its visible shore is not one coherent
cliff. Shore geometry is the interaction of basin depth, rim height, incoming
valleys, substrate/resistance, exposure, slope, shelf width, wetland
suitability, and outlet reservation. Candidate shore intents include:

- shallow depositional or wetland margin;
- gravel transition and ordinary terrain arriving at water;
- rocky or exposed shore;
- incoming tributary fan or delta-like deposit; and
- incised spill/outlet threshold.

The water plane may be flat while bank height, setback, shelf, bed depth,
material, and incoming landform vary continuously around it. Broad hard-edged
shore masks and post-terrain trench/berm repair are rejected mechanisms.

## Exact And LOD Representation

One source must feed exact generation and the current procedural-horizon
clipmap. Neither representation reconstructs the other.

- Exact queries realize block strata, static water, local reach morphology,
  and whole-feature vegetation from the shared facts.
- Coarse queries evaluate the same range, valley, basin, lake, and major-reach
  profiles directly at requested coordinates.
- A major narrow reach may publish signed centerline distance, width, and a
  conservative crossing summary so sparse samples do not erase it.
- Small streams may cease to render as water at coarse levels, but their
  valleys and relation to the trunk remain part of the broad terrain.
- No LOD pan, teleport, or cold tile may construct a transit-scale raster,
  traverse the river graph, or generate exact chunks.

Plan maps and graph overlays are diagnostics. Human terrain acceptance is
based on exact and composed three-dimensional pixels.

The first implementation keeps exact spacing as a literal point query. At
coarser spacings, one eligible crossing may spend at most two bounded
perpendicular point probes against the same surface source. Water visibility
ends at spacing 16 for headwaters, 64 for tributaries, and 256 for trunk,
inlet, and outlet reaches. This is a conservative footprint presentation fact,
not a second channel or a mutation of exact terrain.

Confluences are also explicit typed facts. Their stable identity, distance,
floor, and ecology weight create a low, locally warped joining apron that
prevents nearest-reach valley sectors from leaving a geometric mound between
branches. Water occupancy remains owned by the realized reaches; the apron
does not create a separate pool or river mask.

## Initial Review Domain

The first proof is one characteristic 16-32 km catchment embedded in the
detached continental candidate. That span is a bounded hypothesis, not a
release constant. It must provide five review stories:

1. a range pass and branching headwaters;
2. a tributary confluence in a descending valley;
3. a trunk river with floodplain or terrace variation;
4. a varied lake shoreline with a visible spill/outlet; and
5. a quiet lowland control outside strong hydrographic influence.

The proof must work at arbitrary coordinates and stable owners; these are
selected review locations, not authored showcases. It proceeds through exact
terrain and LOD before Human Review D. Production integration, a general
continent-wide graph population, currents, deltas/braids, sediment simulation,
erosion simulation, and animal migration remain later decisions.

## Acceptance

The first slice succeeds only if:

- the directed graph is acyclic, source-to-sink connected, and monotonic;
- every lake has an explicit rim, level, spill decision, and compatible
  outlet or closed-basin classification;
- exact and coarse samples agree on stable identities, water ownership,
  height, and substrate at their common coordinates;
- arbitrary query, reordered query, window partition, cache reset, negative
  coordinate, and supported-cylinder seam tests agree;
- a cold teleport and retained pan remain bounded and do not trigger regional
  plan construction proportional to the traveled distance;
- the five review places look causally different in real terrain rather than
  as colored plan regions; and
- Human Review D finds recognizable mountains, valleys, streams, river,
  shoreline variation, and a quiet control without accepting production by
  implication.

## Related

- [`continental-ecoregion-planning.md`](continental-ecoregion-planning.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`deterministic-streamed-landscape-planning.md`](deterministic-streamed-landscape-planning.md)
- [`streams-reflowing-reference.md`](streams-reflowing-reference.md)
- [`lod.md`](lod.md)
- [`Tactical 220`](../tactical/220-mclone-overworld-rivers-and-wetlands.md)
- [`Tactical 222`](../tactical/222-bounded-valley-stream-structures.md)
- [`Tactical 225`](../tactical/225-watercourse-morphology-and-coastal-outlets.md)
- [`Tactical 234`](../tactical/234-terrain-lab-hydrology-and-provenance.md)
- [`Tactical 326`](../tactical/326-continental-catchment-and-landform-realization.md)
