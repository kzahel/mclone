# Terrain Lab Canonical Workspace

Status: completed 2026-07-24.

Topic: `gpu-procedural-terrain`

## Objective

Turn Terrain Lab into a configurable terrain-generation workspace rather than
only a CPU/GPU height-field comparison. The Lab must be able to show the same
seed, coordinates, scale, and camera through three independently hideable
panes:

- canonical first-party terrain after normal feature generation;
- CPU LOD-style surface terrain; and
- GPU-resident LOD-style surface terrain.

The canonical pane must use the production generator, block catalogue,
first-party texture atlas, and shared textured section compiler. It may use
cheap preview lighting, but it must not replace exact terrain with a
Lab-specific approximation.

## Originating Direction

The viewport experiment proved that both CPU and GPU field evaluation are fast
enough to make a continent-scale generator editor useful. It also made the
omitted work obvious: the LOD surface does not contain final rivers, bounded
features, vegetation, ordinary blocks, or textured water, and therefore
cannot by itself answer whether the actual generated world looks right.

Terrain Lab now has two related jobs:

1. remain the proving ground for CPU/GPU procedural coverage and refinement;
2. become the quickest way to inspect the real generator at local scale and
   compare its exact output with either LOD producer.

The result should also prove the rendering and readiness boundary needed by a
future in-game exact-to-LOD handoff. It does not yet replace Far LOD.

## Product Contract

### Pane Workspace

The three pane kinds are unique and independently visible. The default shows
all three. A URL records the visible set so a canonical-only, CPU/GPU, or
three-way review can be shared.

All visible panes share:

- seed and profile;
- absolute center coordinates;
- viewport scale;
- map or 3D presentation;
- orbit orientation; and
- navigation gestures.

One shared procedural rendering session draws CPU LOD, GPU LOD, or the aligned
two-panel comparison. One canonical rendering session draws exact textured
chunks. This bounds WebGPU devices and atlas copies to two even when all three
logical panes are visible. On a wide display, the procedural canvas receives
twice the width when it contains both logical panes. On a phone, logical panes
stack vertically at useful height.

### Canonical Generation

The exact source is
`McloneOverworldFeatureDependencyCache::generate_chunks` for
`mclone-overworld-v1`. A surface checkpoint may also be selected for
diagnosis, but the default is final feature-stage generation. Generated blocks
and biomes feed the shared textured terrain compiler and renderer.

The canonical pane has a separate bounded chunk radius. It does not pretend
that a 200 km viewport has materialized exact chunks. At large scale, the
small exact footprint remains centered and the procedural panes retain broad
coverage. This is a deliberate expression of independent visual and canonical
readiness.

Generation runs in a Web Worker. Requests are center-first and one bounded
chunk result is published as soon as it is available. A seed, center,
checkpoint, radius, or cache change starts a new epoch, terminates obsolete
queued work, and prevents stale results from reaching the renderer. A worker
cannot preempt the one synchronous chunk currently inside Rust, but replacing
the worker bounds that tail and cancels the remainder.

### Textures And Preview Lighting

The browser fetches the ordinary authored and generated-fallback first-party
packs. The canonical renderer constructs the shared block catalogue and
texture atlas from those packs and uses the existing textured section
compiler, including its solid, cutout, and translucent material passes.

The first slice deliberately bypasses authoritative propagated light. It uses
the compiler's face shading and ambient occlusion with a clearly labeled
preview-lighting mode. Lighting-dependent gameplay truth remains outside the
Lab. Water must use the ordinary translucent textured material rather than a
flat height-field color.

### Checkpoints And Visibility

Generation checkpoints and presentation visibility are separate:

- `Final features` executes the real final generator.
- `Surface` stops at the production surface checkpoint.
- water visibility may hide water geometry after generation;
- vegetation visibility may hide leaf, log, plant, and similar geometry after
  generation.

Visibility controls must not change generator execution or random-number
consumption. Hidden categories remain present in the generated chunk and can
be restored without producing a different world. Provenance-specific toggles
for structures, streams, ores, or decorators require stable generator
category metadata and are deferred rather than guessed from block identity.

