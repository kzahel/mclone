# Tactical 192: Mclone Overworld Mountains And Valleys

Status: Slice 1, the human-requested shorter-traversal tune, Slice 2's
terrain-language response, and the dedicated no-extraction checkpoint landed
2026-07-22. Human Review 1 accepted the revised geometry before surface rules
changed. Production maps and the maximum-view-distance Review 2 matrix now
show open valleys and transitional shoulders, grass below coherent rocky high
ground, and an unchanged wooded lowland control. Human Review 2 rejected the
current geometry as too smooth and too large-scale. A production-backed
terrain-characteristics checkpoint now measures that finding against exact
Minecraft Java 1.17.1 terrain. Field revision 5 applies the first bounded
multiscale response, but human pixel review rejected its strong diagonal
terrace pattern. Field revision 6 replaces that aligned detail with warped
gradient noise and is the current human-review candidate. Host-equivalence
closeout remains before Tactical 196.

Topic: `mclone-overworld-generation`

Workstream: shared native Rust world generation, with production-backed field
maps and desktop pixels before host equivalence.

## Result

Give `mclone-overworld-v1` its first recognizable mountain and valley family:

- coherent mountain regions rather than isolated noisy spikes;
- readable connected crests, shoulders, foothills, and traversable valleys;
- lowlands and coasts that retain the accepted foundation scale;
- altitude/slope-aware use of the existing surface and vegetation vocabulary;
- structured ridge and ruggedness/erosion facts visible through the same
  point/region sampling contract as generation; and
- an explicit reuse/refactor pause before the next content family.

This remains an intentional update to an internal, unshipped profile. It does
not create a new persisted identity unless the safety ledger gains a concrete
preservation consumer before implementation.

## Product Gate

Approve a concise target before changing output, similar to:

> Broad temperate mountain chains rise out of the existing rolling uplands,
> with irregular connected crests, layered shoulders, occasional stone faces,
> and grassy or wooded valleys that remain useful routes through the region.

Approve broad maps and landscape centers that include at least:

- one range interior and one range edge;
- a connected valley between high areas;
- a coast-to-mountain transition;
- an ordinary lowland control region; and
- positive and negative seeds and coordinates.

The review must reject evenly spaced noise cones, unbroken walls, contour
terracing, implausibly sheer grass faces, and a global height increase that
merely relabels the current uplands.

## Fixed Boundaries

- Keep reference `overworld` output byte-exact and leave its density, biome,
  surface, PRNG, and cache composition untouched.
- Keep Small Island's bounded radial terrain output unchanged unless a later
  behavior-preserving shared extraction names both callers and exact evidence.
- Add only structured fields used by the landed terrain formula. The likely
  first vocabulary is one ridge signal and one broad ruggedness or erosion
  control; final names follow the approved rule rather than reserving a DSL.
- Keep macro composition, thresholds, seed domains, mountain shaping, biome
  response, surface response, and vegetation selection Mclone-owned.
- Keep scheduler, worker, persistence, lighting, publication, and host policy
  profile-neutral. The existing dependency plan should remain unchanged unless
  the content rule truly needs new artifacts or footprint.
- Preserve independent stable domains for foundation relief, new ridge shape,
  and new regional control so changing one family need not perturb another.
- Do not add rivers, wetlands, temperature/moisture climate breadth, caves,
  geology, structures, landmarks, overhangs, or a universal 3D density graph.

## Pre-Slice Reuse Inventory

Classify these candidates before implementation:

