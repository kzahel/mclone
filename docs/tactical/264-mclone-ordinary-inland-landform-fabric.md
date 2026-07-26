# Tactical 264: Mclone Ordinary Inland Landform Fabric

Status: implementation and objective validation complete 2026-07-26.
Awaiting Human Review A; do not begin surface/ecology interpretation.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`
- `mclone-overworld-breadth`

Workstream: topology-aware ordinary inland terrain and its arrival at water.

## Objective

Implement Slices 1-3 selected by Tactical
[`263`](263-cross-era-inland-landform-survey.md):

1. derive explicit continuous landform intent from the existing Mclone
   Overworld fields;
2. give ordinary inland terrain connected rolling, ridge-and-valley, and
   basin form without requiring the sparse mountain gate; and
3. let that incoming inland form remain legible at water while the accepted
   coast system supplies only complementary shaping.

Carry the candidate through exact production terrain, diagnostics, CPU and
GPU previews, the plane and exact 6,144-block X-periodic cylinder,
characteristic and coverage measurements, performance controls, and
geometry-isolating production maps plus warmed exact pixels. Pause before
surface/ecology reinterpretation for Human Review A.

This tactical intentionally does not add a noise field, biome, surface
recipe, vegetation rule, plateau quantizer, escarpment family, drainage
network, basin lake, compound water form, cave, overhang, or 3D density
system. It does not change the reference-locked Java 1.17.1 `overworld`
profile.

## Source Direction

Tactical 263 is the research and acceptance contract:

- Alpha shows the value of terrain structure remaining available outside
  rare categorical regions;
- Beta shows that continuous regional modulation can redistribute one
  terrain fabric;
- Java 1.17.1 shows recognizable terrain identities blended over local
  density;
- Java 26.2 shows terrain intent shared with later biome interpretation; and
- current Mclone measurements show ordinary inland structure below even the
  calmer Beta specimen at every reported 1-64-block scale.

Mclone keeps its cheap, topology-aware two-dimensional macro sampler. The
first candidate must obtain more expressive use from its existing
continentalness, relief, ruggedness, ridge, and warped detail fields rather
than hiding another independent octave stack behind a new name.

## Shared Ownership And Data Flow

- `mclone-worldgen::levelgen::mclone_overworld::fields` owns the
  topology-aware semantic intent and production height composition.
- Existing coast and watercourse facts retain their current owners and final
  carve authority.
- `mclone-worldgen::terrain_preview` owns matching CPU preview semantics.
- `mclone-terrain-view` owns matching GPU reconstruction and presentation.
- `mclone-overworld-review`, Worldgen Lens, and Terrain Lab expose the shared
  facts; they do not define a second classifier.
- Exact chunk generation remains authoritative for final block output.

The production order is:

```text
periodic raw fields
  -> continuous landform strengths and diagnostic dominant family
  -> provisional inland terrain
  -> complementary coast geometry
  -> accepted river and bounded-stream carving
  -> final slope/debug/preview facts