### Cache And Diagnostics

Canonical cache state is independent from the existing procedural tile cache.
The UI can disable reuse and clear/rerun the current exact footprint. Status
reports:

- queued, compiling, published, and stale canonical chunks;
- first chunk and complete-footprint latency;
- generation and main-thread mesh/upload time where those boundaries are
  available;
- exact stage, radius, and presentation visibility; and
- existing CPU/GPU LOD readiness and cold-race facts.

## Shared Ownership

- `mclone-worldgen` remains the only owner of generator semantics and exact
  chunk production.
- `mclone-mesh` remains the only owner of textured block mesh semantics.
- `mclone-render` remains the owner of atlas-backed textured block drawing.
- `mclone-terrain-view` owns the reusable canonical preview request/result
  session and any host-neutral presentation filters.
- `mclone-terrain-lab` owns the narrow Wasm worker/compiler and browser-surface
  facades.
- `tools/terrain-lab` owns Worker mechanics, responsive pane layout, URL
  state, controls, and frame pumping.

The React application must not classify terrain fields or reconstruct block
geometry. The Wasm app must not copy worldgen equations or invent a second
texture renderer.

## Implementation Slices

1. Land this contract and update the durable topic.
2. Add a shared canonical preview compiler/session over exact generated
   chunks, including epoch and visibility contracts.
3. Add the Rust/Wasm worker facade and center-first browser scheduler.
4. Reuse the first-party atlas, textured section compiler, and textured
   renderer in a canonical Terrain Lab canvas.
5. Replace the single source switch with the synchronized, URL-addressed pane
   workspace and canonical controls.
6. Add progressive status, independent cache controls, surface/final
   checkpoint selection, and water/vegetation presentation filters.
7. Validate shared Rust, Wasm, state, desktop/phone interaction, rendered
   pixels, cancellation, cache-off behavior, and the hosted route.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.

## Validation

- exact fixed-seed chunk fingerprints agree with direct production generation;
- final and surface checkpoints have distinct, stable identities;
- presentation toggles do not alter the retained block/biome fingerprint;
- center or seed changes cannot publish stale exact chunks;
- cache-off reruns perform new exact work and publish progressively;
- canonical water, opaque blocks, cutouts, and vegetation render through the
  shared atlas;
- CPU and GPU LOD panes retain exact coordinate-locked comparison;
- pane visibility and navigation round-trip through the URL;
- wide and phone layouts preserve useful, understandable pane identities;
- headed-Wayland BrowserWebGPU captures are inspected at the first canonical
  drawable milestone and again after the complete workspace; and
- a targeted production upload changes only Terrain Lab assets.

## Stop Conditions

Stop for review only if:

- the production generator cannot run in the dedicated browser worker without
  pulling in server or renderer authority;
- the shared textured renderer cannot target a dedicated Terrain Lab surface
  without changing normal game pixels;
- multiple WebGPU sessions exceed a demonstrated browser/device resource
  limit; or
- the exact feature footprint is too slow or large to remain bounded even at
  a one-chunk radius.

## Explicit Follow-Ups

This slice does not yet:

- compute authoritative sky or block light;
- provide provenance-perfect toggles for every feature family;
- roll exact chunks into far summaries;
- replace the current Far LOD scheduler or renderer;
- blend exact and procedural geometry in one game render target; or
- move authoritative generation to the GPU.

After the workspace proves exact and procedural representations together, the
next architectural decision is whether to retain Far LOD's useful
coverage/residency/handoff control plane while replacing its synthetic content
path with the Terrain Lab's shared procedural evaluator and exact handoff
contract.

## Execution Receipt

The completed series landed:

- `27155259` — the canonical workspace contract;
- `99e78b53` — the shared exact terrain preview compiler;
- `6869cfc7` — typed Wasm worker payloads and Rust-owned chunk order;
- `d8a5124c` — first-party atlas-backed exact terrain rendering;
- `92838d98` — synchronized canonical/CPU/GPU panes and scheduling;
- `e86f6453` — stable cross-pane state and desktop/phone acceptance;
- `5d959ca2` — workspace smoke coverage and operator documentation; and
- `bb6f96ca` — atomic cold-race benchmark receipts.

