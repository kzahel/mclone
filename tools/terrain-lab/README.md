# Terrain Lab

Terrain Lab is the browser-hosted macro terrain workbench at `/terrain/`. It
keeps the production first-party terrain sampler as the CPU reference while a
small, explicitly approximate WebGPU evaluator generates aligned resident
terrain tiles. Each tile has a 64 × 64 cell / 65 × 65 sample lattice, while
the viewport may cover and progressively refine many tiles.

The URL owns the review state:

- `seed`: signed 64-bit world seed
- `x` and `z`: preview center in blocks
- `blocks`: continuous viewport width from 64 through 131,072 blocks
- `detail`: `auto` or a power-of-two sample spacing from 1 through 1,024
  blocks
- `source`: `reference`, `gpu`, or `split`
- `view`: `3d` or `map`
- `layer`: `terrain`, `height`, `error`, `continentalness`, or `climate`

Legacy `spacing=` links still load with their original `spacing × 64`
footprint and fixed detail. New links keep coverage and resolution independent.

Wheel and pinch zoom the continuous viewport. Map zoom is anchored under the
pointer; map drag pans; 3D left drag orbits; and Shift+left or middle drag pans
in 3D. `Auto` selects approximately two CSS pixels per sample cell. A manual
detail request remains visible even when the bounded eight-tile-per-axis
budget must raise its effective spacing; zooming in eventually admits every
manual level, including `1:1`.

The shared Rust scheduler covers the viewport coarsely before refining through
nested power-of-two levels. It publishes only complete levels, preserves
resident tile identities across presentation changes, preloads a target-level
margin, compiles under a per-frame CPU budget, and rejects queued work from
obsolete viewport epochs. The evidence panel distinguishes requested,
effective, and published detail and reports queue, cache, stale-work,
coarse-pixel, and target-pixel facts.

CPU reference wall time and WGPU encode/submit wall time are not GPU execution
time. Portable timestamp queries are not enabled in this Lab slice, so the UI
says `GPU execution: unavailable` rather than inferring a GPU/CPU speed ratio.
The GPU evaluator remains a preview experiment, not authoritative world
generation.

`split` is a synchronized paired comparison: every published tile is drawn
twice with identical world coordinates and camera state. Wide canvases place
the views side by side; phone-sized portrait canvases stack two full-width
views so Compare does not halve their screen-space detail. The first panel
uses the CPU production base and the second uses the GPU production base.
`reference` separately shows CPU-final terrain including rivers, wetlands,
and planned streams. The error layer and aggregate readback report CPU-base
versus GPU-base disagreement across the complete target viewport.

The preview surface uses upward-facing counter-clockwise triangles, rejects
back faces, and projects the oblique camera from above the height field.
CPU-final and Compare share the same per-tile sample lattices and viewport
plan. CPU-final can look busier because narrow watercourse and bank fields are
point sampled; those aliases are evidence for a later scale-aware summary
rather than additional mesh resolution.

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
