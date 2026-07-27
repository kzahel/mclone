# Tactical 265: Macro Landform Grammar Research

Status: complete 2026-07-27. No production terrain changes. Research selects a
bounded hybrid planner and a smaller Human Review B prototype.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`
- `mclone-overworld-breadth`
- `modern-minecraft-reference`

Workstream: source-grounded 2.5D landform planning and planner selection.

## Objective

Turn Human Review A of Tactical
[`264`](264-mclone-ordinary-inland-landform-fabric.md) into a more structural
terrain direction.

The field-revision-21 candidate materially improves ordinary inland relief,
but distant review still exposes:

- repeated closed contour rings and similarly scaled hills;
- semantic family names that mostly amplify the same scalar fields;
- major rivers that remain independent warped contours;
- wetland pools and low water that do not explain catchments, spill points,
  or outlets;
- isolated river-bank and coast sand patches; and
- a far heightfield whose flat water-bearing areas provide more composition
  than its repeated positive relief.

Research the smallest topology-aware landform grammar that can provide
direction, hierarchy, circulation, and water relationships while retaining
Mclone's cheap point-sampled macro path. Compare analytic skeleton, coarse
watershed, and hybrid planner shapes before selecting any production
implementation.

The deliverable is a durable landform-grammar contract, a mechanism
comparison, and a bounded next-prototype recommendation. This tactical does
not retune field revision 21 or change terrain, water, surface, ecology,
preview, persistence, topology, or generator identity.

## Representation Boundary

Use `2.5D` precisely:

- the realized regional surface remains single-valued as `height(x,z)`;
- planning may additionally expose regions, curves, graphs, directions,
  distances, levels, hierarchy, and bounded identities;
- the surface may therefore contain an explicit ridge spine, branching
  valley, basin rim, spill saddle, plateau front, or coast arrival without
  evaluating density at every `(x,y,z)` point; and
- later selective 3D density may consume the accepted plan for undercuts,
  arches, shelves, and other multiple-solid-transition geometry.

Two-and-a-half-dimensional planning is not inherently natural. It is useful
only when its semantic objects and composition create more geographic
explanation than a scalar noise sum. Universal 3D noise is likewise not
inherently structured.

## Questions

- Which small set of primitives composes most useful Mclone journeys without
  becoming a catalogue of geological nouns?
- Which relationships make a ridge, valley, basin, plateau, and massif read
  as objects rather than classified noise?
- What should remain an everywhere-cheap point sample, and what requires a
  canonical bounded plan or coarse tile?
- Can analytic distance-field primitives supply convincing hierarchy and
  transitions without obvious cells or authored stamps?
- What minimum coarse drainage solve produces materially better catchments,
  confluences, basins, and outlets?
- Does a hybrid of planned regional envelopes and derived drainage earn its
  additional cache, halo, and periodic-topology cost?
- How do Alpha, Beta, Java 1.17.1, Java 26.2, and selected community
  generators distribute landform identity, local density, and water
  participation?
- Which real-terrain structural facts are visually valuable without claiming
  physical erosion or geological simulation?
- How should the exact 6,144-block X-periodic cylinder own crossing ridges,
  drainage adjacency, sinks, and canonical identities?
- Which plan summaries preserve ridges, lakes, outlets, and quiet corridors
  in Terrain Lab and World Explorer at coarse spacing?

## Research Stages

### Stage 1: failure trace and morphology vocabulary

- [x] Trace the five Human Review A screenshots back to the production
  height, river, wetland, coast, and surface mechanisms.
- [x] Distinguish amplitude/roughness success from hierarchy, direction,
  contour repetition, and water-connectivity failure.
- [x] Revisit the four Minecraft eras using plan view, silhouette, profile,
  transition, and journey questions rather than only roughness statistics.
- [x] Define a compact morphology vocabulary that separates envelopes,
  skeletons, regions, water relations, and local realization.

### Stage 2: real-terrain and procedural mechanism research

- [x] Research drainage divides, branching ridges and valleys, basin rims,
  spill saddles, foothills, piedmont transitions, plateaus, escarpments,
  headlands, coves, and negative space from authoritative sources.
- [x] Research relevant procedural mechanisms through primary papers,
  official source, or maintained project source.
- [x] Separate visual structural rules from expensive or unstable claims of
  physical simulation.
- [x] Record licensing and interpretation boundaries for external material.

### Stage 3: three candidate planner classes

- [x] Specify an analytic skeleton candidate using deterministic regions,
  oriented curves, signed distances, and hierarchical envelopes.
- [x] Specify a coarse watershed candidate using a canonical provisional
  raster, depression handling, flow direction/accumulation, basins, and spill
  facts.
- [x] Specify a hybrid candidate whose regional envelopes and ridge
  tendencies shape a coarse drainage solve.
- [x] Determine the smallest useful offline prototype needed to compare their
  topology, terrain relation, periodicity, cost, and visual character.
- [x] If implemented, keep all prototype code outside production generation
  and label every map as research output.

### Stage 4: grammar and selection

- [x] Select the minimal first grammar of regional envelopes, ridge/valley
  skeletons, basins, plateaus/escarpments, massifs, coast arrivals, and
  negative space.
- [x] Define typed facts, ownership, precedence, reconstruction, bounds, and
  near/far summary requirements.
- [x] Identify which features are compositional results and which deserve
  later bounded specialist families.
- [x] Select, reject, or defer each planner class with explicit evidence.
- [x] Bound the next implementation tactical and its human review gate.

## Comparison Contract

Use the same seeds and geographic bounds for every implemented prototype.
The default comparison corpus is:

- seeds `12345`, `8675309`, and `-98765`;
- an ordinary plane;
- the exact 6,144-block X-periodic cylinder;
- at least one full fundamental-domain map;
- one inland-to-coast region;
- one broad basin/lowland region; and
- one journey crossing at least three planned primitives.

Maps should make structure inspectable before block realization:

- regional envelopes and quiet-space reservations;
- ridge spines, branch order, endpoints, saddles, and influence width;
- valley/drainage edges, direction, confluences, and outlet status;
- basin ownership, rim, floor, lowest saddle, and open/closed status;
- provisional and composed height plus contours and shaded relief;
- coast arrivals and receiving-water relationships; and
- any tile, halo, canonical ownership, or periodic seam boundary.

Measure only decision-relevant facts:

- closed-contour size and repetition rather than roughness alone;
- ridge/valley branch and length distributions;
- drainage completion, cycle count, confluences, and terrain alignment;
- basin count, area, spill validity, and open/closed ratio;
- pass/saddle availability and quiet-corridor extent;
- scale hierarchy and repeated characteristic-size alarms;
- exact seam and deterministic reconstruction behavior; and
- point cost, tile construction cost, cached query cost, memory, and coarse
  summary cost.

Metrics remain alarms. Inspected plan maps, contours, oblique relief, and
journey sequences retain veto authority.

## Selection Bias

The current leading hypothesis is a hybrid:

1. cheap periodic regional envelopes establish land/ocean, quiet country,
   upland/range opportunity, broad basins, and plateau/escarpment permission;
2. bounded canonical plans create ridge and valley skeletons with hierarchy;
3. a coarse drainage pass derives catchments, sinks, spills, and accepted
   reaches from those shared facts;
4. continuous height reconstruction blends envelopes and distances without
   categorical seams; and
5. later local detail, surfaces, ecology, and selective 3D density interpret
   the accepted plan.

This is a hypothesis to challenge, not a preselected implementation.
Analytic-only planning may prove sufficient; a coarse watershed may prove too
expensive or cell-shaped; a different bounded hybrid may be clearer.

## Human Review A Failure Trace

Field revision 21 succeeds at its narrow objective. It materially increases
ordinary-land span and detrended roughness, makes positive relief reach the
coast, and preserves exact periodic and CPU/GPU output. The review does not
revoke those gains.

It does show that the objective alarms were incomplete:

| Visible evidence | Production cause | Structural diagnosis |
|---|---|---|
| many similarly sized closed contour rings | the folded ridge field and local detail are amplified by several continuous family strengths | multiple names reshape a common scalar vocabulary; no ridge spine or branch order persists across the rings |
| distant terrain reads as a sequence of hills | height is a weighted sum of broad relief, folded-ridge profile, basin lowering, mountain lift, and 32/8-block detail | amplitude and roughness increased without adding axes, hierarchy, or counterform |
| major river crosses broad relief as an independent band | the major river is a warped near-zero contour with a global sea-level plane and a later bank carve | river location is not derived from a catchment, divide, basin, or receiving outlet |
| ponds and wetland water touch lowlands without explaining them | wetland pools are a 96-block texture gated by major-river banks and low grade | low water has no basin owner, rim, spill saddle, or outlet relation |
| isolated sand spots and repeated water-edge accents | river-bank and coast materials interpret local distance and deposition selectors after geometry | surface recipes expose disconnected water/terrain authority rather than causing it |
| lake fields provide useful flat composition | water creates genuine negative shape against busy positive relief | quiet space is valuable, but its topology is accidental and frequently disconnected |

The close view makes the water problem especially legible: a pond and a large
river occupy one lowland without a receiving or outlet relationship. The
top-down maps show river corridors, lake fields, and relief contours touching
without sharing a plan. The far view is the strongest terrain evidence:
local variation is abundant, but persistent axes, catchment-scale hierarchy,
passes, and quiet counterforms are scarce.

The required correction is therefore not “more noise,” “more ruggedness,” or
“add erosion.” It is relational structure.

## Cross-Era Minecraft Mechanism Result

The source audit used the locally decompiled primary source for Alpha
v1.1.2_01, Beta 1.7.3, Java 1.17.1, and Java 26.2.

| Era | Macro mechanism | What it contributes | What it does not contribute |
|---|---|---|---|
| Alpha | scale/depth fields reshape a coarse interpolated 3D density lattice | continuous silhouettes and locally volumetric terrain | explicit range, divide, basin, reach, or outlet objects |
| Beta | Alpha-like density plus temperature participation | regional modulation without isolated terrain stamps | hydrologic or skeletal topology |
| Java 1.17.1 | 5-by-5 biome depth/scale blending feeds `BlendedNoise` | categorical hills, mountains, plateaus, and shattered identities over continuous 3D form | a terrain-owned drainage graph |
| Java 26.2 | continentalness, erosion, weirdness, and folded ridges route offset, factor, and jaggedness splines before base 3D noise | a much richer scalar terrain grammar shared with biome interpretation | persistent ridge polylines, basin objects, or derived river connectivity |
| Mclone revision 21 | periodic scalar fields route continuous quiet, rolling, ridge/valley, basin, and mountain strengths | cheap exact point samples, clear diagnostics, and better ordinary relief | hierarchy and shared terrain/water authority |

Modern Minecraft remains valuable evidence for separating semantic regional
coordinates from local density. Its `TerrainProvider` explicitly includes
low-erosion mountains, mountains, wide and narrow plateaus, plains, extreme
hills, swamps, saddles, and a river-permission threshold inside a spline
graph. `NoiseRouterData` then maps offset, factor, and jaggedness into density.
Those names still describe scalar slices rather than durable geographic
objects.

The Mclone conclusion is deliberately not “modern Minecraft has a hidden
watershed planner.” It does not. Mclone can keep the useful scalar
regional/local separation while adding an original, bounded graph/raster
planning layer for the relationships vanilla leaves implicit.

## Structural Terrain Evidence

The selected real-terrain lessons are modest visual contracts, not a claim to
simulate geology:

- a drainage basin routes to a common outlet, and ridges or hills separating
  basins are drainage divides;
- watersheds are nested, so major valleys and their tributaries need hierarchy
  rather than equal-width line texture;
- weak-relief and closed basins need explicit treatment because a defensible
  outlet may not exist and storage may fill before spilling;
- foothills and piedmont are transitions between a strong mountain front and
  low country, often including isolated ridges, fans, terraces, or a broad
  apron rather than a uniform amplitude fade;
- plateaus are valuable because a coherent high interior and an enclosing or
  partial scarp create different journeys from another rounded hill;
- rocky headlands and embayed sand/gravel shores demonstrate that coast
  character can be a consequence of inland form arriving at water; and
- negative space is not feature absence. A broad floor, open saddle,
  floodplain, basin interior, or quiet coast approach is a planned
  counterform that makes the neighboring high structure legible.

Primary terrain and hydrology references:

- [USGS: Watersheds and Drainage Basins](https://www.usgs.gov/water-science-school/science/watersheds-and-drainage-basins);
- [USGS: Drainage Area](https://water.usgs.gov/themes/hydrofabric/drainage-area/);
- [USGS: Stream Order](https://www.usgs.gov/media/images/streamorder);
- [USGS: Pediments and Alluvial Fans](https://pubs.usgs.gov/of/2004/1007/fans.html);
- [USGS: Piedmont and Blue Ridge physiography](https://pubs.usgs.gov/publication/pp1265);
- [USGS: Kentucky physiography](https://pubs.usgs.gov/pp/p1151h/physiography.html);
- [NOAA: Olympic Coast habitats](https://olympiccoast.noaa.gov/science/habitat/);
  and
- [NOAA repository: headland-embayed beach morphology](https://repository.library.noaa.gov/view/noaa/33191).

These sources motivate topology and transition, not literal dimensions,
frequencies, erosion rates, material ages, or a promise of Earth realism.

## Procedural Mechanism Evidence

Three primary procedural references materially influence the selection:

1. Génevaux et al.,
   [*Terrain Generation Using Procedural Models Based on Hydrology*](https://cs.purdue.edu/homes/bbenes/papers/Genevaux13ToG.pdf),
   generate a hierarchical river graph before terrain, derive watersheds and
   crest lines, and reconstruct a continuous surface with compactly supported
   terrain and river primitives in a construction tree. Their results show
   that hydrologic graph structure and analytic point evaluation are
   compatible rather than competing representations.
2. Génevaux et al.,
   [*Terrain Modelling from Feature Primitives*](https://www.cs.purdue.edu/cgvlab/www/publications/Genevaux15CGF/),
   represent point-, curve-, and contour-skeleton features with compact
   support and combine them hierarchically. Bounding volumes prune
   point queries, and skeletal primitives remain far smaller than a uniform
   fine raster. The model does not provide hydrologic correctness by itself.
3. Fischer, Boeckers, and Zachmann,
   [*Procedural Generation of Landscapes with Water Bodies Using Artificial Drainage Basins*](https://cgvr.cs.uni-bremen.de/papers/cgi22/CGI22.pdf),
   explicitly choose a rivers-and-lakes-first heightmap pipeline because
   adding paths to finished terrain produced weak integration. Their
   priority-grown surface demonstrates a useful inverse construction, though
   its authored regions and artificial flow map are not directly suitable as
   an unbounded deterministic Mclone planner.

Two DEM-analysis references bound the raster half:

- Barnes, Lehman, and Mulla's
  [Priority-Flood](https://arxiv.org/abs/1511.04463) provides a small,
  efficient way to fill or label depressions and derive drainage; and
- maintained [TauDEM documentation](https://hydrology.usu.edu/taudem/taudem5/help53/TauDEMToolboxOverview.htm)
  separates pit removal, flow direction, contributing area, channel
  delineation, stream order, and watershed/subwatershed products.

This research adopts algorithms and abstract representations, not code,
assets, tuned parameters, or copied feature profiles. External papers remain
references; all future Mclone implementation must be clean-room,
Mclone-owned, and compatible with repository distribution policy.

## Candidate Planner Comparison

### Candidate A: analytic skeleton

Shape:

1. cheap regional fields choose highland opportunities, quiet areas,
   orientations, and coast relationships;
2. a deterministic bounded graph grows ridge and valley curves;
3. signed-distance and along-curve profiles create height, width, passes,
   shoulders, and local detail; and
4. a spatial hierarchy prunes point queries to nearby primitives.

Strengths:

- height remains a continuous point function;
- curves naturally provide orientation, branch order, bounds, and stable IDs;
- bounded skeletons cross a periodic seam through one canonical owner and
  local work lift; and
- far summaries can retain the same graph rather than resample every block.

Weaknesses:

- convincing automatic skeleton placement is the central problem, not a
  solved input;
- arbitrary curve growth can look authored, cellular, or stamped;
- drainage, basin, spill, and outlet correctness must be invented alongside
  the curves; and
- a catalogue of feature profiles can disguise weak regional composition.

Decision: **retain as the realization and query mechanism; reject as the sole
first planner.**

### Candidate B: coarse watershed

Shape:

1. sample a provisional surface on a canonical coarse raster;
2. classify protected ocean, lake, and closed-basin sinks;
3. resolve other depressions and flats;
4. compute topology-aware flow directions and accumulation;
5. extract reaches, catchments, divides, outlets, and spill facts; and
6. carve or reconstruct terrain from those products.

Strengths:

- terrain and water have one measurable relationship;
- confluences, upstream area, basin ownership, and drainage completion follow
  from the solve;
- well-studied bounded algorithms exist; and
- a full finite torus can be audited for cycles and explicit sinks.

Weaknesses:

- solving revision 21 as-is inherits its repeated closed-ring morphology;
- ordinary depression filling would erase desired inland lake opportunities;
- raster direction and stream thresholds can expose cell scale;
- an unbounded plane tile edge is not a legitimate ocean outlet; and
- uncached point samples cannot perform a tile flood or upstream traversal.

Decision: **retain as a bounded consistency/derivation pass; reject current
height plus watershed carving as the complete terrain answer.**

### Candidate C: hybrid regional plan, coarse drainage, analytic reconstruction

Shape:

1. cheap periodic envelopes establish land/ocean, quiet country, uplift or
   highland opportunity, broad lows, front orientation, and allowed sinks;
2. a bounded canonical plan selects regional anchors, receiving outlets, and
   protected closed-basin/lake candidates;
3. a coarse provisional raster combines those envelopes with restrained
   residue, then derives flow, accumulation, basins, divides, spills, and
   accepted reaches;
4. graph extraction turns significant raster products into compact typed
   skeletons; and
5. analytic profiles reconstruct terrain around the ridge, valley, basin,
   front, and highland facts while local fields add subordinate texture.

Strengths:

- regional composition does not depend entirely on the accidental extrema of
  the current noise field;
- water relationships constrain terrain without making every final point a
  raster lookup;
- point queries and distant summaries can consume a compact spatial index;
- intended lakes survive because sinks are classified before depression
  handling; and
- topology logic is concentrated in plan ownership and graph adjacency.

Weaknesses:

- cold plan construction, halos, cache bounds, and deterministic arbitration
  are new costs;
- coarse solve and continuous reconstruction can disagree if their shared
  contracts are weak;
- too many anchors or profiles would recreate catalogue terrain; and
- plan scale can become visible unless hierarchy spans more than one cell
  scale.

Decision: **select for the next bounded prototype.**

## Selected Landform Grammar

The grammar is intentionally smaller than a list of landform nouns.

| Fact | Minimum typed content | Geometry / journey role |
|---|---|---|
| regional envelope | stable region ID, continuous strengths, broad level/range, orientation tendency, quiet reservation, water permissions | says where structure may exist and where it must recede |
| ridge/divide skeleton | nodes, directed or undirected edges, parent/order, crest level, width, endpoints, saddles, bounds | creates long high axes, spurs, passes, and separation between basins |
| valley/drainage skeleton | directed reaches, upstream/downstream IDs, order/area, bed levels, width, confluences, outlet/sink | creates circulation, nested lows, and terrain-owned water paths |
| basin | owner, boundary/rim summary, floor range, lowest saddle, sink, water/spill level, open/closed state | creates enclosed low country, lakes, wetlands, and named exits |
| front | oriented curve/region boundary, high and low side, continuity, scarp and apron widths | distinguishes a plateau/escarpment or mountain front from another rounded hill |
| bounded highland | center or small skeleton, extent, base/crest range, relation to nearby divides/fronts | supplies isolated massif, hill group, or local destination without global roughness |

Several important forms are relations or derived products, not new primitive
classes:

- a **pass** is a selected low saddle on a ridge/divide;
- **foothills/piedmont** are the structured transition between a highland or
  front and low envelope, optionally receiving fan/terrace specialists later;
- a **coast arrival** records whether a ridge, valley, front, basin low, or
  quiet plain meets its receiving coast;
- **negative space** is envelope or basin area reserved from competing
  positive relief, not a `PlainFeature`;
- a **headland/cove** can emerge where high and low arrivals meet the coast;
  and
- ordinary hills are subordinate texture or bounded highlands, not one graph
  node per bump.

Deltas, anabranches, braided reaches, alluvial fans, gorges, oxbows, sea
stacks, arches, and detailed terraces remain later bounded specialist
families. The base grammar must make room for them, but Human Review B does
not need them.

## Representation And Ownership Contract

Names remain provisional, but the ownership split is selected:

```text
WorldgenContext
    dimension key
    seed and profile revision
    horizontal topology and vertical/environment facts