`CanonicalTerrainCompiler` now produces surface or final production chunks
through `McloneOverworldFeatureDependencyCache`. Fixed-seed tests compare its
final result directly with production generation. Its stable fingerprint is
computed before presentation filtering, so hiding fluids or vegetation cannot
change retained generator truth.

The browser creates a replaceable Worker for each exact request identity.
Rust supplies the center-first chunk order. The Worker compiles and transfers
one typed chunk payload at a time; replacing the Worker cancels the remaining
queue and prevents stale epochs from reaching the canonical renderer.

The canonical surface uses the shared block catalogue, first-party authored
and generated-fallback packs, textured section compiler, and section draw
resources. New chunks update only their own sections and immediate neighbors;
the renderer no longer rebuilds one combined mesh after every arrival.
Authoritative propagated light is intentionally absent. The UI identifies the
first-party atlas, preview lighting, and generated fallback tiles.

The web workspace defaults to all three logical panes. It records pane set,
exact stage/radius, water/vegetation visibility, shared coordinates, scale,
view, and LOD layer in the URL. CPU and GPU LOD still publish independently
inside one procedural WebGPU session. Exact rendering uses a second session,
so the three-pane product does not allocate three devices or three atlases.

## Validation Receipt

Shared and browser validation passed:

- `cargo test -p mclone-terrain-view --lib` — 16 tests;
- the targeted `mclone-mesh` fluid-visibility test;
- `cargo test -p mclone-terrain-lab --lib` — 3 tests;
- `cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown`;
- Terrain Lab TypeScript/Wasm typecheck and 11 URL/state tests;
- production Vite build;
- Playwright desktop Chrome and Pixel 7 projects; and
- local and hosted headed-Wayland BrowserWebGPU desktop/phone smokes.

The complete 25-chunk radius-two exact footprint was also exercised manually.
Incremental section updates removed the earlier quadratic remesh behavior and
kept center-out pop-in observable. Captures were inspected at close exact
scale, with water hidden, at multiple land sites, and in the complete
three-pane desktop and phone layouts.

The final hosted Pixel-sized run compiled nine final chunks center-first:

- first exact chunk: 170.1 ms;
- complete exact footprint: 423.6 ms;
- accumulated generation: 146.3 ms; and
- accumulated mesh plus upload: 138.7 ms.

The hosted desktop run recorded 182.9 ms to the first exact chunk and
578.4 ms to all nine. These are browser wall-time observations for one fixed
site, not generator throughput promises.

Both hosted runs navigated to 65.5 km, retained effectively zero base-height
mean/P95 disagreement, 100% ocean agreement, and continentalness error around
`2e-8`. Their cache-off stress race visibly followed:

```text
neither target ready -> GPU target ready -> both targets ready
```

The hosted desktop race measured GPU target plus validation readback at
469.7 ms and CPU target publication at 11,726.4 ms while the host was under
other build load. The Pixel-sized run measured 1,010.2 ms and 3,052.8 ms.
These boundaries include different work and are not pure GPU execution
timings.

## Hosted Receipt

A targeted R2 upload changed only Terrain Lab objects; the Cloudflare Worker,
game, other labs, and root first-party pack objects were not redeployed.
Production now serves:

- JavaScript `index-BbUZPeEi.js`;
- Worker JavaScript `canonical-worker-DEYoRExd.js`;
- stylesheet `index-D63K5d8a.css`; and
- Wasm `mclone_terrain_lab_bg-CRfewa58.wasm`.

Each immutable object was uploaded and hash-verified before
`terrain/index.html` was switched. Direct HTTPS responses report the expected
JavaScript, CSS, and Wasm content types, one-year immutable caching for hashed
assets, and revalidation for HTML. Dedicated hosted desktop and Pixel smokes
then passed against `https://mclone.kzahel.com/terrain/`.
