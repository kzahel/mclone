# Tactical 267: Hybrid Macro Landform Plan Prototype

Status: implementation and internal evidence complete 2026-07-27. Paused at
Human Review B. Research-only; production field revision 21 remains
unchanged.

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
- exact `x` versus `x + 6144` cylinder reconstruction, discrete source
  equality, and source-scalar periodic tolerance.

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
- [x] Add focused deterministic, sink, completion, and seam tests.

### Slice 2: continuous reconstruction and summaries

- [x] Build a bounded spatial index over extracted skeletons.
- [x] Reconstruct graph-only height through compact-support profiles.
- [x] Add subordinate existing local detail as a separate candidate.
- [x] Add plan and reconstruction checksums and memory accounting.
- [x] Build and measure a compact 128-block far-summary product.
- [x] Prove cached point queries do not perform drainage traversal.

### Slice 3: evidence products

- [x] Add control, envelope, plan, reconstruction, atlas, oblique, and journey
  images.
- [x] Add contour/repetition, topology, water, coast-arrival, and performance
  receipts.
- [x] Run all three seeds on plane and cylinder.
- [x] Inspect every atlas and representative oblique/journey product.

### Slice 4: Human Review B handoff

- [x] Update Tactical 265 and living topics with implementation truth.
- [x] Record exact commands, artifact paths, timings, limitations, and
  subjective internal findings.
- [x] Commit the completed research prototype and documentation.
- [x] Stop before production integration.

## Prototype Result

The contained diagnostic now implements the selected representation end to
end:

- one retained topology-aware context owns the production control sampler,
  periodic regional fields, canonical study grid, drainage forest, typed
  facts, spatial index, and reconstruction revision;
- genuine ocean cells and up to four defensible protected closed sinks seed a
  deterministic priority-grown drainage forest;
- flow accumulation, stream order, confluences, basin ownership, and spill
  facts complete with zero cycles or unreachable cells in the fixed corpus;
- drainage segments follow accepted channel parents, while divide extraction
  retains only sibling or otherwise divergent receiver ownership. Ancestor
  versus descendant reach boundaries are not terrain divides;
- compact-support profiles reconstruct ridge lift, drainage lows, protected
  basin floors, and optional subordinate production detail without a
  watershed traversal at query time; and
- a 128-block far raster retains quantized uplift, quiet, broad-low, basin,
  and base-level facts beside the already compact drainage/divide/sink
  skeleton.

The ancestor filter was a material research correction. The first literal
receiver-boundary extraction emitted 6,250 plane divide segments for seed
`12345` and produced obvious parallel hatching. Recognizing that an upstream
reach and its downstream ancestor do not define a drainage divide reduced
that case to 1,750 segments while retaining 219 distinct divergent
catchment pairs. This is the candidate in the review corpus.

## Fixed-Corpus Evidence

Final release evidence was generated from `f538a271` on the 2026-07-27 Linux
host under:

```text
/tmp/mclone-hybrid-landform-hr-b-f538a271
```

Across the six seed/topology cases:

| Receipt | Range |
|---|---:|
| cold 6,144-by-6,144 plan construction | 46.364-66.782 ms |
| detailed 36,864-point warm-query median | 33.589-45.910 ms |
| detailed warm point throughput | 0.803-1.097 million points/s |
| average / maximum indexed candidates | 8.56-18.40 / 68-116 |
| approximate working plan state | 7.82-9.78 MiB |
| 128-block far-summary construction | 0.109-0.235 ms |
| approximate far-summary bytes | 0.21-0.40 MiB |
| drainage cells | 963-1,783 |
| divergent divide segments / owner pairs | 1,123-2,294 / 165-303 |
| protected closed basins with valid spills | 2-4 / 2-4 |
| quiet land coverage | 6.48-38.01% |
| selected journey facts | 5-6 |

The warm-query lane performs the full production control sample plus the
regional envelope and nearby indexed primitives. It is therefore a stricter
workload than Tactical 258's production-only 2.13-2.44 million-point/s
baseline, and currently costs roughly two to three times as much per point.
This is not yet a production budget or optimized evaluator. It does prove the
important architectural distinction: plan construction is cold bounded work,
while a warm point query is local and does not perform flow accumulation,
depression handling, or upstream traversal.

