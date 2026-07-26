# Tactical 260: Mclone Coast Intent And Shore Terrain

Status: active 2026-07-26.

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

- [ ] Add a period-compatible broad coast field under a distinct seed domain.
- [ ] Define a small public coast family/sample with proximity, selector,
  suitability, transition, and cold-response facts.
- [ ] Keep classification deterministic, point-sampled, and primarily 2D.
- [ ] Preserve accepted river and planned-stream outlet authority.
- [ ] Add focused family, transition, and exact periodic-seam tests.
- [ ] Extend production receipts/maps with family coverage and direct-water
  facts.

### Slice 2: geometry and surface realization

- [ ] Evaluate provisional terrain, coast intent, final coast geometry, then
  final slope/exposure without cyclic queries.
- [ ] Give rocky intent enough bounded near-shore relief to create genuine
  water-facing terrain instead of a stone-colored beach.
- [ ] Keep sandy shaping low and depositional; keep gravel intermediate.
- [ ] Allow ordinary grass/soil terrain directly at water.
- [ ] Add sand/sandstone, gravel/stone, rock, ordinary, and cold surface
  responses while preserving watercourse precedence.
- [ ] Bump the internal-mutable Mclone field/fingerprint evidence.

### Slice 3: preview and topology parity

- [ ] Carry the shared intent through CPU preview material/height semantics.
- [ ] Port the same field, classifier, geometry, and material semantics to the
  GPU preview evaluator.
- [ ] Update Worldgen Lens and review colors/labels.
- [ ] Prove exact CPU/GPU comparison, plane behavior, and the exact
  6,144-block / 384-chunk-X seam.
- [ ] Prove partition/order output and persistence reopen with the revised
  internal profile.

### Slice 4: measured and visual candidate

- [ ] Compare schema-14/15 coast measurements on seeds `12345`, `8675309`,
  and `-98765` at the same 6,144-block, spacing-eight grids.
- [ ] Record coast-family coverage, coherent run length, depositional width,
  and rocky/ordinary direct-water adjacency.
- [ ] Rerun Tactical 258's point, preview, exact, and World Explorer
  performance controls.
- [ ] Inspect first-drawable and final warmed pixels before presenting them.
- [ ] Capture a compact review atlas covering ordinary, sandy, gravel, rocky,
  outlet, positive/negative-seed, and cylinder-seam cases.
- [ ] Pause for Human Review 1.

### Slice 5: response and closeout

- [ ] Apply Human Review 1 classifier/geometry/material corrections.
- [ ] Rerun affected topology, determinism, persistence, performance, and
  pixel evidence.
- [ ] Present warmed ordinary and showcase regions for Human Review 2.
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