| Candidate | Expected direction | Evidence boundary |
|---|---|---|
| `SeedDomain`, `ValueNoise2d`, and octave composition | reuse existing primitives | positive/negative point locks and field maps |
| Mclone point/region sampler | extend with only live mountain facts | point/region equality and schema-revision receipt |
| foundation continentalness/relief | compose without changing their raw values | retain existing raw-field fingerprints |
| chunk column writer and heightmaps | reuse as-is | seams, selected columns, and chunk fingerprints |
| Mclone biome/surface/decoration rules | extend concretely in the profile module | distribution and final-payload locks |
| slope/exposure calculations | keep local first; compare after pixels | dedicated checkpoint after two concrete uses exist |
| Small Island height/noise composition | compare, but keep radial policy concrete | existing island cards and fingerprints |
| reference Overworld density interpolation | reference only unless the actual rule shares its mechanics | full oracle locks before any extraction |
| typed plan and Surface dependency cache | reuse unchanged by default | exact plan and warm-cache reports |

## Execution Checklist

### Slice 0: target, fields, and clean baselines

- [x] Approve the visual rule and reject-list above.
- [x] Select broad-map controls from production foundation maps; select exact
  range-interior and range-edge landscape centers from the first production
  ruggedness/ridge map rather than inventing debug-only coordinates.
- [x] Define the minimum live structured fields, seed domains, ranges, and
  composition units.
- [x] Decide whether valleys are a derived relation of ridge/regional fields or
  require one independently meaningful live signal.
- [x] Complete the reuse inventory and safety-ledger audit.
- [x] Capture clean foundation field hashes, selected chunk/final payload
  fingerprints, cards, distribution counts, and release generation cost.

Gate: the desired relief can be evaluated quantitatively and visually before
the generator changes.

Slice 0 selected `ruggedness` as a broad signed regional control and `ridges`
as a normalized connected-crest signal. Valleys are initially derived from
their relation and do not receive a third noise field. Foundation
continentalness and relief retain their raw domains and pinned values. New
field scales must divide the provisional 6,144-block Tactical 196 period or
else force that tactical to revisit its period explicitly.

The clean Linux baseline at commit `8cfa1ac4`, seed `12345`, center `(0,0)`,
radius one, and three release iterations produced 3,279.500 Mclone surface
chunks/s, 600.997 cold decorated target chunks/s, and 4,654.574 warm decorated
target chunks/s. Production 385-by-385 field maps at 16-block stride took
12.392 ms at seed `12345`, origin, and 17.457 ms at seed `-98765`, chunk
`(-96,72)`. Existing deterministic locks retain the clean raw foundation,
selected chunk, terrain-language, order, and partition fingerprints. The
living topic owns the fixed-cost hydrology, performance-regression, and future
stored generation-quality policy.

### Slice 1: first concrete relief caller

- [x] Extend pure point and bounded-region samples through one production
  implementation used by chunk generation and review maps.
- [x] Compose mountains only where continental and regional intent permits;
  preserve oceans, beaches, and the ordinary lowland control.
- [x] Produce connected crests, shoulders, foothills, and valley floors without
  chunk seams or coordinate-sign assumptions.
- [x] Keep height and local slope within the approved first-slice bounds.
- [x] Pin raw foundation fields separately from new fields and derived height.
- [x] Capture the first drawable mountain and valley before biome/surface
  response is broadened.

Gate: the terrain geometry alone reads as a range and valley system.

### Review 1: geometry and scale

- [x] Inspect broad raw/derived maps for repetition, axes, grid artifacts,
  ridge fragmentation, and valley connectivity.
- [x] Inspect the approved landscape matrix for silhouette, traversal, coast
  transition, foundation preservation, and spawn quality.
- [x] Record height percentiles, highland fraction, slope bands, connected
  ridge/valley measures where useful, and generation cost.
- [x] Accept the geometry, tune existing composition, or add at most one field
  whose absence is demonstrated by the review.

Gate: obtain human review before extracting helpers or changing content rules.

Field revision 3 used ruggedness scales 1,536 and 512 plus ridge-source scales
768 and 256. Human Review 1 found the slopes coherent but too gradual and slow
to traverse. Field revision 4 retains the broad ruggedness region, halves only
the ridge scales to 384 and 128, and narrows the lift response from a `0.12`
to a `0.22` ridge threshold. All scales still divide the planned 6,144-block
periodic circumference. Ocean-floor composition is unchanged, and the
dependency plan remains the existing 3-by-3 work/5-by-5 Surface footprint.

