# Tactical 223: Mclone Climate And Bookend Biomes

Status: active 2026-07-23.

Topics: `mclone-overworld-generation`, `mclone-overworld-breadth`

Workstream: pure periodic climate fields, original-profile biome recipes,
surface and decoration language, review maps, far LOD, persistence, and
bounded performance.

## Objective

Expand `mclone-overworld-v1` beyond its mostly green temperate palette with
the first climate-driven regional system:

- independent broad temperature and moisture fields;
- a cool-wet conifer family with spruce/pine, ferns, and berries;
- a snowy alpine family selected by climate, altitude, and exposure;
- a warm-dry steppe family with golden grass, tall grass, and sparse acacia;
- the existing temperate meadow and oak woodland preserved between those
  bookends; and
- deterministic maps, topology, LOD, persistence, performance, and production
  pixel evidence before human review.

This tactical proves the climate and recipe machinery. It does not attempt all
biome breadth at once.

## Baseline

Field revision 12 emits five biome IDs: ocean `0`, plains `1`, forest `4`,
river `7`, and beach `16`. Only plains and forest have Mclone decoration
tables, both using oak, grass, dandelions, and poppies. Mountains and wetlands
mostly reuse plains identity, and ocean/beach/river have no profile-owned
vegetation.

Terrain-scale breadth is already stronger: continents, coasts, bathymetry,
lowlands, wooded uplands, mountains, valleys, rivers, wetlands, exposed
stone, and the reviewed creek are live. This slice changes regional color,
surface, and vegetation language without replacing that geometry.

Before field changes, pin:

- current field/terrain/decorated fingerprints;
- seed `-98765` stream and mountain review sites;
- a no-stream origin generation baseline;
- emitted biome/surface ratios over at least three broad regions; and
- the exact 384-chunk-X periodic seam.

## Java 1.17.1 Reference Read

The local reference sources were read before implementation:

- `VanillaBiomes.taigaBiome(...)` combines spruce/pine vegetation, ferns,
  flowers, taiga grass/mushrooms, berries, surface freezing, and
  temperature/downfall identity;
- `baseSavannaBiome(...)` combines no precipitation, warm temperature, zero
  downfall, acacia, tall/savanna grass, and warm flowers;
- `BiomeDefaultFeatures` keeps trees, grass, flowers, berries, and freezing as
  separately ordered feature families;
- Java's biome palette treats hills/plateaus/modified variants as variations
  of recognizable regional language; and
- the shared Rust engine already ports the relevant tree placers, plant
  patches, snow/ice blocks, and tint definitions.

Mclone deliberately does not port the vanilla layered-biome distribution.
It owns continuous climate fields and composes them with its existing
landform facts.

## Climate Contract

Add a named `McloneOverworldClimateSample` with normalized temperature and
moisture in `[-1,1]`.

Initial field shape:

- temperature uses independent 1,536- and 384-block domains;
- moisture uses independent 1,024- and 256-block domains;
- all scales divide the 6,144-block cylinder circumference;
- values are pure absolute-coordinate samples and never depend on request
  order, feature placement, or cache residency;
- altitude-adjusted temperature is a derived biome fact, not written back
  into the raw climate field; and
- fields remain independent of continentalness/ruggedness so dry, wet, warm,
  and cool regions can occur across more than one landform.

The exact scale/weight thresholds may move after the first maps. Do not tune
from a single screenshot. Record area ratios, patch diameters, boundary
length, and landform cross-products for several seeds.

## Recipe Contract

Introduce one profile-owned recipe classification before mapping to
vanilla-compatible biome IDs:

```text
ocean / shore / river override
  -> snowy alpine
  -> cool-wet conifer
  -> warm-dry steppe
  -> sheltered temperate woodland
  -> open temperate meadow
```

The recipe owns semantic selection; the biome ID supplies current
renderer/protocol tint and biome facts:

| Recipe | Initial biome ID | Initial language |
|---|---:|---|
| cool-wet conifer | taiga `5` | spruce/pine, fern-rich ground, sparse berries |
| snowy alpine | snowy mountains `13` or snowy tundra `12` after first maps | snow-covered open highland, exposed rock, treeless or very sparse trees |
| warm-dry steppe | savanna `35` | warm grass tint, tall grass, sparse acacia |
| temperate woodland | forest `4` | existing oak upland |
| temperate meadow | plains `1` | existing open grassland |

Water and shore keep their existing IDs in this slice. Frozen rivers/oceans,
warm reefs, climate-colored water, and richer shore variants belong to later
families so this tactical does not reopen the settled hydraulic work.

Boundaries should be broad and legible. Biome tint blending may soften block
color, but classification must not flicker at fine-noise scale.

## Surface Contract

- Cool-wet conifer remains grass/soil initially; future ancient forest may add
  podzol/coarse-dirt mosaics.
- Warm-dry steppe remains a grass block so biome tint produces golden ground;
  small dry-soil patches may follow only if pixels need them.
- Snowy alpine receives an explicit terrain-scale snow surface visible to
  full chunks and synthetic far LOD, while steep/exposed faces may retain
  stone.
- Watercourse, wetland, beach, river-bank, and exposed-stone priorities remain
  explicit and deterministic.
