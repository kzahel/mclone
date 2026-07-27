# Tactical 267: Hybrid Macro Landform Plan Prototype

Status: active implementation 2026-07-27. Research-only; production field
revision 21 remains unchanged.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`
- `mclone-overworld-breadth`

Workstream: topology-aware regional envelopes, drainage, landform skeletons,
analytic reconstruction, and Human Review B.

## Objective

Implement the smallest decision-useful prototype selected by Tactical
[`265`](265-macro-landform-grammar-research.md):

1. retain field revision 21 as the production control;
2. build a bounded hybrid plan from regional envelopes, protected sinks, a
   coarse drainage solve, ridge/divide and drainage skeletons, basins, and
   quiet-space facts;
3. reconstruct a continuous heightfield from indexed plan facts;
4. compare reconstruction with and without subordinate local detail; and
5. stop at Human Review B before any production integration.

The prototype must answer whether explicit relational structure can reduce
the repeated-scalar-hill character and connect terrain to water without
discarding cheap cached point queries, exact periodic topology, or coarse
summaries.

This tactical does not change production terrain, water, biome, surface,
ecology, decoration, generation profile identity, persistence, Terrain Lab,
World Explorer, GPU reconstruction, or the Java 1.17.1 reference Overworld.

## Owner And Containment

Add a standalone `mclone-native-client` diagnostic binary beside
`mclone-overworld-review`.

- It may call the production `McloneOverworldSampler` and public original
  noise primitives for controls and deterministic regional inputs.
- Planner types, algorithms, reconstruction profiles, maps, and receipts
  remain private to the diagnostic binary.
- It writes research artifacts only to an explicit output directory,
  defaulting under `/tmp`.
- Nothing in exact generation, preview compilation, persistence, or app
  runtime may depend on the prototype.
- Promotion requires a later tactical after Human Review B identifies the
  reusable shared owner and compatibility decision.

## Fixed Corpus

Use:

- seeds `12345`, `8675309`, and `-98765`;
- an ordinary 6,144-by-6,144-block plane window;
- the complete 6,144-block X-periodic cylinder fundamental domain over an
  equal Z span;
- 32-block coarse planning cells unless implementation evidence forces an
  explicitly recorded alternative; and
- one best-scoring cross-plan journey per seed/topology, with the review set
  retaining at least three journeys that cross distinct facts.

The cylinder uses canonical X in `[0, 6144)`. The plane window is centered on
the origin. Finite diagnostic Z edges are crops, not oceans or automatic
outlets.

## Prototype Pipeline

```text
seed + topology + fixed study bounds
  -> production revision-21 control samples
  -> cheap periodic regional envelopes
       continent/ocean, uplift, quiet, broad low, sink permission
  -> protected receiving-water and closed-basin sinks
  -> topology-aware coarse flood / drainage forest
       parent, accumulation, terminal sink, flat/depression facts
  -> typed plan extraction
       drainage reaches and confluences
       catchments and open/closed basins
       ridge/divide skeleton
       quiet reservations and coast arrivals
  -> bounded spatial index
  -> continuous reconstruction
       envelope + basin relation + drainage profiles + divide profiles
  -> optional subordinate local detail
  -> maps, journeys, topology checks, metrics, and cost receipt