The 385-by-385, 16-block-stride production maps reported:

| Seed and center | Surface range | Mountain region | Valleys | Crests | Y>=105 | Y>=135 |
|---|---:|---:|---:|---:|---:|---:|
| `12345`, chunk `(0,0)` | 45-147 | 5,481 | 1,361 | 1,166 | 585 | 52 |
| `-98765`, chunk `(-96,72)` | 47-153 | 12,904 | 2,857 | 3,066 | 2,874 | 525 |
| `8675309`, chunk `(128,-96)` | 45-91 | 0 | 0 | 0 | 0 | 0 |

The revised matrix covers seed `12345` range interior `(-133,-66)` and range
edge `(-142,-51)`, seed `-98765` range interior `(-186,25)` and valley
`(-204,22)`, plus seed `8675309` lowland control `(122,-96)`. All cards use the
current maximum render distance 16 at 800 by 500 pixels per panel. Each receipt
reports 1,225/1,225 target chunks ready and at least 24 blocks of clearance
above actual terrain/canopy at every eye. The landscape view can no longer
spawn underground merely because terrain at the displaced eye is higher than
terrain at the review center.

Internal review found no isolated cone field, unbroken wall, coordinate-axis
discontinuity, or global uplift. Connected crests, multiple neighboring peaks,
and valley-to-crest changes now fit inside the 256-block loaded radius. At
16-block review stride, edges changing by at least three blocks increased from
2,092 to 5,878 for seed `12345` and from 8,476 to 12,642 for seed `-98765`;
the lowland control moved only from 1,821 to 1,845. This is the intended
shorter traversal signal rather than a global roughness increase. The
height-only wooded-upland rule still blankets some shoulders, which remains a
terrain-language finding rather than a reason for a third geometry field.

On the same Linux host and command as Slice 0, revision 4 measured 3,029.299
Mclone surface chunks/s (7.6 percent below the six-lattice-field foundation),
628.933 cold decorated targets/s, and 4,217.298 warm decorated targets/s. The
three 148,225-point maps sampled in 14.801-18.493 ms. The tune changes no field
count and is inside the 25 percent review threshold; this remains the
attribution baseline for periodic fields and rivers.

### Slice 2: terrain-language response

- [x] Make existing biome choice react to the landed altitude/exposure facts
  without adding unused climate dimensions or custom registry content.
- [x] Make grass/soil, exposed stone, and vegetation eligibility respond to
  altitude and slope through Mclone-owned rules.
- [x] Preserve readable valley routes and avoid trees or grass on clearly
  unsuitable faces.
- [x] Re-prove safe spawn and decoration order/partition independence.
- [x] Pin biome/surface distributions and final decorated payloads.

Gate: blocks and vegetation reinforce the relief instead of hiding it.

Slice 2 adds `McloneOverworldLandformSample`, which keeps the accepted raw
terrain sample intact and adds one derived slope in blocks risen per horizontal
block. The slope is a central difference over a two-block radius: east minus
west and south minus north are each divided by the four-block sample diameter,
then combined as a two-dimensional gradient magnitude. Production chunk fill
samples a 20-by-20 terrain halo once and derives all 256 column gradients from
those 400 samples. A naive five-point query per column would require 1,280
samples. The bounded halo is internal to the chunk and does not enlarge the
existing 3-by-3 feature work or 5-by-5 Surface dependency footprints.

Exposure is a bounded relation of accepted facts rather than another noise
field. It combines altitude from Y=82 through Y=130 with ridge strength above
`0.35`, then gates that result by mountain-region strength. The landed rules
are:

| Response | Rule |
|---|---|
| Wooded upland | Y>=75, slope `<0.45`, exposure `<0.44`, and not a mountain valley or open shoulder |
| Mountain valley | dry land, mountain strength `>=0.35`, and ridge `<=0.28` |
| Open shoulder | dry land, mountain strength `>=0.15`, and ridge `>=0.65` |
| Exposed stone | Y>=80 and either slope `>=0.80` or exposure `>=0.76` |
| Grass/soil | remaining dry land |

