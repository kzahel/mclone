# Tactical 265: Macro Landform Grammar Research

Status: active research 2026-07-27. No production terrain changes.

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

- [ ] Trace the five Human Review A screenshots back to the production
  height, river, wetland, coast, and surface mechanisms.
- [ ] Distinguish amplitude/roughness success from hierarchy, direction,
  contour repetition, and water-connectivity failure.
- [ ] Revisit the four Minecraft eras using plan view, silhouette, profile,
  transition, and journey questions rather than only roughness statistics.
- [ ] Define a compact morphology vocabulary that separates envelopes,
  skeletons, regions, water relations, and local realization.

### Stage 2: real-terrain and procedural mechanism research

- [ ] Research drainage divides, branching ridges and valleys, basin rims,
  spill saddles, foothills, piedmont transitions, plateaus, escarpments,
  headlands, coves, and negative space from authoritative sources.
- [ ] Research relevant procedural mechanisms through primary papers,
  official source, or maintained project source.
- [ ] Separate visual structural rules from expensive or unstable claims of
  physical simulation.
- [ ] Record licensing and interpretation boundaries for external material.

### Stage 3: three candidate planner classes

- [ ] Specify an analytic skeleton candidate using deterministic regions,
  oriented curves, signed distances, and hierarchical envelopes.
- [ ] Specify a coarse watershed candidate using a canonical provisional
  raster, depression handling, flow direction/accumulation, basins, and spill
  facts.
- [ ] Specify a hybrid candidate whose regional envelopes and ridge
  tendencies shape a coarse drainage solve.
- [ ] Determine the smallest useful offline prototype needed to compare their
  topology, terrain relation, periodicity, cost, and visual character.
- [ ] If implemented, keep all prototype code outside production generation
  and label every map as research output.

### Stage 4: grammar and selection

- [ ] Select the minimal first grammar of regional envelopes, ridge/valley
  skeletons, basins, plateaus/escarpments, massifs, coast arrivals, and
  negative space.
- [ ] Define typed facts, ownership, precedence, reconstruction, bounds, and
  near/far summary requirements.
- [ ] Identify which features are compositional results and which deserve
  later bounded specialist families.
- [ ] Select, reject, or defer each planner class with explicit evidence.
- [ ] Bound the next implementation tactical and its human review gate.

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

Pending research.

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