- Climate must not place snow beneath water, vegetation, or a later structure
  by accident. The selected pipeline order must be documented in the
  realization checkpoint.

## Decoration Contract

Reuse existing shared feature primitives while owning Mclone density and
selection:

- conifer: Java-shaped spruce/pine tree placers, ferns, sparse large ferns,
  and restrained berries;
- snowy alpine: initially none or extremely sparse conifer accents, depending
  on the first full-distance pixels;
- steppe: sparse acacia using the existing forking trunk/flat canopy, tall
  grass, ordinary grass, and restrained warm flowers;
- temperate rows retain their current exact tables unless a deliberate
  fingerprint update is recorded.

Feature randomness uses a new decoration revision/domain only where needed.
No TypeScript or app-local biome policy is allowed.

## Review Tooling

Production review receipts must add:

- raw temperature;
- raw moisture;
- altitude-adjusted temperature;
- regional recipe;
- biome ID;
- surface recipe;
- climate/landform cross-product counts; and
- per-family decoration block counts.

Maps should include separate temperature and moisture panels plus a combined
climate/recipe card. At least three seeds should show all active land families
without requiring pathological search distances. Review sites must include
one conifer/temperate boundary, one snowy alpine shoulder, one steppe valley,
and one watercourse crossing a climate boundary.

## LOD, Persistence, And Topology

- Full chunks, synthetic far LOD, review sampling, native workers, and browser
  Workers consume the same climate and recipe facts.
- Snow is a terrain-scale visual fact and must remain visible in far LOD;
  trees and ordinary plants may retain the existing feature-omitting LOD
  policy.
- The exact 384-chunk-X periodic seam must match for climate, recipe, surface,
  and final chunks.
- The current internal-mutable profile may update field/decoration revisions
  and disposable fixtures in place.
- SQLite and shared portable chunk records already persist final biome and
  block payloads; prove one new-family close/reopen rather than inventing a
  separate climate record.

## Performance Contract

Climate adds hot-path samples to every surface column and LOD tile.

- Compare revision 12 and the candidate on the same host at a temperate
  control plus conifer, alpine, and steppe sites.
- Record field-map time and surface/cold/warm targets per second.
- A greater-than-15-percent broad no-feature-control regression requires
  profiling; a twofold regression blocks visual review.
- Climate samplers must remain cheap immutable values; do not add global
  registries or per-column allocation.
- Retain batch sampling and existing stream-plan cache reuse.
- Run a 3,600-frame mixed-climate traversal if the new decoration or snow
  materially changes render/stream pressure.

## Explicit 3D-Geology Handoff

The user's requested overhangs, arches, rock formations, and interesting
three-dimensional outcrops are first-class next work in
[`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md).
They are not hidden inside biome decoration and not implemented as a few
cosmetic blobs in this tactical.

The dedicated follow-up should compare:

1. placed boulder/talus features;
2. bounded structure-shaped tors/arches/hoodoos using start/piece metadata;
3. a selective regional 3D density modifier for cliff shelves and overhangs.

It must render and benchmark each mechanism before choosing a reusable
formation stack. Disabled Caves & Cliffs Part 1 reference paths remain out of
scope.

## Slice Plan

### Slice 0: ledger, references, and baseline

- [x] Create the Mclone breadth tracker with vanilla grouped reference
  families and explicit mechanism/live/reviewed states.
- [x] Record 3D geology as a dedicated formation campaign.
- [ ] Pin current field, biome, surface, decoration, topology, and performance
  baselines.

### Slice 1: climate fields and maps

- [ ] Add periodic temperature/moisture sampling and raw fingerprints.
- [ ] Add altitude-adjusted temperature and recipe classification.
- [ ] Add production maps, ratios, cross-products, and multi-seed review.
- [ ] Tune only broad scales and thresholds; generated blocks remain unchanged
  until the distribution is accepted internally.

### Slice 2: biome payload and surface realization

- [ ] Emit conifer, alpine, and steppe biome IDs.
- [ ] Add alpine snow/exposed-rock surface language.
- [ ] Preserve water/shore priorities and exact target partition.
- [ ] Update intentional field and surface fingerprints.

### Slice 3: decoration language

- [ ] Add conifer vegetation.
- [ ] Add sparse steppe acacia/tall-grass language.
- [ ] Decide whether alpine needs sparse trees from inspected pixels.
- [ ] Prove deterministic decoration counts, boundaries, and periodic seams.

### Slice 4: production closeout

- [ ] Prove far-LOD climate/snow agreement.
- [ ] Prove native SQLite reopen of one chunk from each new family.
- [ ] Compile browser Worker/WASM boundaries.
- [ ] Benchmark controls and family hotspots.
- [ ] Capture fully warmed high-view-distance multi-family cards.
- [ ] Stop for human judgment of distribution, color, density, snowline, and
  family identity.

## Stop Conditions

Pause before expanding scope if:

- the first climate maps require a materially different regional model;
- biome identity needs a new persisted registry rather than current compatible
  IDs;
- snow cannot agree between authoritative chunks and far LOD without changing
  a broader surface contract;
- shared feature placement cannot express sparse Mclone-owned densities
  without importing whole vanilla tables; or
- the first complete pixels present materially different distribution or
  art-direction choices.

Otherwise continue through objective gates and stop at the first complete
human visual review point.
