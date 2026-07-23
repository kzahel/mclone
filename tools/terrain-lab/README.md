# Terrain Lab

Terrain Lab is the browser-hosted macro terrain workbench at `/terrain/`. It
keeps the production first-party terrain sampler as the CPU reference while a
small, explicitly approximate WebGPU evaluator generates and renders the same
64 × 64 preview lattice on the GPU.

The URL owns the review state:

- `seed`: signed 64-bit world seed
- `x` and `z`: preview center in blocks
- `spacing`: power-of-two sample spacing from 2 through 1,024 blocks
- `source`: `reference`, `gpu`, or `split`
- `view`: `3d` or `map`
- `layer`: `terrain`, `height`, `error`, `continentalness`, or `climate`

This makes a view reproducible and shareable without loading a game world or
chunk store. The evidence panel reports the CPU sampling time, GPU
encode/submit time, resident buffer size, and an asynchronous readback
comparison. The GPU evaluator is a preview experiment, not authoritative world
generation.

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
