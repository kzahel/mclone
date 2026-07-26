# LOD Surface Appearance Quality

Status: complete (2026-07-26).

Topic: `vanilla-terrain-lod`

## Objective

Make procedural terrain previews communicate more of the visible surface
language without quietly turning the fast product back into chunk or dense
footprint generation.

The first slice adds two explicit surface-appearance qualities:

- **Basic** keeps the existing per-point biome top-material approximation and
  adds only biome grass tint that can be derived from the biome already
  selected for the point.
- **Inferred** adds one builder-aware two-dimensional surface-noise evaluation
  per point. It may select stone, gravel, coarse dirt, or podzol for the
  vanilla mountain, gravelly-mountain, giant-tree-taiga, and
  shattered-savanna builders. The renderer also marks steep mountain faces
  from derivatives of the geometry it is already drawing; this adds no
  worldgen sample.

No quality in this tactical adds neighboring density columns, extra vertical
density probes, child-tile roll-ups, or footprint taps.

## Originating Direction

Human review found that kilometer-scale LOD terrain reads mostly as grass and
sand even where the canonical reference contains exposed stone, gravel, dirt,
or distinct grass colors. The desired direction is richer large-scale
legibility with a permanently available cheap option and explicit control over
any later sampling cost.

Start with facts already available at each requested point. Add biome tint
when its marginal cost is negligible. Try an exposure/material classifier only
when its cost and accuracy can be measured. Do not begin with many footprint
samples; defer them until the cheap result has been reviewed.

## Reference Findings

Minecraft Java 1.17.1 does not select mountain stone from terrain slope.
`NoiseBasedChunkGenerator.buildSurfaceAndBedrock` evaluates its seeded
four-octave surface-noise field once per column and passes that value to the
biome's surface builder.

The relevant builder decisions are:

- `MountainSurfaceBuilder`: stone above noise `1.0`, otherwise grass;
- `GravellyMountainSurfaceBuilder`: gravel below `-1.0` or above `2.0`,
  stone above `1.0`, otherwise grass;
- `GiantTreeTaigaSurfaceBuilder`: coarse dirt above `1.75`, podzol above
  `-0.95`, otherwise grass; and
- `ShatteredSavanaSurfaceBuilder`: stone above `1.75`, coarse dirt above
  `-0.5`, otherwise grass.

The Rust production generator already ports the same surface-noise
initialization and these surface-builder thresholds. The preview can reuse
them without translating a second algorithm.

Java grass color starts from the biome's temperature/downfall colormap result,
then applies optional dark-forest or swamp modifiers. The live canonical mesh
path already contains the reviewed 1.17.1 biome visual matrix. The first LOD
slice may use its per-biome grass results without a new biome lookup or
five-by-five biome blend. Exact boundary blending and swamp coordinate
variation remain separately measurable refinements.

## Quality Contract

Surface appearance quality is independent from terrain-source fidelity and
spatial resolution:

| Control | Meaning |
|---|---|
| `Fast macro` / `Sampled exact` | vertical density fidelity |
| `Resolution` | horizontal sample spacing and sample count |
| `Basic` / `Inferred` surface | material classification at each retained point |

`Basic` must remain a real selectable product, not a label for the inferred
result. `Inferred` is the preferred default only if measurements show that one
surface-noise lookup is small relative to both fast-macro and sampled-exact
density work and visual comparison shows a useful improvement.

The quality value belongs in preview requests, viewport tile identity, Worker
messages, cache identity, URL state, and diagnostics. Changing it must not
publish a tile compiled for the previous quality.

## Shared Appearance Contract

`mclone-worldgen` owns profile-specific material classification.
`mclone-terrain-view` owns the shared material palette and selection of the
basic or inferred material carried by a sample. Terrain Lab exposes the
quality control and evidence; it does not own classification thresholds.

The same palette must recognize at least:

- grass, water, stone, dirt, sand, gravel, snow, and coarse dirt;
- podzol, mycelium, and red sand when emitted by the vanilla surface language;
  and
- the existing Mclone Overworld macro materials.

Vanilla grass uses reference-shaped biome tint. Mclone grass retains its
production temperature/moisture tint. The shared renderer should not make all
profiles use one climate model.

## Performance Boundary

For this tactical, one output point performs:

```text
Basic:
  existing terrain sample + existing biome selection

Inferred:
  Basic + one 2D four-octave surface-noise lookup

Inferred renderer:
  existing triangle geometry + one derivative-based steep-face decision
```

The following remain deferred:

- additional height or density samples around a point;
- a slope classifier that resamples terrain;
- material coverage or dominant/secondary material footprint taps;
- conservative parent summaries or fine-child roll-ups;
- biome tint blending that performs more biome-source queries; and
- complete surface-builder column mutation.

A future sampled quality may add bounded taps, but it requires its own work
accounting and mobile evidence. It must not alter what `Basic` or `Inferred`
mean.

## Result

The landed contract is:

- `Basic` remains permanently selectable and performs no surface-noise
  lookup;
- `Inferred` is the default after measurement;
- both qualities use reference-shaped per-biome grass color without another
  biome-source query;
- Inferred reuses the production surface-noise field and shared builder
  thresholds; and
- Inferred additionally colors already-rendered steep mountain faces as
  stone. The face classifier uses shader derivatives only and does not change
  CPU/Worker sampling.

Seven production surface fixtures cover mountains, mountain relief, gravelly
mountains, gravelly relief, giant tree taiga, shattered savanna, and shattered
savanna plateau. Across their 1,668 non-water columns, Basic matched 1,309
visible top materials. Inferred corrected the other 359 and matched all 1,668.
This verifies the point classifier on the selected reference-shaped builders;
it does not claim complete surface-builder or side-wall parity.

Five optimized development-profile runs were recorded for each fixed
65-by-65, 2,048-block grid. Median cold grid compilation was:

| Seed and center | Basic exact | Inferred exact | Basic macro | Inferred macro |
|---|---:|---:|---:|---:|
| `12345`, `0,0` | 172.72 ms | 171.05 ms | 48.08 ms | 48.79 ms |
| `-98765`, `-304,336` | 273.14 ms | 283.20 ms | 75.55 ms | 76.60 ms |

The macro delta was 1.4-1.5%. Exact ranged from run noise on the first fixture
to 3.7% on the second. Inferred changed 338 and 62 non-water lattice points,
respectively. No height, density, biome-neighbor, or footprint work changed.

Headed Chrome passed on desktop and the Pixel 7 profile at 2 km. The inspected
Basic/Inferred captures show biome grass variation in both modes and coherent
stone, coarse-dirt, gravel, and podzol regions only in Inferred. A separate
64-block seed-33 capture compared 25 canonical surface chunks with exact and
macro LOD; the derivative classifier changed steep heightfield faces from
grass to the broad stone-cliff read visible in the canonical pane.

Evidence captures:

- `/tmp/mclone-terrain-lab-desktop-chrome-vanilla-basic.png`
- `/tmp/mclone-terrain-lab-desktop-chrome-vanilla-inferred.png`
- `/tmp/mclone-terrain-lab-desktop-chrome-vanilla-mountain-close.png`
- the corresponding `phone-chrome` captures under `/tmp`

## Implementation Receipt

- `8f772fff` records the quality and performance boundary.
- `a6fa839d` implements shared classification, rendering, request identity,
  Worker/Wasm routing, UI state, measurements, and browser evidence.

## Acceptance

- Terrain Lab exposes durable `Basic` and `Inferred` surface qualities and
  round-trips the selection through its URL.
- Basic vanilla output retains biome-top material classification while gaining
  biome grass tint without additional biome-source queries.
- Inferred vanilla output reuses the production surface-noise field and
  reference thresholds for stone, gravel, coarse dirt, and podzol.
- Basic and inferred tile/cache/Worker identities cannot alias.
- The shared palette presents all emitted material ids deliberately.
- Fixed native measurements record basic-versus-inferred cost for fast macro
  and sampled exact at the existing 2 km fixtures.
- Canonical-versus-LOD captures show whether the inferred material regions are
  directionally useful; the result is not described as exact surface parity.
- Mclone CPU/GPU preview behavior remains coherent and both profiles build
  through the shared renderer.
- Focused Rust, TypeScript, Wasm, production-build, and headed desktop/phone
  WebGPU validation pass before closure.

## Implementation Order

1. Record this contract in the durable vanilla terrain LOD topic.
2. Add revisioned surface-quality request and cache identity.
3. Expose production surface noise and the builder-aware top-material
   classifier with threshold tests.
4. Add the shared material palette and reference-shaped grass tint.
5. Add Terrain Lab URL/UI/Worker routing and truthful diagnostics.
6. Measure both quality modes and compare rendered output with canonical
   terrain.
7. Select the default from evidence and record remaining sampling experiments.

Commit each coherent slice with `Topic: vanilla-terrain-lod`.