MacroLandformPlan
    canonical plan ID and bounds
    construction bounds and finite halo
    regional envelope summary raster
    ridge/divide and drainage graphs
    basins, fronts, and bounded highlands
    spatial index and coarse LOD summary
    reconstruction revision and deterministic receipt
```

The planner is constructed from the dimension context and retains the compact
topology fact. It does not pass a large context into every interpolation,
distance, or profile function.

Plan construction uses canonical identity. Geometry evaluation uses one
target-relative Euclidean work lift. Rendering later chooses an
observer-relative presentation lift. Random identity, cache keys, and
downstream claims remain canonical.

Precedence is semantic:

1. topology and land/ocean facts constrain valid ownership;
2. quiet reservations, protected sinks, and major receiving waters constrain
   regional structure;
3. ridge/divide and drainage relations establish the shared skeleton;
4. basins and fronts set levels, rims, and transitions;
5. analytic reconstruction creates provisional surface;
6. accepted reaches, lakes, and later bounded structures apply their declared
   carve/fill authority; and
7. local detail, surfaces, ecology, and decoration interpret the result.

The reconstruction must be deterministic from the plan. A point query may
evaluate envelope fields plus nearby indexed primitives; it may not run flow
accumulation or walk arbitrarily upstream. Exact chunks, Terrain Lab, World
Explorer, and any future GPU evaluator consume the same revisioned summary or
honestly reject an unsupported content stage.

## Periodic And Unbounded Planning Contract

The exact 6,144-block X-periodic cylinder adds graph obligations, not a
different terrain family:

- planning-cell widths divide 6,144 where practical;
- X neighbors wrap during every raster and graph operation;
- construction halos wrap and deduplicate;
- stable plan/feature IDs use canonical X ownership;
- routes and primitive bounds use one shortest-displacement work lift with a
  deterministic half-period tie;
- a ridge, river, front, basin rim, or highland may cross X=0 without
  changing family, frequency, or quality; and
- no canonical seam becomes an outlet, wall, or feature-suppression belt.

The cylinder remains unbounded in Z. Therefore a finite diagnostic window
must receive legitimate coarser outlets/sinks or mark edge-contaminated
results; it may not pretend its north/south crop edges are oceans.

A future torus has no external edge. Every drainage component needs an
explicit ocean, lake, wetland, or closed-basin sink. Construction must detect
cycles. A monotonically descending river cannot loop around a
non-contractible direction and return to its starting level.

This supports the existing dimension-scoped context direction. It does not
justify 5D/6D noise or avoiding the seam.

## Cost And Summary Contract

The existing field graph remains the cheap everywhere-available layer.
Tactical 258 measured about 2.44 million full plane terrain samples per second
and 2.13 million cylinder samples per second in its finest 500-kilometre
control. A planned graph/raster product is a different workload and must not
be hidden in those point numbers.

The next prototype must separately report:

- envelope sampling;
- cold coarse-plan construction;
- depression/flat handling;
- flow, accumulation, graph extraction, and indexing;
- plan bytes by typed product;
- warm cached point and bounded-region queries;
- coarse-summary construction; and
- current-field control sampling.

No fixed regression budget is selected from paper timings or one host.
Architecturally:

- a cold plan build may be materially more expensive than a point;
- ordinary cached point queries should evaluate only envelope fields and
  nearby indexed primitives;
- exact generation amortizes plan construction over many chunks;
- far products use graph/envelope summaries rather than repeated high-detail
  plan traversal; and
- cache bounds and eviction remain explicit dimension/profile/topology facts.

## Next Prototype And Human Review B

Do not implement three independently tuned pretty generators. That comparison
would mostly measure how much effort went into each candidate and would force
analytic-only placement to solve the selected planner problem before it could
be judged.

The smallest decision-useful prototype is one research-first hybrid plan with
two controls:

1. current field revision 21 height and water facts;
2. hybrid coarse plan reconstructed only from regional envelope,
   ridge/divide, drainage, basin, and quiet-space facts; and
3. the same reconstruction with subordinate current local detail restored.

The first prototype deliberately excludes production output, surfaces,
vegetation, 3D density, detailed plateaus/escarpments, deltas, anabranches,
and specialist geology. It should use seeds `12345`, `8675309`, and `-98765`
on the plane and exact cylinder, generate the Comparison Contract maps, and
report construction/query cost without installing a compatibility promise.

Advance to Human Review B only when:

- major high and low axes persist across several apparent hill widths;
- tributaries merge, reaches terminate, and basins have valid sink/spill
  status;
- ridge/divide and drainage facts agree rather than intersect arbitrarily;
- some broad quiet corridors and basin floors survive;
- repeated closed-ring scale is visibly reduced against the control;
- a ridge, valley, low plain, and receiving drainage relation can reach the
  coast in the plan;
- the cylinder seam is visually and numerically ordinary;
- maps expose coarse cells, halos, ownership, graph IDs, and rejected/cyclic
  cases; and
- cold cost, cached query cost, bytes, and far-summary cost are recorded.

Human review should first inspect plan view, contours, oblique relief, and
three named journeys without surfaces or trees. If the skeleton still reads
as a cellular flow raster, a curve stamp set, or a differently colored noise
field, stop before production integration.

## Stop Condition

Stop when the research can answer:

- the minimal primitive set;
- the first planner representation;
- how water and terrain share authority;
- how plane and cylinder topology remain exact;
- how point sampling and distant summaries remain cheap;
- which visible defects the next prototype is expected to fix; and
- what evidence triggers Human Review B.

Do not change production terrain or begin surface, ecology, volumetric
geology, compound-water realization, or a general worldgen framework in this
tactical.

## Validation Record

Research completed on the 2026-07-27 Linux host:

- hydrated the gitignored Alpha and Beta primary-source specimens with
  `pnpm reference:alpha` and `pnpm reference:beta`;
- inspected the Alpha/Beta `OverworldChunkGenerator`, Java 1.17.1
  `NoiseSampler`, Java 26.2 `TerrainProvider` and `NoiseRouterData`, and
  current Mclone field/water source directly;
- reinspected all five Human Review A screenshots at original resolution;
- read the cited primary procedural papers and authoritative USGS, NOAA, and
  TauDEM material;
- compared analytic, watershed, and hybrid representations against the
  existing bounded-topology and performance contracts; and
- made no production terrain, water, surface, topology, profile, or generator
  identity change.

No prototype pixels or performance receipts are claimed by this research
tactical. They are explicit acceptance evidence for its next implementation
slice.

## Related

- [`264-mclone-ordinary-inland-landform-fabric.md`](264-mclone-ordinary-inland-landform-fabric.md)
- [`263-cross-era-inland-landform-survey.md`](263-cross-era-inland-landform-survey.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
- [`../topics/modern-minecraft-reference.md`](../topics/modern-minecraft-reference.md)
- [`../topics/still-life-and-tectonic-reference.md`](../topics/still-life-and-tectonic-reference.md)
- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
