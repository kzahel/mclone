# Terrain Lab Large Canonical Footprints

Status: active 2026-07-25.

Topic: `gpu-procedural-terrain`

## Objective

Expand Terrain Lab's centered exact/canonical footprint from `9x9` to
`31x31` while preserving the responsive scheduler and making its material
cost visible.

The completed Lab must:

- offer centered footprint sides `1`, `3`, `5`, `7`, `9`, `11`, `15`, `21`,
  and `31`;
- accept the matching radii `0`, `1`, `2`, `3`, `4`, `5`, `7`, `10`, and
  `15` through URL and shared Rust ordering;
- generate and publish all `31x31 = 961` requested chunks progressively;
- retain `30x31 = 930` chunks across a one-chunk pan and schedule only the 31
  entering chunks;
- keep cached and generated admission paced and cancelable;
- bound the session raw exact cache while keeping a one-step return useful;
- expose measured resident raw, raw-cache, and used GPU mesh bytes; and
- prove complete desktop and phone pixels without weakening exact production
  generation or the selected Mclone/vanilla profile.

## Originating Direction

Human review accepted the chunk-boundary desired-set scheduler and observed
that `9x9` is now unnecessarily small. At a 512-block viewport, a `9x9`
footprint covers only 144 blocks. A centered `31x31` footprint covers 496
blocks and therefore nearly fills the same view with exact blocks.

An even `30x30` square has no single center chunk and would require an
asymmetric or half-chunk identity. Radius 15 gives the nearest symmetric
contract: `31x31 = 961`.

## Product Contract

### Stepped Centered Sizes

The UI exposes deliberate review sizes rather than every possible radius:

| Radius | Side | Chunks | Width |
| ---: | ---: | ---: | ---: |
| 0 | 1 | 1 | 16 blocks |
| 1 | 3 | 9 | 48 blocks |
| 2 | 5 | 25 | 80 blocks |
| 3 | 7 | 49 | 112 blocks |
| 4 | 9 | 81 | 144 blocks |
| 5 | 11 | 121 | 176 blocks |
| 7 | 15 | 225 | 240 blocks |
| 10 | 21 | 441 | 336 blocks |
| 15 | 31 | 961 | 496 blocks |

Rust remains authoritative for the centered order and clamps untrusted callers
to radius 15. TypeScript accepts only the stepped product choices.

### Responsiveness And Reuse

Tactical 239 remains the scheduling contract:

- camera motion within one center chunk does not change the exact epoch;
- a center-chunk change diffs the desired coordinate set;
- retained chunks and uploaded render sections remain drawable;
- cache and Worker results share bounded backpressure; and
- the main thread admits at most one exact chunk in one animation frame.

A complete radius-15 footprint shifted by one chunk must begin at `930/961`.
Only the 31 entering chunks may be generated or admitted. Returning one chunk
may reuse those 31 raw cache entries but must pace them over 31 frames.

The one-per-frame policy makes the zero-work lower bound for an initial 961
chunk publication roughly 16 seconds at 60 Hz. Actual generation and meshing
can take longer. Progressive usefulness and UI responsiveness take priority
over pretending the request is instantaneous.

### Bounded Cache

Canonical renderer residency remains exactly the current desired set. The
browser raw-result cache is a separate insertion-ordered LRU capped at 1,024
chunks. This holds one complete radius-15 footprint plus two entering
31-chunk edges, while preventing indefinite memory growth during a long pan.

Cache eviction may discard raw results for chunks that remain GPU-resident;
renderer residency owns its independent block copy. Returning beyond the LRU
window truthfully regenerates the missing coordinates.

### Memory Evidence

The canonical report exposes:

- resident raw block and biome bytes owned by the Wasm renderer;
- raw block and biome bytes retained by the browser cache;
- used vertex, index, and optional grass-patch bytes in the shared GPU mesh
  arenas; and
- their sum as tracked exact bytes.

This is a measured lower bound, not process memory. It excludes Wasm allocator
overhead, JavaScript object overhead, GPU buffer spare capacity, atlas,
pipelines, depth/color targets, generator dependency caches, and browser
runtime memory. Labels must say `tracked`, `raw`, or `mesh used` rather than
claiming total memory.

## Ownership

- `mclone-terrain-view` owns the shared maximum radius and centered order.
- `tools/terrain-lab/src/state.ts` owns the stepped URL/UI product choices.
- `mclone-render` owns reusable GPU mesh-arena byte facts.
- `mclone-terrain-lab` owns exact raw-resident and GPU-used aggregation.
- `CanonicalTerrainCanvas` owns the bounded browser raw cache, its byte count,
  and progressive browser reporting.

The large-footprint option does not change authoritative generation, game
render distance, LOD coverage, or profile semantics.

## Implementation Slices

1. Land this contract and update the tactical index.
2. Raise and test the shared Rust centered radius bound.
3. Add stepped browser validation and explicit UI labels through `31x31`.
4. Add bounded exact-cache LRU behavior and measured memory reports.
5. Add a slow focused browser proof for 961 progressive chunks, 930-chunk
   overlap, a 31-frame entering edge, and a 31-hit cached return.
6. Capture and inspect complete desktop and phone radius-15 panes.
7. Run local and hosted validation, deploy only `/terrain/`, and record the
   receipt.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.

## Acceptance

- `radius=15` round-trips and requests exactly 961 centered coordinates.
- An out-of-range radius is clamped to 15 in Rust and rejected to the product
  fallback by TypeScript.
- The footprint selector shows nine deliberate choices with an explicit
  `31 x 31 / 961 chunks / 496 blocks` maximum.
- Initial radius-15 publication is progressive and cancelable.
- One-chunk movement begins at 930 resident hits and completes after 31
  admissions.
- Returning one chunk records 31 cache hits over 31 admission frames.
- The exact raw cache never exceeds 1,024 chunks.
- Tracked memory is nonzero, grows with the footprint, and remains labeled as
  a lower bound.
- Desktop and phone headed-WebGPU captures show the complete shifted
  radius-15 footprint with zero browser errors.

## Explicit Follow-Ups

This slice does not move border-aware meshing out of the browser main thread.
If one paced chunk still causes visible hitches, the next step remains
Worker-side section meshing and transferable vertex/index payloads. A dynamic
viewport-derived exact radius may later choose among these same steps, but
this slice leaves selection explicit so generation and memory cost stay under
reviewer control.