Open regions reuse the existing sparse oak/grass/flower table and wooded
regions reuse the existing denser oak table. Stone cannot satisfy the shared
grass-substrate placement gates, so trees, grass, and flowers do not need a
second terrain-specific exclusion path. No biome registry content or climate
field was added. Raw field revision 4 is unchanged; decoration revision 3
records the intentional biome/table selection change.

The first drawable threshold attempt exposed 154,917 of 263,169 block-scale
columns around the range interior and read as one bare stone bowl. The accepted
tune exposes 81,475 columns, or 31.0 percent, at that deliberately mountainous
site. It preserves continuous faces and crests while returning the lower
shoulders and valley floor to grass. The final 385-by-385 production maps at
16-block stride report:

| Seed and center | Open land | Wooded upland | Exposed stone |
|---|---:|---:|---:|
| `12345`, chunk `(0,0)` | 32,508 | 28,479 | 210 |
| `-98765`, chunk `(-96,72)` | 28,710 | 62,844 | 857 |
| `8675309`, chunk `(128,-96)` | 26,982 | 23,487 | 0 |

The final card matrix is under `/tmp/mclone-overworld-language-v3/` at range
interior `(-186,25)`, mountain valley `(-204,22)`, range edge `(-142,-51)`,
positive-seed range `(-133,-66)`, and lowland control `(122,-96)`. Every card
uses the supported maximum render distance 16 and 800-by-500 panels. Every
receipt proves 1,225/1,225 target chunks ready and 24, 128, and 315 blocks of
camera clearance. The positive-seed card retained 69 extra background chunks;
capture acceptance now correctly keys on the exact target set while recording
unrelated background work separately.

On the same Linux release command used by the earlier slices, the landed path
measured 3,730.502 surface chunks/s, 596.967 cold decorated targets/s, and
5,267.459 warm decorated targets/s. Cold decorated throughput is 5.1 percent
below field revision 4's measurement and effectively equal to the 600.997
foundation baseline, well inside the 25 percent review threshold. Raw
148,225-point map sampling took 14.091-27.970 ms under variable host load; the
separate exact five-point landform diagnostic took 68.626-69.853 ms. Production
generation uses the 400-sample chunk halo rather than that diagnostic path.

`cargo test -p mclone-worldgen --lib` passes 270 tests with the existing one
ignored gauntlet. The locks cover broad language distributions, selected
surface chunks, reviewed final decorated mountain payloads, safe spawn,
neighbor partition independence, cache reuse, Small Island, and reference
Overworld fixtures. Six native-client showcase tests also pass.

### Slice 3: dedicated reuse/refactor checkpoint

- [x] Compare the working mountain path with foundation relief, Small Island,
  and reference Overworld mechanisms.
- [x] Extract only mechanisms with two concrete callers or an already frozen
  data boundary.
- [x] Keep rule composition, domains, thresholds, and tables profile-owned.
- [x] Land behavior-preserving extraction separately from any aesthetic tune.
- [x] Re-run exact reference locks, Small Island locks/cards, Mclone maps/cards,
  typed-plan facts, cache reports, and performance after each extraction.
- [x] Explicitly record rejected abstractions and why they obscure policy or
  add cost.

Gate: the next river slice will not copy a proven mechanism, and no generic
terrain framework exists without real callers.

The checkpoint keeps the only new shared value inside the concrete Mclone
profile: biome and surface policy both consume the same
`McloneOverworldLandformSample`, while review tooling observes it. The
20-by-20 cache remains a private Mclone chunk-fill optimization because Small
Island has no slope caller and reference Overworld owns density interpolation
and biome surface builders with different semantics. No reusable extraction
was justified, so there is no separate behavior-preserving code commit.

