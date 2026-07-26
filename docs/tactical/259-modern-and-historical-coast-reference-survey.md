# Tactical 259: Modern And Historical Coast Reference Survey

Status: complete 2026-07-26.

Topics:

- `modern-minecraft-reference`
- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`

Workstream: reference tooling and source-grounded Mclone coast planning.

## Objective

Ground the first Mclone coast campaign before terrain behavior changes:

1. establish a reproducible pinned current-stable Java side reference;
2. compare the coast-selection, geometry, and material mechanisms in Alpha
   v1.1.2_01, Beta 1.7.3, Java 1.17.1, Java 26.2, and current Mclone;
3. capture and measure the current Mclone coast baseline through production
   sampling;
4. identify the smallest useful first coast-family vocabulary and inputs; and
5. update the living macro plan with an evidence-backed next tactical.

This tactical changes no Mclone terrain, biome, surface, chunk, persistence,
worker, LOD, or topology output.

## Slices

### Slice 1: modern reference bootstrap

- [x] Teach the shared mapped-release bootstrap to distinguish an official
  current unobfuscated jar from an old unmapped obfuscated release.
- [x] Add repeatable class/package-prefix selective decompilation.
- [x] Pin Java 26.2 behind `pnpm reference:modern`.
- [x] Record naming, hashes, release, tool, and selection provenance.
- [x] Preserve the Java 1.17.1 mapped/Parchment path unchanged.

### Slice 2: cross-era source survey

- [x] Record explicit and emergent coast families in Alpha, Beta, 1.17.1, and
  26.2.
- [x] Separate selection, macro geometry, surface material, climate response,
  and direct-water exceptions.
- [x] Preserve exact constants and representative fixtures where they clarify
  the mechanism.

### Slice 3: current Mclone evidence

- [x] Add coast metrics to the existing production field receipt without
  changing sampled terrain.
- [x] Run equal 6,144-by-6,144-block, eight-block-spacing maps for positive,
  negative, ordinary, and known water-review seeds.
- [x] Inspect terrain-language maps and record coast-adjacent materials,
  heights, slopes, sampled shoreline length, and land-intent beach distance.

### Slice 4: decision and closeout

- [x] Select the first bounded Mclone coast vocabulary and classifier inputs.
- [x] State topology, performance, exact/preview, and visual acceptance
  obligations.
- [x] Update the reference, macro-planning, Overworld-generation, breadth,
  tactical, and topic indexes.
- [x] Commit the research/tooling without changing terrain output.

## Initial Constraints

- Java 1.17.1 remains the parity target; Java 26.2 is comparative evidence.
- A visual family is not automatically a distinct persisted biome ID.
- Material choice may respond to coast intent but must not flatten accepted
  terrain merely to make the material fit.
- Coast facts must be reconstructible from the stored dimension descriptor,
  seed, and topology.
- Exact generation and broad previews must consume the same coast intent at
  different declared resolutions.
- No classifier may create a low-quality or featureless wrap meridian.
- The first implementation should remain primarily two-dimensional and
  preserve the measured macro sampling baseline.

## Cross-Era Findings

The eras demonstrate three distinct ways to get coastline variety. None is a
complete physical coast simulation.

| Specimen | Coast selection and geometry | Surface response | Important lesson |
|---|---|---|---|
| Alpha v1.1.2_01 | No biome or shore classifier. A continuous density field crosses the global sea level at Y64. | Broad four-octave sand and gravel masks alter top/filler material near sea level. | Rough and soft coasts emerge from terrain geometry plus overlapping material masks. |
| Beta 1.7.3 | Climate and biome top/filler arrive, but there is still no explicit beach-family classifier. | The old sand/gravel masks remain; deserts force sand, most land remains grass/dirt, and sand can become sandstone. | Climate can modify a geometry-led coast without making every shore a categorical beach. |
| Java 1.17.1 | A late four-neighbor `ShoreLayer` inserts Beach, Snowy Beach, Stone Shore, or Mushroom Field Shore from biome adjacency. Mountain IDs `3`, `34`, and `20` touching ocean become Stone Shore. | The selected shore biome supplies its own depth/scale and surface builder. | Categorical adjacency is enough to guarantee rocky water edges, but it is only indirectly terrain-aware. |
| Java 26.2 | `OverworldBiomeBuilder` owns a continentalness coast band `[-0.19, -0.11]` and combines it with erosion, temperature, humidity, weirdness, and terrain slice. Low/mid slices with erosion bands 0-2 select Stony Shore; other cells may select Beach, Snowy Beach, Desert, shattered coast, an ordinary middle biome, River, or Frozen River. | Beach families receive sand/sandstone. Stony Shore is mainly stone with a narrow two-dimensional gravel-noise interval `[-0.05, 0.05]`. | A coherent macro classifier can admit both named shore families and ordinary terrain directly at water without a late shoreline layer. |
| Current Mclone | Ocean intent is `continentalness <= 0`. Mountain strength fades toward that threshold and the Beach surface recipe wins whenever final surface is at or below sea level + 3. Exposed Stone requires Y84; Eroded Slope requires Y72. | Sand is therefore the ordinary low-coast material. | The selection and geometry rules structurally prevent ordinary rocky low coasts; this is not only a palette problem. |

The 1.17.1 exact seed-74739 chunk `(6, 8)` fixture is useful quantitative
evidence for the visual reference. Its Stone Shore/water edge spans surface
Y62-Y83; 57 sampled water-land edges have neighbor delta at least four and
the same 57 have delta at least eight. Twenty-three columns have neighbor
delta at least sixteen and 35 columns form vertical faces. Vanilla's rocky
coast is therefore a geometry-visible direct-water condition, not just gray
beach material.

Modern source answers the mechanism question more reliably than a
cross-version same-seed atlas: the releases do not share comparable seed
spaces, and the visual observation being tested was already represented by
the exact 1.17.1 fixture and current Mclone production maps. A modern
client-backed image atlas remains optional follow-up if a future question is
about block-scale appearance rather than selection architecture.

## Current Mclone Measurement

Receipt schema 14 adds a `coastMetrics` section to the existing production
field review. It defines ocean intent as `continentalness <= 0`, counts
four-neighbor land/ocean edges on the sampled raster, and measures land-side
surface recipes, height, slope, and sampled Manhattan distance from every
land-intent Beach recipe column with an ocean-intent sample in the raster to
its nearest such sample.

Three 769-by-769 maps covered 6,144 by 6,144 blocks at eight-block spacing:

| Seed | Coast land | Beach | Other | Height p10/p50/p90/max | Slope p50/p90/p99/max | Raster shore |
|---:|---:|---:|---:|---|---|---:|
| `12345` | 2,631 | 2,366 (89.93%) | 265 river/wetland | 63/64/65/65 | 0/1/1.414/1.953 | 27,744 blocks |
| `8675309` | 3,705 | 3,341 (90.18%) | 364 river/wetland | 63/64/65/66 | 0/1/1.414/1.768 | 40,160 blocks |
| `-98765` | 2,197 | 1,987 (90.44%) | 210 river/wetland | 63/64/65/65 | 0/1/1.414/1.768 | 24,656 blocks |

Every non-Beach coast-adjacent sample was River Bed, River Bank, or Wetland
Bed. Exposed Stone, Eroded Slope, and Grass/Soil were all zero. The inspected
terrain-language maps show the same result as the original screenshots:
continuous tan coastal collars punctured by outlets, with no ordinary rocky
terrain reaching the water.

Land-intent Beach recipe distance was:

| Seed | p50 | p90 | p99 | max |
|---:|---:|---:|---:|---:|
| `12345` | 128 | 368 | 752 | 896 |
| `8675309` | 128 | 336 | 600 | 856 |
| `-98765` | 104 | 280 | 1,336 | 1,712 |

This is not literal beach width. It includes every low-elevation
land-intent Beach recipe column, including inland patches. The large tails
are valuable precisely because they expose how far the current elevation
rule extends beyond a depositional shoreline concept. Raster shoreline
length is likewise comparable only on equal grids.

## Selected First Coast Contract

The first implementation should prove a small visual vocabulary without
creating new persisted biome IDs merely to name materials:

1. **sandy depositional coast** for low-gradient, suitably sheltered shelves;
2. **gravel transitional coast** for intermediate shelf, exposure, or
   substrate conditions;
3. **rocky/exposed coast** where accepted terrain, ruggedness, and substrate
   support stone or a direct face into water;
4. **cold response** as a modifier that can add snow or ice without duplicating
   the whole family table; and
5. **ordinary terrain direct to water** as an allowed outcome rather than an
   error that must be covered by a shore recipe.

Marsh, delta, dune, reef, and frozen-ocean morphology remain later water or
regional-family work. The first coast campaign should not absorb those
systems.

The classifier should consume:

- topology-aware coastal signed distance and a broad alongshore selector;
- provisional land slope, curvature, ruggedness, and exposure;
- shelf grade and width from bathymetry;
- a small substrate/resistance intent that can initially reuse existing
  terrain facts rather than requiring the geology campaign;
- temperature for the cold modifier; and
- reserved river/stream outlet facts so a coast does not close a receiving
  channel.

The broad selector should participate. It is the cheapest way to obtain long
recognizable sandy, gravel, and rocky runs instead of pixel-scale
classification chatter. It may bias among suitable outcomes, but it may not
turn a steep exposed face into a sand shelf or erase an accepted outlet.

Represent the result as a small shared `CoastIntent`-like semantic fact, not
as a global generation context or as final block material. Geometry and
surface choice remain separate consumers. Exact generation, point review,
and distant terrain should reconstruct the same intent from dimension
descriptor, seed, topology, and declared sampling resolution.

## Topology And Performance Contract

- Every new coast field must be queried through the dimension topology; its
  periodic cell count and interpolation must be compatible with the wrap.
- Cylinder validation must sample across the exact X seam and compare coast
  intent, surface, bathymetry, and outlet continuity. No feature-free seam
  exclusion band is acceptable.
- The classifier is primarily two-dimensional. The first campaign does not
  justify universal 3D density or neighborhood searches.
- Tactical 258's ordinary point, preview, exact, and World Explorer controls
  must be rerun. Add a coast-heavy exact hotspot only if geometry realization
  becomes materially more expensive than the ordinary path.
- Preview maps may filter or summarize at coarse spacing, but they must not
  invent a coast family that exact generation would reject.

## Next Tactical Acceptance

The implementation tactical should add only coast-specific review products:

- classified coast family, signed distance, shelf grade, and transition
  weight maps;
- coast-family coverage and coherent alongshore run-length distributions;
- depositional-band width and direct rocky/ordinary-water adjacency;
- the schema-14 current baseline as a before/after comparison on seeds
  `12345`, `8675309`, and `-98765`;
- positive, negative, plane, and 384-chunk-X-periodic exact fixtures,
  including a seam-crossing rocky or gravel family;
- partition/order determinism, persistence reopen, exact/preview semantic
  agreement, and outlet continuity;
- first-drawable terrain pixels before decoration and final production pixels
  from ordinary as well as deliberately rocky/sandy regions; and
- the Tactical 258 performance controls with classification cost stated
  separately from any exact geometry cost.

Do not build a generic composite-map framework for this slice. Add a composite
only if a concrete failure requires seeing two or more of coast family,
geometry, surface, bathymetry, climate, and outlet intent together.

## Validation

Completed:

- `bash -n scripts/decompile-mc.sh scripts/decompile-modern-mc.sh`;
- clean focused 26.2 bootstrap plus an idempotent rerun: 18 Java files,
  official client SHA-1
  `2dc72797acbc1b63fc16a11c4ac393605f453754`, no assets;
- expected rejection of Parchment and a changed selective receipt on the
  unobfuscated branch;
- idempotent mapped Java 1.17.1 bootstrap with 4,142 Java files retained;
- release check and build of `mclone-overworld-review`;
- three schema-14 equal-grid Mclone coast receipts and inspected
  terrain-language maps under `/tmp`;
- formatter, JSON, shell syntax, and repository diff checks; and
- source review confirming that no Mclone terrain, surface, topology,
  persistence, worker, or LOD rule changed.

The tooling and durable reference boundary landed in `0314655c`. Generated
Minecraft sources, jars, receipts, and screenshots remain local and
uncommitted.
