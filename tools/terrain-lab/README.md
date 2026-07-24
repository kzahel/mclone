# Terrain Lab

Terrain Lab is the browser-hosted terrain workspace at `/terrain/`. Its
visible panes are independently configurable but coordinate-locked:

- `Real terrain` compiles exact first-party chunks through the production
  generator and renders their blocks, biomes, fluids, and final features with
  the production first-party texture atlas and cheap preview lighting.
- `CPU LOD` samples the production preview fields and near-detail structured
  records.
- `GPU LOD` evaluates the aligned natural fields in WebGPU and consumes the
  same sparse structured records.

The exact compiler runs in a replaceable Web Worker and publishes chunks
center-first as they finish. The LOD panes use 64 × 64 cell / 65 × 65 sample
tiles and may cover and progressively refine many tiles. Every visible pane
shares seed, center, footprint, camera, and navigation.

The URL owns the review state:

- `seed`: signed 64-bit world seed
- `x` and `z`: preview center in blocks
- `blocks`: continuous viewport width from 64 through 131,072 blocks
- `detail`: `auto` or a power-of-two sample spacing from 1 through 1,024
  blocks
- `panes`: comma-separated `canonical`, `cpu`, and/or `gpu`
- `canonical`: `surface` or `final`
- `radius`: exact chunk radius from `0` through `2`
- `water` and `vegetation`: `1` to show or `0` to hide retained exact blocks
- `source`: legacy/procedural compatibility value (`reference`, `gpu`, or
  `split`); new links should use `panes`
- `view`: `3d` or `map`
- `stage`: `base`, `hydrology`, `structured`, `surface`, or `cover`
- `layer`: `terrain`, `height`, `error`, `continentalness`, `climate`,
  `rivers`, `wetlands`, `landforms`, `biomes`, `surface`, or `streams`

Legacy `spacing=` links still load with their original `spacing × 64`
footprint and fixed detail. New links keep coverage and resolution independent.

Wheel and pinch zoom the continuous viewport. The stage captures wheel events
through a non-passive native listener, so a Mac trackpad or mouse wheel zooms
terrain without scrolling the document. Map zoom is anchored under the
pointer; map drag follows grab semantics on both axes; 3D left drag orbits;
and right, Shift+left, or middle drag pans in 3D. Arrow keys pan both views.
A click or tap without a drag selects a production point receipt using analytic
map picking or the shared 3D projection and a bounded heightfield ray.
`Auto` selects approximately two CSS pixels per sample cell. A manual detail
request remains visible even when the bounded eight-tile-per-axis interactive
budget must raise its effective spacing; zooming in eventually admits every
manual level, including `1:1`.

The exact scheduler terminates and replaces its Worker when generation
identity changes, so stale chunks cannot land after a seed, center, checkpoint,
radius, or cold-cache change. Water and vegetation switches only remesh
retained chunks; they do not rerun or mutate generation. The shared LOD
scheduler has independent CPU compilation and GPU dispatch queues. Each lane
covers the viewport coarsely before refining through nested power-of-two
levels, and each publishes its finest complete level without waiting for the
other. The evidence panel reports exact arrival/generation/upload timing and
the LOD queues, readiness, throughput, and published detail.

CPU reference wall time and WGPU encode/submit wall time are not GPU execution
time. Portable timestamp queries are not enabled in this Lab slice, so the UI
says `GPU execution: unavailable` rather than inferring a GPU/CPU speed ratio.
The GPU evaluator remains a preview experiment, not authoritative world
generation. Content checkpoints are dependency-ordered preview compiler
stages, not gameplay generator flags. Natural rivers, banks, wetlands, pools,
and submerged outlets are pure CPU/GPU fields. Planned streams remain bounded
CPU route records reconstructed only through `1:4`, then merged into either
LOD lane. Coarser structured views explicitly report them unavailable.

When both LOD panes are visible, wide canvases place them side by side and
portrait canvases stack two full-width views. The CPU and GPU panels publish
independently at identical coordinates; their labels expose the race while
the final comparison waits for matching target samples. The exact pane is not
another height field: it is a bounded block-and-feature patch nested at the
same world center and scale. Generated fallback atlas tiles intentionally
remain visible for block materials that do not yet have curated first-party
textures.

The exact and LOD caches are independent. `Exact cache on` retains generated
chunks by seed, checkpoint, and chunk coordinate; `Exact cache off` compiles
the current patch cold. `Cache on` uses a session-local 192-tile LRU keyed by
seed, aligned origin, sample spacing, and content stage. It retains CPU samples, uploaded
reference data, GPU-computed buffers, and completed validation readbacks while
panning and zooming. Camera, layer, and pane visibility are not LOD cache
identity. `Cache off` retains only the active request and disables speculative
preload. `Cold current view` invalidates both cache domains explicitly.

`Run stress race` selects the fixed review seed/site, turns cache off, and
uses a larger 12-tile-per-axis diagnostic budget over a 2 km requested-`1:2`
Compare map. This makes independent publication observable without changing
the default interactive budget. CPU end-to-end and GPU
dispatch-to-validation-readback throughput use labeled, different timing
boundaries; neither is presented as pure GPU execution time.

The LOD preview surface uses upward-facing counter-clockwise triangles,
rejects back faces, and projects the oblique camera from above the height
field. CPU and GPU LOD share the same per-tile sample lattices and viewport
plan. The sample payload retains natural river signed distance and width, so
the renderer preserves a bounded river contour at levels coarser than the
physical channel. Comparison reports both base and final height/water/material
agreement plus channel, bank, wetland, biome, and surface agreement. Full
provenance remains point-local in the inspector rather than bloating every
tile.

## Develop

From the repository root:

```sh
pnpm terrain-lab:install
pnpm terrain-lab:web
```

Vite serves the lab at <http://127.0.0.1:5180/terrain/>. Rust/WASM bindings are
regenerated before every development or production build.

## Validate

```sh
pnpm terrain-lab:typecheck
pnpm --dir tools/terrain-lab test
pnpm terrain-lab:web:test
pnpm terrain-lab:web:smoke
pnpm terrain-lab:web:smoke -- --mobile
```

The smoke path follows the repository's headed-Wayland WebGPU policy on Linux
and writes screenshots and its diagnostic report under `/tmp`.

To exercise an already-hosted aggregate deployment without rebuilding or
starting Vite:

```sh
TERRAIN_LAB_SMOKE_BASE_URL=https://mclone.kzahel.com \
  pnpm terrain-lab:web:smoke
```
