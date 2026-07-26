# Vanilla Terrain LOD

Topic: `vanilla-terrain-lod`

Status: sampled-exact and fast-macro products implemented and validated on
2026-07-25. Tactical
[`240`](../tactical/240-vanilla-terrain-lod-in-terrain-lab.md) records the
first direct sampler; Tactical
[`246`](../tactical/246-vanilla-fast-macro-terrain-preview.md) records the
exact-path optimization, accepted macro algorithm, comparison evidence, and
independent browser products. Tactical
[`251`](../tactical/251-lod-surface-appearance-quality.md) is the active
bounded surface-appearance quality slice.

## Scope

This topic owns a direct, presentation-only Minecraft Java 1.17.1 Overworld
terrain sampler and its first product use in Terrain Lab.

The sampler exists to review recognizable vanilla macro geography without
generating complete chunks and discarding almost all of their block data. It
does not replace canonical vanilla generation or Mclone's independent
procedural CPU/GPU preview graph.

## Product Decision

Terrain Lab has one global terrain profile:

- `mclone-overworld-v1` selects exact Mclone terrain, Mclone CPU LOD, and the
  optional Mclone GPU LOD;
- `overworld` selects exact Java 1.17.1 reference terrain, `Sampled exact`,
  and `Fast macro`; and
- the two vanilla procedural products can be shown independently or together,
  while Mclone GPU LOD remains unavailable for `overworld`.

One workspace never mixes exact terrain from one profile with LOD from the
other. Seed, center, footprint, camera, cache identity, Worker epoch,
diagnostics, inspector receipts, and URL state all carry the selected profile.

The browser may display and transport the selection. Shared Rust owns profile
semantics, supported stages/layers, sampling, and canonical generation.

## Current Two-Fidelity Contract

`Sampled exact` is revisioned
`vanilla-1.17.1-density-column-lod-v2`. It preserves the direct Java 1.17.1
density result at each requested point and generates only columns with
non-zero horizontal interpolation weight: one at an aligned density-lattice
corner, two on a single-axis interior position, or four at an arbitrary
interior block.

`Fast macro` is revisioned
`vanilla-1.17.1-sparse-density-column-lod-v2`. For each retained horizontal
column it uses one center noise biome and nine ordinary vanilla blended-noise
nodes spaced four vertical cells apart. It interpolates across the resulting
32-block intervals and retains vanilla water fill, biome lookup, and
approximate visible-material semantics.

The macro sampler is deliberately presentation-only. Fixed 2 km native
fixtures show a 3.45-3.87x cold compile speedup over the optimized exact path,
94.18-96.59% water agreement, 3.63-4.92-block mean solid error, and 11-16-block
P95 solid error. The headed Pixel 7 viewport reached macro target in 462 ms
and exact target in 1,948 ms.

## Surface-Appearance Quality

The first material follow-up preserves three independent quality axes:

- Fast macro versus Sampled exact controls vertical density fidelity.
- Terrain Lab Resolution controls the horizontal sample lattice.
- Basic versus Inferred surface quality controls material classification at
  each retained point.

Basic retains the current biome top-material result and adds reference-shaped
grass color from the biome already selected for the point. Inferred adds one
evaluation of Vanilla's existing four-octave two-dimensional surface-noise
field and applies the production mountain, gravelly-mountain,
giant-tree-taiga, and shattered-savanna thresholds. This can reveal stone,
gravel, coarse dirt, and podzol without adding density columns, vertical
probes, neighbor height samples, or footprint taps. The shared shader also
uses derivatives of geometry it already renders to mark steep mountain faces
as stone; that classifier adds no worldgen sample.

The value is part of request, tile, cache, Worker, URL, and diagnostic
identity. Basic remains permanently selectable. Inferred becomes the default
because fixed timing and canonical visual evidence show that the bounded
lookup is worthwhile.

On the two fixed 65-by-65, 2 km grids, Inferred added 1.4-1.5% to median Fast
Macro compile time. Sampled Exact ranged from run noise to 3.7%. Across 1,668
non-water columns in seven relevant production surface fixtures, Basic
matched 1,309 visible materials and Inferred matched all 1,668. These results
validate the selected builder decisions, not complete column mutation or
arbitrary side-wall parity.

Mclone Overworld already emits water, sand, snow, stone, gravel, coarse dirt,
and grass through its macro surface contract and already supplies
temperature/moisture grass inputs. The shared palette and quality vocabulary
must preserve those semantics rather than treating the feature as
Vanilla-only.

Footprint material coverage, extra slope/height samples, biome-blend lookups,
and complete surface-builder mutation remain deferred. Their cost cannot be
hidden inside either first quality.

## First-Pass Included Contract

The first pass includes:

- the ordinary, unamplified Java 1.17.1 Overworld density and biome settings;
- direct absolute-coordinate column sampling derived from vanilla
  `NoiseBasedChunkGenerator.getBaseHeight` / `iterateNoiseColumn`;
- exact vanilla density interpolation at every queried point;
- solid terrain height, visible water height, water presence, and the selected
  block-position biome;
- an approximate visible surface material derived from the biome's ordinary
  top-material language;
- row-major 65-by-65 sample grids at Terrain Lab's existing power-of-two
  spacings and aligned tile identities;
- reuse of shared density lattice columns and a bounded sampler cache;
- asynchronous CPU tile compilation in a replaceable Web Worker;
- progressive coverage-first publication through the existing Terrain Lab
  viewport scheduler and tile renderer;
- exact canonical `Surface` and `Final features` panes generated by the
  corresponding selected profile; and
- terrain, height, biome, and surface-oriented review layers for vanilla.

The result is exact at sampled density columns but approximate between samples.
The first pass preserves the generator's large biome-driven terrain language,
oceans, coast-scale transitions, hills, mountains, valleys, and river-biome
relief to the extent visible at the requested sample lattice.

## First-Pass Exclusions

The first pass deliberately excludes:

- chunk materialization as an implementation shortcut;
- WebGPU evaluation or CPU/GPU comparison for vanilla;
- amplified, large-biome, customized, finite, or periodic vanilla worlds;
- Beardifier structure terrain influence;
- carvers, exposed caves, ravines, and underground representation;
- structures, placed features, decoration, vegetation, snow layers, and
  feature-order side effects;
- exact surface-builder mutation and random-stream parity;
- eroded-badlands pillars, frozen-ocean icebergs, swamp water correction,
  badlands color bands, and other specialized surface-builder geometry;
- exact grass/foliage biome tint, propagated light, shadows, and weather;
- footprint extrema, variance, conservative skyline summaries, or filtered
  roll-ups across finer child levels; and
- any collision, persistence, protocol, authoritative world, or save
  obligation.

Surface material and special-surface differences are accepted in this pass.
They must remain labeled as approximation, not hidden behind an exact or
vanilla-parity claim.

## Direct Sampler Contract

The Java reference operation conceptually evaluates four density columns
surrounding one block coordinate, trilinearly interpolates their vertical
cells, and scans from the top for a matching base state. The Rust exact
sampler exposes the same result without allocating a `16 x 16 x 256` block
buffer and skips columns whose interpolation weight is zero:

```text
seed + absolute X/Z
  -> 5 x 5 biome-depth/scale neighborhoods at density lattice corners
  -> one, two, or four cached vertical vanilla density columns
  -> trilinear interpolation and top-down vertical scan
  -> solid surface + visible water + biome + approximate material
```

Adjacent points and tiles reuse density columns by their absolute lattice
coordinate. Cache residency is optional for correctness, bounded, reset by
source identity, and byte-neutral between cold and warm results.

The sampler remains more expensive than Mclone's mostly two-dimensional field
graph. That difference is honest. Avoiding chunk buffers, all 256 horizontal
columns, surface mutation, carvers, feature regions, block palettes, and CPU
mesh construction is still the material win.

## Terrain Lab Capability Contract

For `overworld`:

- visible pane choices are `Real terrain`, `Sampled exact`, and `Fast macro`;
- canonical stage remains `Surface` or `Final features`;
- the procedural content checkpoint is the bounded vanilla macro surface;
- supported diagnostic layers are terrain, height, exact/macro error, biomes,
  and surface;
- Mclone hydrology, streams, wetlands, landforms, climate, cover, and
  GPU controls are unavailable rather than zero-filled;
- cache keys and reports include `overworld`; and
- switching profiles cancels incompatible Worker work and cannot publish a
  stale result.

Sampled exact and Fast macro own separate Wasm compilers, Workers, queues,
in-flight revision maps, resident buffers, target/coarse readiness, compile
timings, and source revisions. Their split view shares coordinates, camera,
footprint, spacing, and rendering, but neither product waits for the other to
publish a complete level.

Returning to `mclone-overworld-v1` restores the ordinary Mclone pane,
checkpoint, and layer choices. URL normalization must be deterministic and
must never preserve a hidden vanilla/Mclone pane mixture.

## Validation

The first pass was validated with:

- direct-sampler fixtures against full vanilla noise columns across different
  seeds, negative coordinates, density-cell edges, ocean floor, water surface,
  and far coordinates;
- cold/warm sampler equivalence and bounded density-column cache reuse;
- canonical vanilla `Surface` and `Final features` equivalence against their
  production generators;
