# Tactical 260: Mclone Coast Intent And Shore Terrain

Status: Human Review 1 corrections complete 2026-07-26; awaiting Human
Review 2.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`
- `mclone-overworld-breadth`

Workstream: first topology-aware Mclone coast-family implementation.

## Objective

Replace the nearly universal elevation-band sand collar with one bounded,
shared coast intent that can produce:

1. sandy depositional shores;
2. gravel transitional shores;
3. rocky or exposed terrain entering water;
4. ordinary terrain entering water without mandatory shore material; and
5. a cold surface response without claiming complete frozen-ocean
   morphology.

Carry that intent through production point sampling, exact chunks, surfaces,
previews, diagnostics, plane/cylinder topology, persistence evidence,
performance controls, and production pixels. Pause at the first meaningful
undecorated/minimally decorated visual candidate for Human Review 1, then tune
and close the warmed production result through Human Review 2.

This tactical does not add wave or sediment simulation, universal 3D density,
overhangs, sea caves, dunes, marshes, deltas, reefs, icebergs, new persisted
biome IDs, or a general worldgen context object.

## Source Direction

Tactical
[`259`](259-modern-and-historical-coast-reference-survey.md) is the research
contract:

- Alpha and Beta demonstrate geometry-led coasts with broad material masks;
- Java 1.17.1 proves tall categorical rocky direct-water faces;
- Java 26.2 demonstrates an integrated coast/terrain/climate classifier with
  ordinary terrain still allowed at water; and
- current Mclone measurements show 89.93-90.44% Beach on coast-adjacent land,
  with every remaining sample explained by river/wetland recipes.

The Mclone implementation should use a broad independent alongshore selector,
but slope, ruggedness, relief, shelf character, climate, and accepted outlet
facts constrain the outcome. Geometry and surface material remain separate
consumers of one reconstructible semantic fact.

## Shared Ownership

- `mclone-worldgen::levelgen::mclone_overworld::fields` owns the
  topology-aware coast field, semantic intent, and bounded geometry response.
- `mclone-worldgen::levelgen::mclone_overworld::surface` owns material
  realization.
- `mclone-worldgen::terrain_preview` owns CPU preview semantics.
- `mclone-terrain-view` owns the matching GPU reconstruction and presentation.
- `mclone-overworld-review`, Worldgen Lens, and Terrain Lab consume the shared
  facts; they do not define coast rules.
- Exact chunks and stream plans remain authoritative for water/outlet
  realization.

## Slices

### Slice 1: semantic coast intent

- [x] Add a period-compatible broad coast field under a distinct seed domain.
- [x] Define a small public coast family/sample with proximity, selector,
  suitability, transition, and cold-response facts.
- [x] Keep classification deterministic, point-sampled, and primarily 2D.
- [x] Preserve accepted river and planned-stream outlet authority.
- [x] Add focused family, transition, and exact periodic-seam tests.
- [x] Extend production receipts/maps with family coverage and direct-water
  facts.

### Slice 2: geometry and surface realization

- [x] Evaluate provisional terrain, coast intent, final coast geometry, then
  final slope/exposure without cyclic queries.
- [x] Give rocky intent enough bounded near-shore relief to create genuine
  water-facing terrain instead of a stone-colored beach.
- [x] Keep sandy shaping low and depositional; keep gravel intermediate.
- [x] Allow ordinary grass/soil terrain directly at water.
- [x] Add sand/sandstone, gravel/stone, rock, ordinary, and cold surface
  responses while preserving watercourse precedence.
- [x] Bump the internal-mutable Mclone field/fingerprint evidence.

### Slice 3: preview and topology parity

- [x] Carry the shared intent through CPU preview material/height semantics.
- [x] Port the same field, classifier, geometry, and material semantics to the
  GPU preview evaluator.
- [x] Update Worldgen Lens and review colors/labels.
- [x] Prove exact CPU/GPU comparison, plane behavior, and the exact
  6,144-block / 384-chunk-X seam.
- [x] Prove partition/order output and persistence reopen with the revised
  internal profile.

### Slice 4: measured and visual candidate

- [x] Compare schema-14/16 coast measurements on seeds `12345`, `8675309`,
  and `-98765` at the same 6,144-block, spacing-eight grids.
- [x] Record coast-family coverage, coherent run length, depositional width,
  and rocky/ordinary direct-water adjacency.
- [x] Rerun Tactical 258's point, preview, exact, and World Explorer
  performance controls.
- [x] Inspect first-drawable and final warmed pixels before presenting them.
- [x] Capture a compact review atlas covering ordinary, sandy, gravel, rocky,
  outlet, positive/negative-seed, and cylinder-seam cases.
- [x] Pause for Human Review 1.

### Slice 5: response and closeout

- [x] Apply Human Review 1 classifier/geometry/material corrections.
- [x] Rerun affected topology, determinism, persistence, performance, and
  pixel evidence.
- [x] Present warmed ordinary and showcase regions for Human Review 2.
- [ ] Record the accepted visual language and update living topic ledgers.
- [ ] Close the tactical and commit the execution record.

## Quantitative Guardrails

These are review alarms, not aesthetic targets:

- no family may disappear on all three ordinary review seeds;
- the coast-adjacent Beach-equivalent share must fall materially below the
  current roughly 90%, without replacing it with one universal rock collar;
- rocky and ordinary direct-water adjacency must be nonzero;
- family transitions must form regional runs rather than sample-scale
  checkerboards;
- outlets may not gain closure, unsupported-source, or sloped-surface
  regressions;
- the cylinder seam must not become a family boundary or exclusion band; and
- point/preview cost must remain near Tactical 258's baseline, with any
  material exact-generation increase stated separately.

## Execution Record

### Semantic intent checkpoint

Field revision 18 adds one independent 768-block value-noise field. The
selected cylinder period contains exactly eight cells, so the ordinary
topology constructor supplies exact X-periodicity without a seam exception.
`McloneOverworldCoastIntent` records:

- one of Offshore, Sandy, Gravel, Ordinary, Rocky, or Inland;
- continentalness as an explicitly named signed-distance proxy, not a false
  physical block distance;
- coast proximity, broad selector, terrain-constrained character;
- depositional and rocky suitability;
- transition weight; and
- a separate low-altitude cold response.

Receipt schema 15 and the coast-intent review strip expose those facts before
they alter terrain. Equal 6,144-block, spacing-eight maps found all four coast
families at the sampled land/ocean edge:

| Seed | Sandy | Gravel | Ordinary | Rocky |
|---:|---:|---:|---:|---:|
| `12345` | 881 | 711 | 667 | 372 |
| `8675309` | 1,287 | 1,020 | 866 | 532 |
| `-98765` | 316 | 336 | 914 | 631 |

The inspected maps show broad coherent selector regions cut by the irregular
shoreline rather than sample-scale family chatter. Surface recipes remain at
their schema-14 baseline in this checkpoint, proving the semantic change is
independently measurable before geometry/material realization.

### Geometry and surface candidate

Field revision 18 now evaluates:

1. the former land height as a provisional surface;
2. the shared coast intent and its bounded coast-height response;
3. authoritative watercourse carving; and
4. final slope, exposure, biome, and surface selection.

This order avoids cyclic terrain queries and keeps an accepted river or
planned stream authoritative over coast shaping. Sandy intent remains a low
depositional response. Gravel may add zero to two blocks. Rocky intent may
add up to 22 blocks near water; grass remains on sufficiently flat shoulders
while steep faces expose stone. Ordinary terrain falls through to its climate
surface at water. Cold is a modifier over the narrow depositional coast, not
a claim that frozen-ocean morphology is complete.

Sand, gravel, and cold shore material use the narrow high-proximity part of
the shared intent. Rocky geometry may operate farther inland so a headland
has a silhouette rather than a stone-colored beach. Watercourse surface
recipes retain precedence over every coast recipe.

### Equal-grid measurements

Receipt schema 16 adds provisional/base/final surfaces, coast geometry delta,
coast families, and the distinct surface outcomes. The same 769-by-769,
spacing-eight maps used by Tactical 259 produced:

| Seed | Sandy surface | Gravel surface | Rocky surface | Cold surface | Grass/soil | River |
|---:|---:|---:|---:|---:|---:|---:|
| `12345` | 779 | 622 | 311 | 188 | 477 | 252 |
| `8675309` | 1,139 | 803 | 415 | 422 | 565 | 355 |
| `-98765` | 239 | 284 | 586 | 129 | 751 | 203 |

The sandy share of sampled coast-adjacent land is now approximately 30%,
31%, and 11%, respectively, rather than 89.93-90.44%. Rocky and ordinary
direct-water results are nonzero on every grid. Inspected family maps retain
broad regional runs rather than sample-scale checkerboards.

Sandy surface distance from water has seedwise p50 values of 40-48 blocks,
p90 values of 72-128 blocks, and maxima of 112-272 blocks. Those are
horizontal sample distances across irregular coves and spits, not a claim of
physical beach width normal to the shore. Coast geometry deltas remain
between zero and 22 blocks. Maximum sampled coast height is 86-87; p90 is
67-75. All three receipts retain hydraulic closure.

### Performance controls

Release measurements use five iterations after one warmup and compare the
current candidate with the recorded same-host Tactical 258 baseline. Because
other horizon work landed between the baseline and this tactical, the deltas
bound the current total change; they do not isolate every millisecond to the
coast field.

| Lane | Tactical 258 | Candidate | Change |
|---|---:|---:|---:|
| plane regional 65k points | 3.229 ms | 4.066 ms | +26.0% |
| plane roughly-500k medium points | 97.892 ms | 110.403 ms | +12.8% |
| plane 65k base preview | 1.948 ms | 2.176 ms | +11.7% |
| plane 65k surface preview | 1.904 ms | 2.139 ms | +12.3% |
| cylinder regional 65k points | 4.277 ms | 4.371 ms | +2.2% |
| cylinder roughly-500k medium points | 111.483 ms | 123.862 ms | +11.1% |
| cylinder 65k base preview | 2.145 ms | 2.304 ms | +7.4% |
| cylinder 65k surface preview | 2.107 ms | 2.266 ms | +7.5% |

The one added broad field is therefore visible but remains a bounded
ordinary-path cost: roughly 11-13% on the large point workload and 7-12% on
the preview workloads. The small plane lane is more sensitive to fixed cost
and host noise and raised the largest alarm. Repeated exact cold/warm medians
remained within roughly 8% of Tactical 258; no exact-generation regression
was inferred from one cold-process outlier.

The fixed-budget World Explorer control completed both native-window and
offscreen sessions with 160 ready slots and zero pending work. Offscreen
first-coarse/target times were 42.070/488.932 ms, close to the recorded
34.176/484.793 ms. Native process first-coarse/target times were
1,188.197/1,677.934 ms versus 123.906/394.627 ms, but that longitudinal lane
also includes the subsequently added vegetation pipeline and shader
construction and cannot attribute the increase to coasts. Movement remained
bounded at 2.795 ms mean / 4.670 ms p95 native and 2.607 / 4.044 ms
offscreen. All six completed-frame native captures were inspected and had
continuous terrain.

### Topology, persistence, and preview evidence

- Exact plane and cylinder field, slope, feature-realization, partition, and
  fingerprint tests pass with the revised internal profile.
- The exact 6,144-block cylinder period is continuous at chunk X zero; a
  production card centered on that seam shows no coast-family or material
  break.
- SQLite reopen tests preserve ordinary Mclone terrain and the periodic seam,
  including edits, after restart.
- The native WGPU conformance test compared 4,225 points. Base, final, and
  displayed heights had zero maximum error; water, ocean, material, channel,
  landform, biome, and surface-recipe agreement were all 1.0.
- The browser/Wasm build passes. Two headed-Wayland Terrain Lab capture
  attempts reached WebGPU but failed asynchronous validation-buffer mapping,
  so their blank panes are not claimed as pixel evidence. Native production
  pixels and native WGPU compute are the review evidence for this gate.
- The complete `mclone-worldgen` library suite passes: 371 passed and one
  ignored. Focused scene, server, persistence, hydraulic-closure, and terrain
  view suites also pass.

### Human Review 1 candidate

The inspected production atlas is
`/tmp/mclone-coast-human-review-1.png`. It contains:

- a bounded sandy cove/reach rather than a universal collar;
- a broad gravel patch whose scale and plainness need subjective judgment;
- ordinary grass/soil entering the ocean beside a major-river outlet;
- a green rocky headland with exposed water-facing stone;
- a dramatic negative-seed grass-topped cliff whose long face may read too
  straight or abrupt;
- a bounded cold shore with green/conifer terrain inland; and
- the cylinder seam centered in an ordinary/cold coast with no visible
  break.

This is intentionally the pause point. In particular, Human Review 1 should
decide gravel breadth/plainness, rocky cliff abruptness, sandy width and
family-transition crispness before any fine tuning is accepted.

### Human Review 1 response

Human Review 1 retained the steep-coast idea but did not accept the first
candidate as final. The specific defects were:

- stone handed abruptly to grass with no transition;
- sandy, gravel, and rocky masks read as broad sculpted strokes with too
  little local irregularity;
- snow covered sand while adjacent grass remained green; and
- water features acquired a concrete, repeated material “beard.”

Field revision 19 corrects those issues without adding another noise field or
changing the 768-block coast plan. It reuses the already sampled periodic
mountain detail, relief, and ridge facts to derive a local coast-realization
texture. That texture moves the final proximity and character thresholds,
feathers sandy and gravel inland edges, and selects mixed grass, coarse dirt,
gravel, and stone on rocky transition shoulders. Rocky height response
remains continuous and bounded at 22 blocks, so the accepted steep silhouette
survives.

The cold response is now a climate/altitude fact rather than a
coast-proximity mask. `SnowCover` can therefore cover sand, grass, gravel,
coarse dirt, or rock across one cold lowland while retaining the underlying
substrate. River-bank sand is now a reach-dependent depositional opportunity:
ordinary reaches may remain grass or use coarse dirt/gravel, while only
suitable reaches receive short sand traces. Planned streams and water/channel
recipes remain authoritative.

Receipt schema 17 and GPU evaluator revision
`mclone-overworld-v1-gpu-preview-a9` carry the same correction. Equal-grid
coast-adjacent surface counts are:

| Seed | Sandy | Gravel | Rocky | Snow cover | Grass/soil | River |
|---:|---:|---:|---:|---:|---:|---:|
| `12345` | 778 | 638 | 426 | 202 | 364 | 223 |
| `8675309` | 1,129 | 808 | 639 | 412 | 390 | 327 |
| `-98765` | 242 | 279 | 841 | 120 | 511 | 204 |

The sandy share and its broad alongshore regions remain close to the first
candidate rather than devolving into sample-scale noise. Sandy-distance p50
values remain 40-48 blocks, p90 values 72-128 blocks, and maxima 120-280
blocks across the three grids. Snow now covers 29,839, 30,370, and 45,641
whole-grid land samples respectively instead of tracing only a narrow shore.
All three grids retain hydraulic closure.

No new noise field is sampled. Against the Human Review 1 candidate, the
sequential roughly-500k point controls changed from 110.403 to 124.184 ms on
plane and from 123.862 to 121.540 ms on cylinder. The 65k preview controls
rose 5.6-7.2%. Exact cold-region medians rose approximately 13-14% because
snow materialization now applies across the cold ground region; the very
small warm controls remained noisy. This is a stated exact-material cost, not
a hidden increase in macro field dimensionality.

The corrected candidate passes:

- 371 `mclone-worldgen` tests with one ignored;
- 57 `mclone-terrain-view` tests with one adapter-gated test ignored;
- five Worldgen Lens tests and 14 server topology/persistence tests;
- native-client compilation and the Terrain Lab Wasm build; and
- native GPU conformance over 4,225 points with zero height error and 1.0
  agreement for every discrete material, biome, landform, water, and surface
  channel.

The exact 6,144-block cylinder seam remains continuous. The inspected Human
Review 2 atlas is `/tmp/mclone-coast-human-review-2.png`; it contains the
corrected sandy cove, mixed gravel coast, ordinary outlet, two rocky
silhouettes, cross-substrate cold coast, and centered cylinder seam. This is
another subjective gate, not an assertion that the coast language is final.

## Human Review Gates

Human Review 1 happens after the classifier, geometry, surfaces, previews,
exact topology evidence, and internally inspected pixels all agree, but before
fine tuning is treated as accepted. Review questions:

- do rocky coasts read as terrain continuing into water rather than pasted
  ridges or gray beaches?
- are sandy reaches coherent, suitably narrow, and no longer universal?
- do gravel and ordinary transitions look geographic rather than noisy?
- are horizon silhouettes and walking-scale approaches both convincing?
- do outlets remain legible and naturally connected to the receiving coast?

Human Review 2 happens after requested corrections and warmed production
captures with final water, surface, vegetation interaction, and ordinary
regions. It accepts or rejects the family as part of Mclone's terrain
language.

## Compatibility

The compatibility safety ledger marks `mclone-overworld-v1` as internal and
mutable. This tactical intentionally changes it in place, updates field
revision/fingerprints/fixtures, and assumes disposable internal worlds. It
does not change the reference-locked `overworld` profile.

## Validation

The execution record must include:

- focused and workspace Rust tests for affected shared crates;
- CPU/GPU preview comparison and shader construction;
- exact plane/cylinder seam, partition/order, and persistence fixtures;
- hydraulic closure for representative coast outlets;
- equal-grid review receipts and inspected `/tmp` maps;
- Tactical 258 performance lanes affected by the implementation;
- native production screenshots inspected before each review gate; and
- relevant browser/Android/XR/dedicated boundaries when the shared contract
  crosses them.
