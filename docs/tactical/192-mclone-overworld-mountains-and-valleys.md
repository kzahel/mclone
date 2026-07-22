# Tactical 192: Mclone Overworld Mountains And Valleys

Status: Slice 1 and a human-requested shorter-traversal tune landed 2026-07-22;
the renewed Review 1 is ready for human judgment. Production maps and the
maximum-view-distance geometry matrix show shorter connected mountain crests,
traversable valleys, bounded range transitions, and an unchanged lowland
control. Every camera eye is proven above loaded terrain and canopy. Existing
height-only forest selection still blankets some relief; surface and
vegetation response remains intentionally unimplemented until this geometry
review is accepted.

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
- [ ] Accept the geometry, tune existing composition, or add at most one field
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

- [ ] Make existing biome choice react to the landed altitude/exposure facts
  without adding unused climate dimensions or custom registry content.
- [ ] Make grass/soil, exposed stone, and vegetation eligibility respond to
  altitude and slope through Mclone-owned rules.
- [ ] Preserve readable valley routes and avoid trees or grass on clearly
  unsuitable faces.
- [ ] Re-prove safe spawn and decoration order/partition independence.
- [ ] Pin biome/surface distributions and final decorated payloads.

Gate: blocks and vegetation reinforce the relief instead of hiding it.

### Slice 3: dedicated reuse/refactor checkpoint

- [ ] Compare the working mountain path with foundation relief, Small Island,
  and reference Overworld mechanisms.
- [ ] Extract only mechanisms with two concrete callers or an already frozen
  data boundary.
- [ ] Keep rule composition, domains, thresholds, and tables profile-owned.
- [ ] Land behavior-preserving extraction separately from any aesthetic tune.
- [ ] Re-run exact reference locks, Small Island locks/cards, Mclone maps/cards,
  typed-plan facts, cache reports, and performance after each extraction.
- [ ] Explicitly record rejected abstractions and why they obscure policy or
  add cost.

Gate: the next river slice will not copy a proven mechanism, and no generic
terrain framework exists without real callers.

### Review 2 and closeout

- [ ] Re-run all production field maps and the complete landscape matrix.
- [ ] Review mountain frequency, silhouette variety, valley connectivity,
  surface exposure, vegetation, coast readability, repetition, and cost.
- [ ] Prove native thread and production browser Worker equivalence.
- [ ] Re-run SQLite/IndexedDB only if identity, persistence, or startup behavior
  changed; otherwise cite Tactical 188's unchanged host contract.
- [ ] Update the topic, safety ledger, worldgen status, tactical execution
  record, accepted defects, and next content decision.

Gate: the range/valley family is accepted and Tactical 196 can freeze the live
field inventory before rivers/wetlands add another field family. If geometry
still needs a bounded tune, record that exception explicitly.

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
