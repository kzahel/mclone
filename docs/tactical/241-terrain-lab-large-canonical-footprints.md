# Terrain Lab Large Canonical Footprints

Status: completed 2026-07-25, including local and hosted desktop/phone
headed-WebGPU maximum-footprint proof.

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

## Implementation Receipt

The shared exact order now clamps arbitrary callers to radius 15 and returns
961 center-first chunk coordinates. Browser URL state intentionally accepts
only the nine stepped product choices, and each selector label states side,
chunk count, and exact block width.

Canonical renderer residency continues to use the desired-set diff from
Tactical 239. The Wasm renderer now reports raw block/biome bytes and used
vertex/index/grass mesh-arena ranges after retain, accept, presentation, and
render operations. The browser raw-result cache is a tested access-ordered
LRU capped at 1,024 entries. Browser diagnostics expose cache count, both raw
domains, mesh used bytes, and their explicitly lower-bound tracked sum.

The headed browser regression starts at radius zero, observes progressive
publication after selecting radius 15, and waits for all 961 chunks. It then
moves exactly one chunk and requires 930 resident hits, 31 admission frames,
and a maximum of one admission per frame. Returning requires the same 930
resident chunks plus 31 cache hits over 31 frames. Desktop and phone both
passed. The complete local suite result was 14 passed and 2
platform-inapplicable cases skipped in 6.7 minutes.

Local validation passed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view
  -p mclone-terrain-lab --lib`: 24 passed;
- `cargo check --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --target wasm32-unknown-unknown`;
- `pnpm --dir tools/terrain-lab test`: 23 passed;
- `pnpm --dir tools/terrain-lab typecheck`; and
- the full headed-Wayland desktop/phone WebGPU suite: 14 passed, 2 skipped.

The complete local captures were inspected at:

- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-961.png`; and
- `/tmp/mclone-terrain-lab-phone-chrome-canonical-961.png`.

The production bundle built from `48128e5af0cf` was uploaded only under
`/terrain/`. Every immutable object was fetched from the public route and
byte-compared before `index.html` was published last:

| Object | SHA-256 |
| --- | --- |
| `index.html` | `02d3dc459a67f97559f0ae0ffaa7977869b59bb6b0bd4aa081d126f014bd4866` |
| `assets/canonical-worker-BQ6kSMVs.js` | `b30aba96dd7a337d8a4997e4d1959b9eab4a242bb739d30bc61381dbc51b2dad` |
| `assets/index-CoWCOr4c.css` | `63ee48983f72ded10cfeb1b5891f1da019576678e7059e2f74dc7fb1c70ad343` |
| `assets/index-DUxEAYZs.js` | `992ebefa1da6fa8bc47a8d0966764052fc6f648d561a0d41d5743872a7c24710` |
| `assets/lod-worker-DsChIzJJ.js` | `163c8df4a49368733c7a9bdfacf57c547b046cd10f0716b74fb0f514d542bf4d` |
| `assets/mclone_terrain_lab_bg-DmE252FX.wasm` | `22b08569c9c2b05479f6f5d57dfb3b014ed669c12259ac9916ee86eb3d7e348f` |

Standard hosted desktop and phone smokes passed with zero browser errors and
showed the new memory diagnostics. The opt-in maximum proof then exercised
the deployed bundle:

| Hosted lane | Initial 961 | Generated shift | Cached return |
| --- | ---: | ---: | ---: |
| desktop | 123.47 s | 6.72 s | 6.52 s |
| phone | 120.92 s | 7.16 s | 7.24 s |

Both lanes retained 930 chunks per move, admitted 31 chunks over 31 frames,
recorded 31 return cache hits, and ended with 992 cached chunks. Both measured
the same footprint:

| Tracked domain | Bytes | Approximate |
| --- | ---: | ---: |
| Wasm resident raw | 66,916,352 | 63.82 MiB |
| Browser raw cache | 66,916,352 | 63.82 MiB |
| GPU mesh used | 192,203,528 | 183.30 MiB |
| Tracked lower bound | 326,036,232 | 310.93 MiB |

Hosted completed-footprint captures were inspected at:

- `/tmp/mclone-terrain-lab-hosted-desktop-canonical-961.png`; and
- `/tmp/mclone-terrain-lab-hosted-mobile-canonical-961.png`.

Implementation commits are `c1064d2b` (contract), `5831f442` (shared radius
and controls), `bad0ac9f` (bounded cache and memory), `48128e5a` (browser
proof), `c6a12a37` (hosted proof lane), and `adcde5bb` (epoch-keyed wait),
followed by this receipt.