Rejected abstractions are a generic slope/noise trait, a cross-profile
exposure formula, a universal surface-rule DSL, and a shared mountain-biome
classifier. Each would either have only one profile family, move thresholds
out of their owner, or make reference output riskier without removing a second
implementation. The existing column-biome payload traversal, placed-feature
executor, typed plan, and Surface dependency cache remain the proven reuse
boundaries.

### Review 2 and closeout

- [x] Re-run all production field maps and the complete landscape matrix.
- [x] Review mountain frequency, silhouette variety, valley connectivity,
  surface exposure, vegetation, coast readability, repetition, and cost.
- [x] Measure the rejected smoothness/scale finding against undecorated
  Minecraft Java 1.17.1 terrain at shared block resolution.
- [x] Rebalance fine, middle, and broad terrain scales against the measured
  reference envelope, then repeat maps and landscape review.
- [x] Reject field revision 5 from the complete high-view-distance landscape
  matrix because of its coherent diagonal terrace artifact.
- [x] Replace aligned value-noise detail with directionally varied,
  periodic-ready detail and repeat characteristic and pixel review.
- [ ] Prove native thread and production browser Worker equivalence.
- [ ] Re-run SQLite/IndexedDB only if identity, persistence, or startup behavior
  changed; otherwise cite Tactical 188's unchanged host contract.
- [ ] Update the topic, safety ledger, worldgen status, tactical execution
  record, accepted defects, and next content decision.

Gate: the range/valley family is accepted and Tactical 196 can freeze the live
field inventory before rivers/wetlands add another field family. If geometry
still needs a bounded tune, record that exception explicitly.

Human Review 2 found the range coherent and the terrain-language response
useful, but found the mountain surface conspicuously smooth and its relief too
large-scale. The first review metrics counted coarse 16-block map edges and
could prove shorter ridge traversal, but could not distinguish a smooth ramp
from a detailed slope. No generator output changed in response to this review.

The measurement checkpoint adds
`pnpm native:worldgen:terrain-characteristics`. Its default suite generates
17 by 17 chunks per site on a shared one-block grid, extracts the highest
non-fluid solid block, and keeps land at Y>=67. The vanilla side invokes the
exact native Minecraft Java 1.17.1 `NoiseBasedChunkGenerator` terrain fill;
the Mclone side invokes the production `mclone-overworld-v1` surface generator.
Neither side includes carvers or decoration. Surface material does not affect
the extracted height. Three established vanilla mountain anchors and three
reviewed Mclone mountain centers are summarized by per-characteristic medians;
one lowland control per profile remains a diagnostic rather than a
representative biome survey.

The reusable height-raster analyzer records:

- RMS and percentile height differences at 1, 2, 4, 8, 16, 32, 64, and 128
  block separations, plus directional anisotropy;
- the absolute five-point discrete Laplacian as a block-scale curvature
  signal;
- least-squares local plane-fit residual RMSE at radii 2, 4, 8, 16, and 32,
  which removes the broad local slope before measuring surface structure;
- local gradient structure-tensor coherence and diagonal alignment over the
  same window radii;
- a log/log roughness exponent over 1-32 blocks, where a value nearer one
  describes smooth ramp-like scale growth; and
- the share of radius-32 residual energy already present at radius 4.

The initial mountain-group result is:

| Characteristic | Vanilla median | Mclone median | Mclone / vanilla |
|---|---:|---:|---:|
| Lag-1 RMS height delta | 1.282 | 0.523 | 0.408x |
| Lag-4 RMS height delta | 3.602 | 1.520 | 0.422x |
| Lag-16 RMS height delta | 8.022 | 5.889 | 0.734x |
| Lag-64 RMS height delta | 12.977 | 20.819 | 1.604x |
| Curvature RMS | 2.379 | 1.128 | 0.474x |
| Plane-fit radius-4 RMSE | 1.365 | 0.279 | 0.205x |
| Plane-fit radius-8 RMSE | 2.279 | 0.361 | 0.158x |
| Plane-fit radius-16 RMSE | 3.904 | 0.859 | 0.220x |
| Plane-fit radius-32 RMSE | 5.569 | 2.692 | 0.483x |
| Roughness exponent | 0.659 | 0.919 | 1.394x |
| Fine-detail energy share, r4/r32 | 0.060 | 0.011 | 0.183x |

