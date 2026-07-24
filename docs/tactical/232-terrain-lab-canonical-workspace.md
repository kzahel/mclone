# Terrain Lab Canonical Workspace

Status: active 2026-07-24.

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