```

The coarse solve may use Priority-Flood ideas, but intentional sinks are
classified before depression handling. It must not fill away all lake
opportunity or treat a cylinder seam/crop edge as a receiving ocean.

## Typed Research Facts

Names remain prototype-local, but receipts and debug maps expose:

- plan identity, canonical bounds, topology, cell size, and construction
  extent;
- regional envelope samples and quiet reservations;
- sink identity, kind, source level, contributing area, spill level, and
  open/closed status;
- drainage parent, accumulation, Strahler-like order, reach bounds, level,
  width, confluence, and terminal sink;
- divide segments with the neighboring catchment identities, width, crest
  lift, and bounds;
- coast-arrival counts for drainage, divide, quiet, and broad-low facts;
- indexed reconstruction candidate counts; and
- enough coarse summary data to render the plan without exact blocks.

Prototype records are reconstruction facts, not persisted game objects.

## Output Contract

For every seed/topology pair, write:

- revision-21 control height/water shaded relief;
- regional-envelope and protected-sink map;
- drainage, basin, divide, confluence, and ownership plan map;
- graph-only reconstructed height shaded relief;
- reconstructed height with subordinate local detail;
- a labeled-by-filename comparison atlas;
- control-versus-candidate oblique relief;
- a selected journey profile and typed journey receipt; and
- one schema-versioned JSON receipt.

Write a corpus index summarizing all six cases. Generated images and receipts
remain `/tmp` research artifacts and are not committed.

## Metrics And Correctness

Record:

- drainage completion, cycle count, terminal sink counts, and unreachable
  cells;
- channel cell/segment counts, confluences, outlets, internal sinks, and
  reach-order distribution;
- basin area, open/closed state, protected-sink ownership, and valid spill
  counts;
- divide segment count/length and catchment-pair identity;
- quiet-space coverage and connected-component alarms;
- coast arrivals by typed fact;
- closed-contour components and size distribution for control and both
  reconstructions;
- characteristic-size and repeated-ring alarms;
- cold envelope, drainage, extraction, indexing, and total construction
  times;
- warm graph-only and detailed point-query throughput and candidate counts;
- plan bytes by major product and summary bytes;
- stable checksums; and
- exact `x` versus `x + 6144` cylinder reconstruction and source-sample
  equality.

Metrics are alarms. Inspected maps and journeys retain veto authority.

## Slices

### Slice 0: contract and diagnostic shell

- [x] Land this tactical and index entry.
- [x] Add the standalone command and argument/receipt shell.
- [x] Prove the command cannot mutate production output.

### Slice 1: topology-aware coarse plan

- [x] Sample the control and regional envelopes.
- [x] Select receiving oceans and bounded protected sinks.
- [x] Build a deterministic plane/cylinder drainage forest.
- [x] Accumulate flow, detect cycles, classify basins, and derive spills.
- [x] Extract drainage and divide skeletons with stable prototype IDs.
- [ ] Add focused deterministic, sink, completion, and seam tests.

### Slice 2: continuous reconstruction and summaries

- [ ] Build a bounded spatial index over extracted skeletons.
- [ ] Reconstruct graph-only height through compact-support profiles.
- [ ] Add subordinate existing local detail as a separate candidate.
- [ ] Add plan and reconstruction checksums and memory accounting.
- [ ] Prove cached point queries do not perform drainage traversal.

### Slice 3: evidence products

- [ ] Add control, envelope, plan, reconstruction, atlas, oblique, and journey
  images.
- [ ] Add contour/repetition, topology, water, coast-arrival, and performance
  receipts.
- [ ] Run all three seeds on plane and cylinder.
- [ ] Inspect every atlas and representative oblique/journey product.

### Slice 4: Human Review B handoff

- [ ] Update Tactical 265 and living topics with implementation truth.
- [ ] Record exact commands, artifact paths, timings, limitations, and
  subjective internal findings.
- [ ] Commit the completed research prototype and documentation.
- [ ] Stop before production integration.

## Human Review B

Advance only if the evidence shows:

- persistent high and low axes spanning several apparent hill widths;
- nested drainage with legitimate confluences and terminal sinks;
- ridge/divide and drainage facts that agree instead of crossing arbitrarily;
- broad quiet corridors or basin floors;
- visibly fewer same-sized closed rings than revision 21;
- typed inland-to-coast arrivals;
- ordinary cylinder seam behavior;
- no dominant coarse-cell, Manhattan-divide, or curve-stamp artifact; and
- an understandable cold-build versus warm-query tradeoff.

Review the six atlases first, then paired oblique relief, then the selected
journey profiles. Reject or retune before shared extraction if the result
looks like a cellular flow raster, a dendritic texture pasted into unrelated
terrain, or merely a different noise field.

## Validation Record

Pending implementation.

## Related

- [`265-macro-landform-grammar-research.md`](265-macro-landform-grammar-research.md)
- [`264-mclone-ordinary-inland-landform-fabric.md`](264-mclone-ordinary-inland-landform-fabric.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