This is a strong quantitative match for the visual finding: Mclone has much
less fine- and middle-scale structure after the broad slope is removed, while
its 64-block height change is larger. The next tune should therefore not raise
mountain amplitude globally. It should add or strengthen coherent detail in
roughly the 4-32-block band and reduce or reshape the current broad lift until
the scale curve moves toward the vanilla envelope. The reference remains a
scale/legibility guide rather than an output-parity target.

The default release run generated each 272 by 272 site in roughly 91-258 ms
and analyzed it in 14-27 ms on the current Linux host. Analysis uses summed-area
tables for constant-time plane windows and is offline review tooling, so it
adds no chunk-generation work. The JSON receipt is written to
`/tmp/mclone-terrain-characteristics.json`; custom sites, radius, land mask,
and output path are CLI arguments.

#### Field revision 5 rejected tuning candidate

Field revision 5 keeps continentalness, relief, ruggedness, and both ridge
domains byte-identical. It adds one live `mountain_detail` value composed from
independent 32- and 8-block `ValueNoise2d` domains at weights `0.55` and
`0.45`. Both scales divide Tactical 196's provisional 6,144-block period. The
detail enters height only through:

```text
mountain_detail * mountain_strength * (6 + 14 * ridge_shoulder)
```

This gives strong crests and shoulders more secondary form while retaining a
smaller response in traversable mountain valleys. Oceans and non-mountain
lowlands multiply the detail by zero. A direct unit test locks that boundary,
and the lowland characteristic control retains its exact previous fingerprint
and every measured value.

The same revision reduces the broad mountain lift from
`4 + 16s + 54s^2` to `4 + 12s + 38s^2`, where `s` is the accepted smoothed
ridge shoulder. This is a reallocation of height variation from broad lift to
coherent local form, not a global amplitude increase. The existing 20-by-20
landform halo and 3-by-3 work/5-by-5 Surface dependency footprints are
unchanged. Production and review use the same sampler. Receipt schema 5 adds
the detail range, map, and fingerprint contribution.

The measurement suite reports:

| Characteristic | Field rev. 4 | Field rev. 5 | Vanilla | Rev. 5 / vanilla |
|---|---:|---:|---:|---:|
| Lag-1 RMS height delta | 0.523 | 0.726 | 1.282 | 0.567x |
| Lag-4 RMS height delta | 1.520 | 2.351 | 3.602 | 0.653x |
| Lag-16 RMS height delta | 5.889 | 6.344 | 8.022 | 0.791x |
| Lag-64 RMS height delta | 20.819 | 15.495 | 12.977 | 1.194x |
| Curvature RMS | 1.128 | 1.259 | 2.379 | 0.529x |
| Plane-fit radius-4 RMSE | 0.279 | 0.839 | 1.365 | 0.615x |
| Plane-fit radius-8 RMSE | 0.361 | 1.619 | 2.279 | 0.711x |
| Plane-fit radius-16 RMSE | 0.859 | 2.441 | 3.904 | 0.625x |
| Plane-fit radius-32 RMSE | 2.692 | 4.341 | 5.569 | 0.779x |
| Roughness exponent | 0.919 | 0.748 | 0.659 | 1.135x |
| Fine-detail energy share | 0.011 | 0.063 | 0.060 | 1.055x |

The tune closes the demonstrated gap without fitting every vanilla number.
The remaining lower block curvature and lag-1 change preserve some original
Mclone smoothness, while middle-scale residuals and broad change now sit much
closer to the reference envelope. The ordinary lowland stays intentionally
calmer than vanilla rather than receiving global detail.