```

The intent is reconstructible terrain meaning, not a persisted biome ID or
final material. It contains continuous strengths so transitions do not
become categorical borders. A dominant family exists only for maps, receipts,
and review anchoring.

## Slices

### Slice 1: explicit landform intent

- [x] Define quiet plain, rolling upland, ridge-and-valley, broad basin, and
  existing mountain-range strengths from current sampled fields.
- [x] Define a deterministic dominant-family classifier without adding
  stored world state or an independent field.
- [x] Expose the intent through production debug samples and review receipts.
- [x] Add focused transition, dominance, and exact periodic-seam tests.
- [x] Expose the same intent in CPU and GPU preview diagnostics before
  accepting changed geometry.

### Slice 2: ordinary inland geometry

- [x] Route 384/128/48-block relief into connected ordinary hills and
  shallow valleys.
- [x] Route the connected ridge signal into shoulders, saddles, and
  approaches below full mountain amplitude.
- [x] Preserve broad negative relief as explicit basin/valley tendency.
- [x] Admit modest 32-block detail only where an ordinary landform strength
  permits it; keep the 8-block contribution selective.
- [x] Retain intentional quiet country and the accepted existing mountain
  family.
- [x] Avoid global roughness and categorical height seams. Repeated
  block-contour shelves remain a Human Review A concern rather than an
  objective acceptance claim.

### Slice 3: inland-to-coast composition

- [x] Keep incoming relief active through the coastal approach.
- [x] Allow ridges to terminate as headlands and low terrain to reach water
  as coves, outlets, or depositional reaches.
- [x] Reduce the existing rocky coast lift where incoming terrain already
  supplies adequate relief, without weakening its semantic family or surface
  contract.
- [x] Preserve major-river and bounded-stream final carve authority.
- [x] Record incoming versus coast-added relief and representative
  inland-to-water transects.

### Slice 4: parity, measurement, and review candidate

- [x] Prove exact CPU/GPU base, final, family, and displayed-height parity.
- [x] Prove plane behavior and exact cylinder seams for every live family.
- [x] Prove deterministic partition/order behavior and persistence reopen.
- [x] Rerun the nine-site cross-era terrain-characteristics corpus.
- [x] Measure three equal 6,144-block maps for family coverage, continuous
  strengths, slopes, low corridors, and coast arrivals.
- [x] Rerun Tactical 258's point, preview, exact, and World Explorer
  performance controls.
- [x] Capture and inspect geometry-isolating production maps and warmed exact
  production pixels. The exact showcase host currently renders existing
  decoration; no new surface or ecology rules were added.
- [x] Assemble Human Review A anchors and stop.

## Quantitative Guardrails

These are alarms, not targets:

- median ordinary-inland vertical span must exceed 24 blocks;
- median lag-16 RMS height change must exceed 3.0 blocks;
- median radius-8 detrended roughness must exceed 0.75 blocks;
- median radius-32 detrended roughness must exceed 2.0 blocks;
- quiet plain may not explain more than 70% of dry land on all three review
  seeds;
- rolling and ridge-and-valley families may not both disappear on two of
  three seeds;
- quiet terrain must remain nonzero;
- the ordinary path must remain a cheap point-sampled 2D path; and
- exact seams, watercourse closure, and CPU/GPU semantic parity are blockers.

The candidate should move materially out of Mclone's sterile baseline without
optimizing toward Alpha, Beta, or Java 1.17.1 numbers as aesthetic targets.
Record slope percentiles and extreme faces rather than assuming that larger
height spans remain navigable.

## Human Review A

Present geometry before any new ecology or geology:

- ordinary positive and negative seeds rather than only selected mountains;
- quiet plain, rolling country, ridge-and-valley, basin, and existing
  mountain anchors;
- inland-to-coast transects where those forms terminate in water;
- top-down, walking-height, and elevated production views; and
- one journey or ordered sequence crossing at least three regimes.

The review asks whether the fabric is connected, whether regional masks or
repeated contours remain too visible, whether approaches and low corridors
are navigable, whether existing mountains still feel distinct, and whether
coasts now inherit enough character from inland terrain.

Do not start surface/ecology interpretation, plateaus, escarpments, major
drainage, basin lakes, compound water forms, or selective 3D geology until
that review decides whether to accept, retune, or reject this fabric.

## Validation Record

Implementation landed in:

- `5a543144`, which exposes continuous shared intent through debug samples,
  review maps, Worldgen Lens, and CPU/GPU preview; and
- `5f59e6eb`, which routes the intent into exact terrain, makes coast lift
  complementary, hardens spawn selection, refreshes internal fixtures, and
  re-anchors stream/persistence evidence.

Field revision 21 adds no noise field. It retains the existing 384-, 128-,
48-, 32-, and 8-block inputs and changes only their topology-aware routing:

- quiet, rolling, ridge/valley, basin, and mountain strengths remain
  continuous;
- the dominant family is diagnostic rather than a categorical height gate;
- broad ordinary relief, signed ridge form, basin lowering, and selectively
  admitted local detail compose the provisional inland surface;
- positive incoming relief reduces rocky coast lift by up to 72%; and
- accepted watercourses still own the final carve.

### Terrain-characteristics result

The nine fixed 272-by-272-block ordinary-inland sites are recorded in
`/tmp/mclone-inland-hr-a-characteristics.json`.

| Characteristic | Pre-slice | Candidate | Alarm |
|---|---:|---:|---:|
| median vertical span | 16 | 25 | >24 |
| median lag-16 RMS change | 1.657 | 5.537 | >3.0 |
| median radius-8 detrended RMSE | 0.305 | 1.139 | >0.75 |
| median radius-32 detrended RMSE | 0.592 | 4.019 | >2.0 |

The nine spans are 12, 19, 20, 21, 25, 28, 31, 36, and 57 blocks. Median
land coverage is 0.813. Fine-detail residual energy remains 0.0144, so the
increase is dominated by connected broad and meso-scale form rather than
universal high-frequency noise.

### Equal-grid regional evidence

The schema-19 evidence receipts sample 769-by-769 points at eight-block
spacing for seeds `12345`, `8675309`, and `-98765`. The finalized
review-site contract is schema 20 because it adds explicit inherited-relief
and low-arrival coast anchors and excludes water-influenced columns from
representative inland-family sites.

| Seed | Dry land | Quiet | Rolling | Ridge/valley | Basin | Mountain |
|---:|---:|---:|---:|---:|---:|---:|
| `12345` | 326,166 | 84,130 | 97,664 | 106,271 | 3,209 | 34,892 |
| `8675309` | 337,542 | 120,165 | 74,536 | 125,823 | 3,605 | 13,413 |
| `-98765` | 389,838 | 70,991 | 95,079 | 192,147 | 4,113 | 27,508 |

Every family remains present on every seed. Quiet coverage is approximately
18-36%, not the 70% alarm. The diagnostic below-baseline dry-land mask has
50,901-81,796 columns; its largest components span approximately
808-by-840, 1,272-by-1,896, and 1,336-by-824 blocks. These are connected
terrain lows, not a claim of planned drainage.

Coast-adjacent incoming relief has seedwise p10 values near -2 blocks,
medians near 0-1, and p90 values near 4-5. Median coast-added lift is zero;
p90 is 3, 11, and 9 blocks, with maxima of 22, 22, and 19. Representative
high incoming-relief shores reach provisional Y77-Y79 with only zero or one
added coast block. Low arrivals begin at provisional Y62 and rely on bounded
complementary coast shaping.

The disposable receipts and maps are:

- `/tmp/mclone-inland-hr-a-fields-12345`;
- `/tmp/mclone-inland-hr-a-fields-8675309`; and
- `/tmp/mclone-inland-hr-a-fields-neg98765`.

### Exactness, topology, and persistence

- All 373 active `mclone-worldgen` library tests pass; one unrelated
  parity-gauntlet test remains ignored.
- All 65 active `mclone-terrain-view` tests pass; the adapter-gated native
  conformance test also passes when explicitly enabled.
- Native WGPU comparison covers 4,225 points with zero base, final, and
  displayed-height error and 1.0 agreement for water, material, channel,
  landform, biome, and surface-recipe channels.
- Fourteen focused server tests pass, including plane and cylinder spawn,
  exact scheduler publication, partition/order behavior, quiescent water,
  SQLite reopen, and periodic seam edits across restart.
- A seam-crossing bounded stream uses one canonical start at seed `-46`,
  chunk `(1,-47)`. Exact field/family repetition and stream realization pass
  at the 6,144-block X period.
- `pnpm native:web:build` passes. Its warnings predate and are unrelated to
  this terrain slice.
- The finalized schema-20 review diagnostic compiles and a 17-by-17 point
  smoke receipt contains the hydrology-free family and coast-arrival keys.

The production seam card at
`/tmp/mclone-inland-hr-a-card-cylinder-seam-clear` is continuous in all three
views. A discarded seed-`-46` landscape panel placed its camera only three
blocks above a foreground hill and saw that hill's vertical face; its
top-down/elevated views and exact seam tests were continuous, so it is not
used as seam evidence.

### Performance controls

Release macro receipts use five iterations after one warmup:

| Lane | Plane | Cylinder X:384 |
|---|---:|---:|
| regional 4,225 points | 4.162 ms | 3.048 ms |
| roughly-500k medium 239,121 points | 125.785 ms | 124.515 ms |
| roughly-500k fine 954,529 points | 481.445 ms | 551.418 ms |
| 65k base preview | 2.513 ms | 2.900 ms |
| 65k surface preview | 2.455 ms | 2.936 ms |

The post-coast roughly-500k medium controls were 124.184 ms plane and
121.540 ms cylinder, making the current same-host totals approximately
1.3% and 2.4% higher. The smaller lanes are more host-noisy and are retained
as raw receipts rather than used to infer a precise isolated percentage.
Ordinary point sampling remains a fixed-work 2D path.

Across three independent 3-by-3 exact receipts, median
surface/cold-feature/warm-feature times are 9.753/40.117/4.460 ms on plane
and 15.766/65.981/4.852 ms on the cylinder. Raw receipts are
`/tmp/mclone-inland-hr-a-macro-{plane,cylinder}.json` and
`/tmp/mclone-inland-hr-a-exact-*`.

The fixed-budget World Explorer smoke ends with all 160 slots ready and zero
pending work in both native-window and offscreen sessions. Offscreen movement
is 2.560 ms mean / 3.846 ms p95; native movement is 2.928 / 5.413 ms. All
twelve captures were inspected and contain continuous complete terrain.
Receipts and pixels are in
`/tmp/mclone-inland-hr-a-world-explorer`.

### Human Review A handoff

The primary inspected atlas is
`/tmp/mclone-inland-hr-a-human-review-atlas.png`. It contains exact warmed
top-down, landscape, and elevated cards for:

- quiet plain: seed `8675309`, chunk `(66,3)`;
- rolling upland: seed `-98765`, chunk `(-172,117)`;
- ridge-and-valley: seed `12345`, chunk `(-108,-45)`;
- broad basin: seed `8675309`, chunk `(-96,127)`;
- existing mountain: seed `12345`, chunk `(-129,-67)`;
- inherited high coast: seed `8675309`, chunk `(142,87)`; and
- low rocky arrival: seed `-98765`, chunk `(-72,-13)`.

`/tmp/mclone-inland-hr-a-journey-atlas.png` follows seed `12345` eastward
through diagnostic ridge, rolling, quiet, and basin families at chunk X
`-160`, `-140`, `-100`, and `-40` along chunk Z zero.

Internal pixel review finds a material improvement over the sparse baseline:
ordinary land now supplies nested hills, connected lows, exposed shoulders,
headlands, and pond-dotted country between rare mountains. It also finds
questions that require human judgment:

- block-contour rings and repeated shelves remain conspicuous;
- rolling upland, ridge/valley, and mountain silhouettes sometimes overlap;
- the strongest quiet family can still look busy rather than restful;
- the broad-basin read is often obscured by existing forest and small water;
- existing snow makes local relief appear substantially busier; and
- the independent major-river route remains visually band-like and is not
  solved by this tactical.

Do not proceed into ecology, new surface recipes, plateaus, escarpments,
basin lakes, compound water, or 3D density until Human Review A accepts,
retunes, or rejects this terrain fabric.

## Related

- [`263-cross-era-inland-landform-survey.md`](263-cross-era-inland-landform-survey.md)
- [`260-mclone-coast-intent-and-shore-terrain.md`](260-mclone-coast-intent-and-shore-terrain.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
