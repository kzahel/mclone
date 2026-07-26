# Tactical 264: Mclone Ordinary Inland Landform Fabric

Status: active 2026-07-26. Stop at Human Review A.

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
undecorated production pixels. Pause before surface/ecology reinterpretation
for Human Review A.

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

- [ ] Define quiet plain, rolling upland, ridge-and-valley, broad basin, and
  existing mountain-range strengths from current sampled fields.
- [ ] Define a deterministic dominant-family classifier without adding
  stored world state or an independent field.
- [ ] Expose the intent through production debug samples and review receipts.
- [ ] Add focused transition, dominance, and exact periodic-seam tests.
- [ ] Expose the same intent in CPU and GPU preview diagnostics before
  accepting changed geometry.

### Slice 2: ordinary inland geometry

- [ ] Route 384/128/48-block relief into connected ordinary hills and
  shallow valleys.
- [ ] Route the connected ridge signal into shoulders, saddles, and
  approaches below full mountain amplitude.
- [ ] Preserve broad negative relief as explicit basin/valley tendency.
- [ ] Admit modest 32-block detail only where an ordinary landform strength
  permits it; keep the 8-block contribution selective.
- [ ] Retain intentional quiet country and the accepted existing mountain
  family.
- [ ] Avoid global roughness, terraces, repeated contour shelves, and
  categorical height seams.

### Slice 3: inland-to-coast composition

- [ ] Keep incoming relief active through the coastal approach.
- [ ] Allow ridges to terminate as headlands and low terrain to reach water
  as coves, outlets, or depositional reaches.
- [ ] Reduce the existing rocky coast lift where incoming terrain already
  supplies adequate relief, without weakening its semantic family or surface
  contract.
- [ ] Preserve major-river and bounded-stream final carve authority.
- [ ] Record incoming versus coast-added relief and representative
  inland-to-water transects.

### Slice 4: parity, measurement, and review candidate

- [ ] Prove exact CPU/GPU base, final, family, and displayed-height parity.
- [ ] Prove plane behavior and exact cylinder seams for every live family.
- [ ] Prove deterministic partition/order behavior and persistence reopen.
- [ ] Rerun the nine-site cross-era terrain-characteristics corpus.
- [ ] Measure three equal 6,144-block maps for family coverage, continuous
  strengths, slopes, low corridors, and coast arrivals.
- [ ] Rerun Tactical 258's point, preview, exact, and World Explorer
  performance controls.
- [ ] Capture and inspect undecorated first-drawable and warmed production
  pixels.
- [ ] Assemble Human Review A anchors and stop.

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

Present undecorated geometry before any new ecology or geology:

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

Pending implementation.

## Related

- [`263-cross-era-inland-landform-survey.md`](263-cross-era-inland-landform-survey.md)
- [`260-mclone-coast-intent-and-shore-terrain.md`](260-mclone-coast-intent-and-shore-terrain.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