Every fixed case has:

- zero drainage cycles and zero unreachable cells;
- a valid spill for every selected protected closed basin;
- nonzero drainage, divide, quiet, and broad-low coast arrivals;
- stable plan and reconstruction checksums;
- five measured query iterations after one warmup;
- graph-only closed-contour components reduced from 1,741-3,206 in the
  revision-21 control to 18-24, with subordinate detail ending at 47-82; and
- no discrete source surface/water mismatch on the cylinder.

Cylinder reconstruction is bit-exact at `x` and `x + 6144`: graph-only and
detailed maximum height error are both zero. Direct source scalar sampling
retains the production periodic-noise tolerance rather than bit identity,
with maximum error `2.70e-14`; source surface and water decisions match
exactly. The diagnostic canonicalizes continuous X before reconstruction,
and no seam is treated as a sink, wall, or exclusion zone.

## Internal Visual Findings

All six atlases and journeys and all paired oblique views were inspected.
The candidate makes a material structural move:

- broad high and low axes persist across many former scalar-hill widths;
- drainage branches, confluences, divides, basin lows, and quiet country read
  as parts of one composition;
- the control's dense same-scale contour noise becomes a smaller number of
  legible ridge, valley, and basin systems;
- coast approaches now carry typed drainage, divide, quiet, and broad-low
  relationships; and
- plane and cylinder cases share one visual language without a seam-specific
  feature desert.

The evidence is ready for Human Review B, not production acceptance. Retain
these concerns during review:

- a few tributary groups still expose parallel or stepped 32-block raster
  tendencies in plan view, even though reconstruction softens them;
- compact profiles can read as smooth, sculpted broad strokes or rounded
  ribbons, particularly where several divides overlap;
- graph-only and subordinate-detail maps are intentionally close at this
  scale, but the present local realization may be too weak to prevent a
  coarse planned surface from feeling sterile at walking scale;
- the small grammar repeats ridge/valley/basin vocabulary across all seeds;
  it does not yet prove fronts, plateaus, escarpments, passes, piedmont,
  bounded highlands, or specialist water forms;
- the software oblique view is a coarse diagnostic and exaggerates vertical
  crop/coast faces; it is not an exact-chunk production capture; and
- no result yet proves surface materials, ecology, block discretization,
  navigation, or selective 3D density on the planned surface.

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

Passed on the 2026-07-27 Linux host:

```bash
cargo test --manifest-path native/Cargo.toml \
  -p mclone-native-client \
  --bin mclone-landform-planner-study

cargo run --release --manifest-path native/Cargo.toml \
  -p mclone-native-client \
  --bin mclone-landform-planner-study -- \
  --output /tmp/mclone-hybrid-landform-hr-b-f538a271
```

The focused test binary passes four tests covering:

- the default fixed corpus;
- cylinder X wrap without accidental Z periodicity;
- deterministic complete acyclic drainage, protected sinks and spills, and
  divergent divide ownership; and
- bit-exact graph/detailed reconstruction seams with source scalar tolerance
  and exact discrete surface/water decisions.

The release command generated 55 primary files: one corpus receipt and eight
images plus one plan receipt for each of six cases. Review contact sheets
were composed in the same `/tmp` directory as `all-atlases.png`,
`all-obliques.png`, and `all-journeys.png`. They are convenience views, not
committed or receipt-owned primary artifacts.

Implementation commits:

- `b7d6bf1f` — bound the Tactical 267 contract;
- `b886a46b` — build the topology-aware graph core;
- `849e7e20` — add indexed reconstruction and evidence products;
- `ebf11699` — retain and reuse the topology-aware sampler context;
- `c80e383a` — stabilize warm-query timing; and
- `f538a271` — add a measured far-summary product.

Production field revision 21, exact chunks, water, surfaces, ecology,
persistence, profile identity, Terrain Lab, and World Explorer remain
unchanged.

## Related

- [`265-macro-landform-grammar-research.md`](265-macro-landform-grammar-research.md)
- [`264-mclone-ordinary-inland-landform-fabric.md`](264-mclone-ordinary-inland-landform-fabric.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