- packed Worker-grid length and finite-value validation;
- viewport profile and cache identity tests plus shader validation;
- TypeScript URL, pane, checkpoint, layer, and profile normalization tests;
- Wasm type checking and a production Vite build;
- the existing headed-WebGPU Mclone comparison/browser test;
- headed-WebGPU vanilla profile tests on desktop and phone, including a
  profile change while vanilla preload work may still be in flight; and
- visual inspection of
  `/tmp/mclone-terrain-lab-desktop-chrome-vanilla-workspace.png` and
  `/tmp/mclone-terrain-lab-phone-chrome-vanilla-workspace.png`.

Tactical 246 additionally validated:

- exact one/two/four-column selection against complete noise-column and chunk
  output;
- deterministic cold/warm macro sampling and bounded sparse-column reuse;
- two signed-coordinate 65-by-65 native timing/error fixtures;
- mean, P95, and maximum solid/display height comparison metrics;
- independent exact/macro Worker admission and stale-result rejection;
- headed desktop and Pixel 7 viewport WebGPU comparison at 2 km; and
- a production Wasm/Vite build with profile-specific pane vocabulary.

The direct sampler contains no `GeneratedChunk` or chunk-buffer ownership.
Whole chunks remain confined to the separate canonical compiler.

The inspected LOD captures show coherent large relief, valleys, water,
profile labels, and no GPU pane. Vanilla terrain uses the approximate material
palette without the Mclone river overlay or Mclone LOD texture modulation.
The exact radius-zero patch is intentionally small within the 512-block
capture; it proves the independently generated canonical pane rather than
claiming an equal-footprint exact comparison.

Commands used:

```sh
cargo test -p mclone-worldgen terrain_preview --lib
cargo test -p mclone-terrain-view --lib
cargo check -p mclone-terrain-lab
pnpm --dir tools/terrain-lab typecheck
pnpm --dir tools/terrain-lab test
pnpm --dir tools/terrain-lab web:build
pnpm host:check -- --probe-browser-webgpu
pnpm --dir tools/terrain-lab exec playwright test \
  -c playwright.config.ts -g "worker-backed vanilla"
pnpm --dir tools/terrain-lab exec playwright test \
  -c playwright.config.ts --project desktop-chrome \
  -g "generates terrain, round-trips"
```

## Implementation Receipt

- `64e28914` records the bounded inclusion/exclusion contract.
- `cdb4ebed` adds the direct cached vanilla density-column sampler and
  profile-aware preview/tile identity.
- `00383533` routes exact Surface/Final compilation through the selected
  production terrain profile.
- `84726929` adds the global browser switch, vanilla Worker, asynchronous CPU
  tile admission, profile-aware rendering, capability UI, and browser proof.
- `92cd71cd` records the two-fidelity workstream.
- `3b056495` removes exact columns with zero interpolation weight.
- `44af6f29` adds the nine-node fast macro sampler and repeatable benchmark.
- `6182bb60` adds independent exact/macro renderer and Worker products.
- `8f772fff` records the surface-appearance quality contract.
- `a6fa839d` adds Basic/Inferred request identity, builder classification,
  grass tint, steep-face exposure, UI control, and evidence.

The browser main thread shares one Wasm initialization promise between its
exact and LOD canvases. The vanilla Worker owns a separate Wasm instance and a
retained sampler. Tiles cross the Worker boundary as transferable packed
`Float32Array` payloads and are checked against the active revision, profile,
seed, tile coordinate, spacing, and stage before upload.

Point receipts remain explicitly unavailable for vanilla in this pass. The UI
does not expose a zero-filled Mclone receipt as if it described vanilla.

## Known Follow-Ups

Possible follow-ups, each requiring its own explicit contract, are:

- a predicted-height bracket that improves rare peak/coast outliers within a
  similarly bounded density-probe budget;
- a GPU implementation of the stable macro semantics;
- specialized surface-builder geometry and closer material parity;
- footprint-aware relief summaries and truthful parent roll-ups;
- sparse semantic proxies for selected vanilla structures or vegetation;
- footprint material coverage or bounded dominant/secondary material taps,
  if later evidence justifies a quality above Inferred;
- use by a future footprint-growing in-game terrain hierarchy, if that
  architecture can preserve the sampler's bounded direct-column contract.

None is required to accept the bounded Terrain Lab first pass.

## Code And Documentation Map

- Java reference:
  `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java`
- shared density generator:
  `native/crates/mclone-worldgen/src/levelgen/generator.rs`
- preview products:
  `native/crates/mclone-worldgen/src/terrain_preview.rs`
- viewport planning/rendering:
  `native/crates/mclone-terrain-view/`
- Wasm facade:
  `native/apps/mclone-terrain-lab/`
- browser product and Workers:
  `tools/terrain-lab/`
- broader Mclone/GPU direction:
  [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md)
- reference profile and compatibility:
  [`world-generation-profiles.md`](world-generation-profiles.md)
