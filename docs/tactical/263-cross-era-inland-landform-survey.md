# Tactical 263: Cross-Era Inland Landform Survey

Status: complete 2026-07-26.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`
- `mclone-overworld-breadth`
- `modern-minecraft-reference`

Workstream: source-grounded inland terrain research and next-tactical
planning.

## Objective

Compare the inland terrain mechanisms and ordinary-land character of four
Minecraft Java specimens:

1. Alpha v1.1.2_01;
2. Beta 1.7.3;
3. Java 1.17.1; and
4. pinned current-stable Java 26.2.

Use current `mclone-overworld-v1` as the candidate/control. Identify why its
inland terrain feels sparse and sterile, explain what each Minecraft era
contributes to a better direction, and define a bounded first inland-feature
implementation plan.

This tactical changes no Mclone terrain, biome, surface, decoration,
topology, persistence, preview, or generator identity. Its only code change
extends the existing offline terrain-characteristics tool to admit the exact
Alpha and Beta ports and to retain all-water survey windows.

## Questions

- Is the visible sterility only a lack of rare mountains, or is ordinary
  inland terrain also too smooth?
- How do the four Minecraft eras make hills, valleys, ridges, plateaus,
  cliffs, and exceptional formations available?
- Which eras tie terrain to biome identity, climate, or shared semantic
  fields?
- Which parts require three-dimensional density, and which should remain
  cheap two-dimensional planning facts?
- How can Mclone gain frequent connected landform structure without turning
  the whole world into mountains or destroying intentional quiet plains?
- How should inland form continue into coasts, water, ecology, previews, and
  the exact 6,144-block cylinder?

## Research Corpus

The generated, gitignored source trees were hydrated with:

```bash
pnpm reference:alpha
pnpm reference:beta
pnpm reference:modern
```

The Java 1.17.1 tree was already present. The primary source surfaces were:

- Alpha and Beta
  `net/minecraft/world/gen/chunk/OverworldChunkGenerator.java`;
- Java 1.17.1 `NoiseSampler`, `NoiseGeneratorSettings`, `VanillaBiomes`,
  `RegionHillsLayer`, `BiomeInitLayer`, and related layer assembly;
- Java 26.2 `TerrainProvider`, `NoiseRouterData`, and
  `OverworldBiomeBuilder`; and
- Mclone's production `mclone_overworld::fields`, coast, biome, surface, and
  debug classifiers.

Alpha and Beta terrain and surface stages are exact against their pinned Java
oracle receipts. The Java 1.17.1 side uses Mclone's reference-locked native
port. Java 26.2 remains source/mechanism evidence: there is no local exact
modern generator port, so this survey does not invent same-seed modern
numbers.

## Measurement Method

`pnpm native:worldgen:terrain-characteristics` now accepts:

- `alpha-v1`;
- `beta-v1`;
- `overworld`; and
- `mclone-overworld-v1`.

Every runnable site becomes a one-block top-solid height raster before
carvers and decoration. Alpha, Beta, and Mclone may have surface material
present, but material cannot alter the extracted top-solid geometry. Water
and air are excluded, and only land at Y>=67 enters the characteristic
analysis.

The tool records:

- land coverage, including an explicit zero for an all-water window;
- top-solid height distribution;
- RMS height change at separations from 1 through 128 blocks;
- five-point curvature;
- detrended local-plane residual roughness at radii 2 through 32;
- local orientation coherence and diagonal alignment;
- roughness exponent; and
- fine-detail residual-energy share.

The survey had two stages:

1. A deterministic scan used 36 candidate centers per profile across seeds
   `12345`, `-98765`, and `8675309`. It established land occupancy and found
   inland windows without hand-selecting only dramatic mountains.
2. The final ordinary-inland corpus used three high-land-coverage centers per
   seed and profile. Each final site is 17 by 17 chunks, or 272 by 272
   one-block samples. Nine sites per profile therefore contribute medians.

Conditioning on land is deliberate: the question is what ordinary inland
terrain looks like after arriving on land, not how much ocean each unrelated
seed space produces. The corpus does not estimate the global frequency of
every biome or landform, and the values are directional review evidence, not
parity targets.

The disposable receipts are:

- `/tmp/mclone-inland-era-scan.json`;
- `/tmp/mclone-inland-era-survey.json`; and
- `/tmp/mclone-inland-conditioned-survey.json`.

## Source Findings

### Alpha v1.1.2_01: continuous structure almost everywhere

Alpha has no inland landform classifier and no biome-owned terrain family.
One continuous three-dimensional density system supplies the whole world:

- min-limit, max-limit, and selector noise blend the local density;
- broad scale and depth noise stretch and move the density baseline;
- a 5 by 17 by 5 lattice is interpolated through the 16 by 128 by 16 chunk;
- sea level is Y64 and a top fade pulls density toward air; and
- sand/gravel surface masks change material without changing geometry.

There is no categorical “mountain region on/off” gate. Hills, cliffs,
overhangs, floating terrain, depressions, and rough water edges are possible
wherever the continuous density happens to create them. The result has
limited semantic control and can be capricious, but it avoids broad inert
land because structural variation is globally available.

Useful Mclone lesson: give ordinary inland terrain a nonzero structural
vocabulary before selecting rare signature families. Three-dimensional noise
is one way Alpha realizes that structure, but the more fundamental lesson is
availability, not a requirement to copy its whole density stack.

### Beta 1.7.3: climate redistributes one continuous terrain fabric

Beta retains Alpha's density lattice and core noise banks. Its important
terrain addition is continuous climate modulation:

```text
g = downfall * temperature
h = 1 - (1 - g)^4
terrain scale noise *= h
```

Warm/wet and dry/cold regions therefore receive different amounts of the
broad terrain-scale signal before discrete biome surfaces and features are
chosen. A biome ID does not independently turn mountains on. Climate
redistributes one continuous geometry-led fabric.

Beta is calmer than Alpha in the measured corpus, but still gives ordinary
land materially more 4-64-block structure than Mclone. Its limitation is the
opposite of modern Minecraft's: it has organic continuous variation but
little explicit regional landform grammar.

Useful Mclone lesson: one broad controller can alter terrain character
continuously without islands of categorical geometry. Climate need not own
Mclone terrain, but shared regional facts should modulate form rather than
merely recolor it afterward.

### Java 1.17.1: explicit regional identities over 3D local density

Java 1.17.1 combines two layers:

1. the layered biome source selects recognizable regional terrain families;
2. `BlendedNoise` supplies local three-dimensional density inside them.

`RegionHillsLayer` inserts hills and rare mutations with neighborhood
support. `NoiseSampler` then blends neighboring biome depth and scale through
a weighted 5 by 5 biome window before density fill. Representative depth and
scale pairs include:

| Family | Depth | Scale |
|---|---:|---:|
| plains/desert | about 0.125 | 0.05 |
| ordinary forest | about 0.1 | 0.2 |
| hills | 0.45 | 0.3 |
| mountains | 1.0 | 0.5 |
| mountain edge | 0.8 | 0.3 |
| savanna/badlands plateau | 1.5 | 0.025 |
| shattered savanna | 0.3625 | 1.225 |
| shattered savanna plateau | 1.05 | 1.2125 |

The biome family supplies regional baseline and amplitude; the blended 3D
field supplies cliffs, overhangs, and local irregularity. Other exceptional
forms use distinct mechanisms: eroded-badlands pillars are surface-builder
geometry, ice spikes are placed features, and caves/ravines are carvers.

This gives strong identities and broad terrain availability, but geometry is
tightly coupled to categorical biome regions. Mclone should copy the
separation of regional intent from local density, not make a biome ID the sole
owner of a mountain, plateau, or valley.

### Java 26.2: shared semantic terrain coordinates

Modern Java makes the terrain/biome relationship more explicit. Its common
two-dimensional semantic inputs include:

- continentalness;
- erosion;
- ridges;
- folded ridges / peaks-and-valleys; and
- weirdness.

`TerrainProvider` maps those inputs through separate offset, factor, and
jaggedness splines. Its erosion-offset construction explicitly contains
very-low-erosion mountains, mountains, wide and narrow plateaus, plains,
extreme hills, and swamps. Jaggedness is inactive offshore and is strongest
in suitable low-erosion inland ridge regions.

`NoiseRouterData` composes those planned facts approximately as:

```text
depth = vertical gradient + offset
initial density = factor * (depth + jaggedness)
final terrain adds the base three-dimensional blended noise
```

`OverworldBiomeBuilder` consumes the same continentalness, erosion, and
peaks/valleys axes to choose peaks, slopes, plateaus, middle terrain,
lowlands, valleys, coasts, and rivers. Terrain semantics therefore exist
before biome/ecology interprets them. The regional plan remains cheap and
two-dimensional while `sloped_cheese` retains local volumetric form.

Useful Mclone lesson: terrain fact first, biome interpretation second. A
small original landform-intent vocabulary can route specialized shaping
without importing the modern density-function graph, literal splines,
384-block world height, aquifers, or cave stack.

### Current Mclone: capable fields, over-gated realization

Mclone already samples useful periodic fields:

- continentalness at 2,048/1,024/512-block scales;
- relief at 384/128/48;
- ruggedness at 1,536/512;
- ridges at 384/128; and
- warped mountain detail at 32/8.

The current height formula is nevertheless dominated by a quiet fallback:

```text
base = 64 + inland_strength * 18
rolling = relief * (2 + inland_strength * 7)
mountain = mountain_strength * ridge-shaped lift
detail = mountain_detail * mountain_strength * ridge-shaped amplitude
```

`mountain_strength` is the product of inland and rugged-region gates. The
32/8-block detail is exactly zero outside that gate. Ordinary land therefore
receives at most a modest broad relief term, while the accepted detailed
mountain family occupies sparse, seed-dependent regions.

The current debug classifier names Lowland, Upland, Mountain Valley,
Mountain Shoulder, and Mountain Massif, but those labels mostly interpret
the final height and one mountain gate; they do not route several independent
inland geometry families. Existing three-seed, 6,144-block maps reinforce the
distribution problem: sampled mountain-region share was approximately 9.06%,
1.08%, and 9.94% of dry land, while the highland share was approximately
0.29%, 0.07%, and 1.06%.

This is why accepted mountain-detail tuning did not solve ordinary-world
sterility. The detailed mechanism is locally competent but absent from most
inland journeys.

## Quantitative Result

The final nine-site ordinary-inland medians are:

| Characteristic | Alpha | Beta | Java 1.17.1 | Current Mclone |
|---|---:|---:|---:|---:|
| Window vertical span | 44 | 29 | 48 | **16** |
| Maximum span in corpus | 54 | 43 | 64 | **21** |
| P95-P05 height band | 29 | 16 | 27 | **9** |
| P95-P50 upper relief | 18 | 12 | 19 | **2** |
| Lag-1 RMS delta | 1.232 | 0.704 | 1.148 | **0.298** |
| Lag-4 RMS delta | 3.275 | 1.827 | 3.003 | **0.608** |
| Lag-16 RMS delta | 7.646 | 4.194 | 7.303 | **1.657** |
| Lag-64 RMS delta | 11.892 | 7.355 | 12.109 | **3.134** |
| Curvature RMS | 2.379 | 1.393 | 2.405 | **0.731** |
| Plane-fit radius-8 RMSE | 2.210 | 1.208 | 2.155 | **0.305** |
| Plane-fit radius-32 RMSE | 5.140 | 3.029 | 6.038 | **0.592** |

The result is not “Mclone has less Alpha chaos,” which could be an intentional
identity. Mclone ordinary inland terrain is below even the calmer Beta
specimen across every reported 1-64-block structure and detrended-roughness
measure. Its median P95 is only two blocks above its median height. Most of a
272-block view is therefore one gentle grade with surface/ecology changes
drawn over it.

The previous mountain-only benchmark remains useful: accepted Mclone mountain
sites now have meaningful local detail but retain broader, smoother scale
allocation than selected Java 1.17.1 mountain sites. The new result is more
fundamental. Ordinary inland windows do not reach those mountain mechanisms
at all.

Do not optimize a new generator against these exact values. Alpha, Beta, and
1.17.1 are three different visual languages, and the corpus is small. The
numbers are a directional alarm and a repeatable before/after baseline.

## Inspected Rendered Evidence

Four production showcase cards were captured at RD16 and inspected:

| Profile | Site | Geometry range | Pixel finding |
|---|---|---:|---|
| Alpha | seed `-98765`, chunk `(0,-64)` | Y67-Y113 | Nested hills, ridges, bowls, cliff faces, and small water interruptions remain present across the whole view. |
| Beta | seed `-98765`, chunk `(0,-64)` | Y67-Y104 | Broader calmer hills still produce continuous approaches, saddles, river margins, and climate/material transitions. |
| Java 1.17.1 | seed `-98765`, chunk `(128,-96)` | Y67-Y115 | Several terrain identities meet: exposed broken uplands, snowy high ground, forested slopes, basins, and water. |
| Mclone | seed `-98765`, chunk `(128,-96)` | Y67-Y86 | Biome and snow boundaries dominate a shallow surface; the forest hides an almost level interior and broad contours read as bands. |

The disposable card paths are:

- `/tmp/mclone-inland-alpha/alpha-v1-seed-neg98765-chunk-0-neg64-card.png`;
- `/tmp/mclone-inland-beta/beta-v1-seed-neg98765-chunk-0-neg64-card.png`;
- `/tmp/mclone-inland-release/overworld-seed-neg98765-chunk-128-neg96-card.png`;
  and
- `/tmp/mclone-inland-current/mclone-overworld-v1-seed-neg98765-chunk-128-neg96-card.png`.

The cards are qualitative evidence, not committed fixtures. Their source
receipts record the exact profile, seed, chunk, render distance, readiness,
and view configuration.

## Cross-Era Synthesis

Each era contributes a different missing ingredient:

| Need | Strongest lesson |
|---|---|
| structure should not disappear outside rare regions | Alpha's globally available continuous density |
| regional conditions should reshape rather than only recolor | Beta's continuous climate modulation |
| recognizable landform families and transitions | Java 1.17.1's depth/scale regions plus neighborhood blending |
| terrain and biome should interpret the same facts | Java 26.2's shared semantic coordinates and routed spline families |
| cheap broad previews and exact periodic topology | current Mclone's pure 2D sampler and topology-aware field inventory |
| true overhangs and multiple solid/air transitions | selective 3D realization from every Minecraft density era |

The selected Mclone direction is a hybrid:

- keep the cheap periodic two-dimensional macro skeleton;
- make useful rolling, ridge/valley, basin, and mountain structure available
  across ordinary inland terrain;
- route a small number of continuous landform strengths instead of one
  mountain on/off gate;
- preserve explicit quiet plains as intentional negative space;
- let biome, surface, vegetation, coast, water, and later geology consume the
  same accepted terrain facts; and
- add volumetric density only for selected regional or bounded features after
  the two-dimensional landform skeleton is convincing.

This is not permission to add global roughness. More noise everywhere would
raise the metrics while retaining no geographic explanation. Structure needs
regional organization, transitions, approaches, valleys, and downstream
meaning.

## First Inland Implementation Plan

### Slice 1: explicit landform intent from current fields

Add one compact Mclone-owned landform-intent value near the production terrain
sampler. It should expose continuous strengths plus a diagnostic dominant
family for:

- quiet lowland/plain;
- rolling upland;
- organized ridge-and-valley country;
- basin or broad valley;
- existing mountain/range; and
- reserved later plateau/escarpment specialization.

Reuse continentalness, relief, ruggedness, ridges, and the existing
mountain-detail bands first. Do not add a generic worldgen context, a large
biome parameter table, or another noise field merely to name the result. The
existing per-dimension sampler already retains seed and topology.

The intent is a terrain fact, not a persisted biome ID and not final block
material. Expose it to the production debug sample, field receipt, CPU
preview, and GPU evaluator before changing geometry.

### Slice 2: ordinary inland geometry

Route the current 384/128/48-block relief and ridge signals through the new
strengths:

- rolling country receives connected hills and shallow valleys rather than
  only the current small relief term;
- ridge-and-valley country receives organized shoulders, saddles, and
  approaches without requiring full mountain amplitude;
- broad negative relief can establish basin/valley tendency used later by
  lakes and drainage;
- the 32-block mountain-detail band may contribute modestly outside full
  mountains where a landform family explicitly permits it;
- the 8-block band should remain more selective so ordinary terrain does not
  become uniform noise; and
- existing mountain lift/detail remains one strong family rather than the
  only interesting inland outcome.

Do not begin with a plateau quantizer or global terrace remap. The first
candidate should prove connected ordinary form without recreating the
rejected diagonal/contour artifacts from Tactical 192. A later bounded slice
may add plateaus and escarpments after the rolling/ridge/basin fabric passes
review.

### Slice 3: coast, water, and traversability composition

Inland geometry must remain active as it approaches water. The coast system
should interpret that incoming form:

- a ridge may terminate as a headland;
- a valley may arrive as a cove, outlet, or low depositional reach;
- rolling upland may meet water without a mandatory coast-owned ridge; and
- the existing up-to-22-block rocky coast adjustment should become
  complementary when incoming terrain already supplies sufficient relief.

Accepted rivers and planned streams retain final carving authority. Record
valley tendency now, but do not claim a drainage network or lake spill plan
in this slice. Measure slopes, passes, and low corridors so stronger terrain
does not make every journey an arbitrary wall.

### Slice 4: surface and ecology interpretation

After geometry review, let existing surface and regional recipes consume the
new intent:

- exposed rock and eroded surfaces respond to accepted slope and landform;
- meadow/woodland/conifer/steppe/alpine recipes interpret ridge, basin,
  shelter, altitude, and exposure;
- snow follows climate and altitude across substrates; and
- vegetation density should reveal approaches, saddles, ridgelines, and open
  basins rather than hide every terrain distinction.

Do not introduce several new biome IDs to prove this. Geometry gets its own
review before ecological content can conceal it.

### Slice 5: selective volumetric follow-up

The first inland pass remains a heightfield. Once its regional skeleton is
accepted, compare one bounded formation and one regional density modifier at
selected ridge/escarpment sites. Alpha, Beta, 1.17.1, and modern Java all
demonstrate the value of coarse interpolated 3D density, but universal 3D work
is not the cure for sparse macro intent.

Ordinary point sampling and far summaries must continue to use the cheap
landform plan. Exact 3D realization should be gated by the accepted intent and
bounded vertical envelope.

## Review And Acceptance Plan

### Quantitative alarms

Rerun the exact nine-site corpus. The first geometry candidate is not ready
for subjective review if ordinary inland terrain remains close to the current
baseline. Directional minimum alarms are:

- median vertical span above 24 blocks rather than 16;
- median lag-16 RMS change above 3.0 rather than 1.66;
- median radius-8 detrended roughness above 0.75 rather than 0.30; and
- median radius-32 detrended roughness above 2.0 rather than 0.59.

Those values remain below the Beta medians and are not aesthetic targets.
They merely require a material move out of the current sterile regime.

On equal 6,144-block maps for seeds `12345`, `8675309`, and `-98765`:

- record dominant and continuous family-strength coverage;
- fail if quiet plain still explains more than 70% of dry land on every seed;
- fail if rolling and ridge/valley families disappear on two of three seeds;
- retain nonzero quiet regions rather than filling every cell with maximum
  relief;
- record slope percentiles, extreme faces, connected low corridors, and
  inland-form arrivals at water; and
- distinguish coast-added relief from incoming inland relief.

### Human Review A: undecorated geometry

Stop after production terrain, CPU/GPU preview parity, topology proof, and
first drawable terrain pixels. Present:

- ordinary positive and negative seeds, not only selected mountains;
- quiet plain, rolling country, ridge/valley, basin, and existing mountain
  anchors;
- inland-to-coast transects where landforms terminate in water;
- top-down, walking-height, and elevated views; and
- one journey or sequence that crosses at least three landform regimes.

Review connectedness, broad-stroke masks, repeated contours, navigable
approaches, slope pacing, and whether the world now supplies places between
rare mountains. Do not add new vegetation or geology before this decision.

### Human Review B: regional interpretation

Only after geometry acceptance, present the same anchors with final surface,
snow, trees, and water. Review whether ecology reveals or hides the landform
plan and whether coast geometry now feels inherited from inland.

### Determinism, topology, preview, and performance

- Preserve exact plane and 384-chunk-X cylinder results with a seam-crossing
  example for every live landform family.
- Prove partition/order behavior, persistence reopen, and native/browser
  reconstruction through existing profile contracts.
- Keep CPU preview, GPU preview, Worldgen Lens, field review, and exact chunks
  on one semantic intent.
- Rerun Tactical 258's point, preview, exact, and World Explorer controls.
- State ordinary-path cost separately from any later volumetric hotspot.
- The first pass should reuse existing fields; a material point-sampling
  increase without a new field is an architecture alarm.

## Decision

The next bounded terrain tactical should implement Slices 1-3 and stop at
Human Review A. It should not continue coast-only tuning, add global
roughness, introduce broad 3D density, or begin several ecological and
geological families at once.

Human Review A decides whether to proceed to surface/ecology interpretation,
retune the landform routing, or reject the chosen family vocabulary. Plateaus,
escarpments, major drainage, basin lakes, compound water forms, and selective
3D geology remain explicit follow-ups rather than being hidden inside the
first tune.

## Validation

Completed:

- exact Alpha, Beta, and Java 1.17.1 source inspection;
- focused Java 26.2 `TerrainProvider`, `NoiseRouterData`, and
  `OverworldBiomeBuilder` inspection;
- current production Mclone field and classifier inspection;
- formatter and four focused terrain-characteristics tests;
- default Mclone/1.17.1 terrain-characteristics run under receipt schema 2;
- deterministic 36-center-per-profile land scan;
- nine-site-per-profile, 17-by-17-chunk final runnable survey;
- four RD16 production cards with every source panel and combined card
  visually inspected; and
- repository diff and documentation-link checks.

Focused Clippy with `-D warnings` remains blocked by three unrelated existing
warnings in `mclone-core`: `unnecessary_to_owned`, `should_implement_trait`,
and `derivable_impls`. This research slice did not change those files.

## Related

- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-overworld-breadth.md`](../topics/mclone-overworld-breadth.md)
- [`../topics/modern-minecraft-reference.md`](../topics/modern-minecraft-reference.md)
- [`192-mclone-overworld-mountains-and-valleys.md`](192-mclone-overworld-mountains-and-valleys.md)
- [`258-mclone-macro-terrain-performance-baseline.md`](258-mclone-macro-terrain-performance-baseline.md)
- [`259-modern-and-historical-coast-reference-survey.md`](259-modern-and-historical-coast-reference-survey.md)
- [`260-mclone-coast-intent-and-shore-terrain.md`](260-mclone-coast-intent-and-shore-terrain.md)