The final review matrix is under
`/tmp/mclone-overworld-fields5-candidate2-{range-negative,valley,positive,range-edge,lowland}`.
Every card uses render distance 16 and 800-by-500 source panels. The inspected
range interior, mountain valley, positive-seed range, range edge, and lowland
control retain 24, 128, and 315 blocks of camera clearance in their three
views. The range cards show secondary peaks, saddles, shelves, and gullies;
the valley remains a connected broad route, and the wooded lowland control is
visually and numerically unchanged. Human review nevertheless rejected the
candidate because its top-down stone surfaces expose a strong diagonal
herringbone pattern.

On the same three-iteration release lane used by prior slices, field revision
5 measured 3,836.042 surface chunks/s, 628.990 cold decorated targets/s, and
4,891.668 warm decorated targets/s. Relative to the field-revision-4 language
baseline, those are +2.8, +5.4, and -7.1 percent respectively and remain well
inside the 25 percent review threshold. The two added lattice samples are not
an observable generation-cost concern in this run. Broad 148,225-point maps
sampled in 16.7 ms and exact landforms in about 80-82 ms; the latter remains a
diagnostic path rather than production's cached halo.

The herringbone is real generated geometry, not the separate aliasing seen
when an 8-block field is displayed on the 16-block diagnostic grid. Both
detail bands use rectangular `ValueNoise2d` lattices at harmonic 32- and
8-block scales. Each cell interpolates corner values independently along X and
Z, creating locally smooth, nearly planar patches. Multiplying that result by
up to 20 blocks and rounding to integer height turns the patches into parallel
diagonal contour steps. Neighboring cells reverse gradient direction, so the
steps form visible chevrons. The existing global X/Z anisotropy measure stays
near one because opposing diagonal orientations cancel across a site.

The analyzer now reports local structure-tensor coherence and diagonal
alignment at every plane-fit radius. This closes the global-cancellation blind
spot in the report shape, but the first result is also a useful warning against
turning one metric into an acceptance oracle: revision 5's mean radius-4 and
radius-8 diagonal alignment is `0.280` and `0.188`, below the vanilla medians
of `0.363` and `0.279`, despite the obvious rejected pattern. Those aggregates
do not measure contour straightness or lattice repetition directly. They
remain supporting evidence; an inspected maximum-view-distance card can veto
a numerically plausible candidate.

#### Field revision 6 review candidate

Field revision 6 adds a deterministic two-dimensional `GradientNoise2d` for
original Mclone content. Lattice hashes select among 16 evenly distributed
gradient directions, corner dot products use quintic interpolation, and the
public periodic constructor wraps lattice identity at a block period divisible
by its scale. Tests cover signed/fractional coordinates and exact repetition at
the provisional 6,144-block period. This is periodic-ready infrastructure;
Tactical 196 still owns conversion of every live field and canonical seam
sampling.

The live terrain keeps the same independent 32- and 8-block mountain-detail
domains but changes their implementation and weights. The 32-block band now
contributes `0.70` and the 8-block band `0.30`. Their coordinates are bent by
already-sampled relief and ruggedness components:

```text
large x += 18 * relief_detail + 4 * relief_fine
large z += 18 * ruggedness_detail - 4 * relief_fine
fine  x += -7 * ruggedness_detail + 3 * relief_detail
fine  z +=  7 * relief_fine + 3 * relief_detail
```

The two different gentle warps break repeated lattice organization without a
new macro semantic field or extra warp-field samples. `mountain_detail`, its
mountain/shoulder amplitude gate, the reduced broad lift, exact ocean/lowland
exclusion, and all chunk dependency footprints are unchanged. A first
`0.55/0.45` gradient candidate removed the herringbone but produced excessive
uniform bustle: fine-detail energy share rose to `0.119` against vanilla's
`0.060`. The selected review weighting corrects that before pixel closeout.

The final candidate measures:

| Characteristic | Field rev. 6 | Vanilla | Rev. 6 / vanilla |
|---|---:|---:|---:|
| Lag-1 RMS height delta | 0.664 | 1.282 | 0.518x |
| Lag-4 RMS height delta | 2.007 | 3.602 | 0.557x |
| Lag-16 RMS height delta | 5.887 | 8.022 | 0.734x |
| Lag-64 RMS height delta | 15.469 | 12.977 | 1.192x |
| Curvature RMS | 1.273 | 2.379 | 0.535x |
| Plane-fit radius-4 RMSE | 0.728 | 1.365 | 0.534x |
| Plane-fit radius-8 RMSE | 1.193 | 2.279 | 0.523x |
| Plane-fit radius-16 RMSE | 2.059 | 3.904 | 0.527x |
| Plane-fit radius-32 RMSE | 3.834 | 5.569 | 0.688x |
| Roughness exponent | 0.768 | 0.659 | 1.165x |
| Fine-detail energy share | 0.055 | 0.060 | 0.915x |

The full inspected matrix is under
`/tmp/mclone-overworld-fields6-candidate2-{range-negative,valley,range-positive,range-edge,lowland}`.
Every card uses render distance 16 and 800-by-500 source panels. The repeated
chevrons are absent across both seeds; mountain interiors have varied local
peaks, the valley remains open, the range edge has no wall, and the lowland
control is visually and numerically unchanged. This is ready for human review,
not yet human acceptance.

On the same three-iteration release lane, revision 6 measured 3,127.978
surface chunks/s, 515.157 cold decorated targets/s, and 4,693.731 warm
decorated targets/s. Those are 18.5, 18.1, and 4.0 percent below revision 5,
respectively, and remain inside the existing 25 percent review threshold.
The shared worldgen/server/app-runtime suites and browser WASM build pass;
native-thread versus production browser-Worker equivalence remains open.

This remains a two-dimensional heightfield correction. Gradient direction and
coordinate warping removed the demonstrated planar lattice signature;
volumetric density is reserved for a later demonstrated need for overhangs,
arches, or undercut cliffs. The new noise must retain stable seed domains,
signed-coordinate continuity, scales compatible with the planned 6,144-block
period, and no larger chunk dependency footprint.

## Evidence

Extend `pnpm native:worldgen:fields` rather than creating a debug-only terrain
formula. Receipts should add:

- field/composition revision and new domain identifiers;
- raw new-field ranges and fingerprints;
- foundation raw-field fingerprints proving independent-domain preservation;
- derived height, highland/valley/slope distributions, and language counts;
- exact seed, center, dimensions, stride, commit/dirty state, and elapsed time.

Use the existing fully warmed card tool for landscape review. Disposable maps,
cards, and host screenshots remain under `/tmp`; committed numeric locks are
regression evidence, not release compatibility promises.

Minimum closeout validation remains the Tactical 188 matrix: worldgen, server,
app runtime, native client, WASM compile, web typecheck, production browser
Worker, desktop pixels, and clean release generation cost. Any shared change
also re-runs the Small Island card and exact reference Overworld locks.

## Follow-Up Boundary

Tactical 196 is the required technical interleave after accepted mountains and
valleys: it makes every live field periodic before another field family lands.
Rivers and wetlands are the preferred next visual family after that contract
because they can consume the newly reviewed macro relief. Streams, cascades,
and waterfalls follow coherent river direction and grade facts rather than
appearing as isolated decorations. Caves and structures remain later. If
mountain geometry demonstrates that a columnar heightfield is insufficient,
record the exact overhang/cliff requirement and open a separate 3D-density
tactical rather than growing this one implicitly.

## Related

- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`188-mclone-overworld-v1-terrain-foundation.md`](188-mclone-overworld-v1-terrain-foundation.md)
- [`191-guarded-generation-planning-refactor.md`](191-guarded-generation-planning-refactor.md)
- [`../worldgen-status.md`](../worldgen-status.md)
