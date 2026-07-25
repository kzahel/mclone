# Terrain Lab Worker Canonical Meshing

Status: implemented and locally validated 2026-07-25; hosted closeout
pending.

Topic: `gpu-procedural-terrain`

## Objective

Move exact/canonical Terrain Lab CPU mesh construction off the browser UI
thread, remove per-arrival neighbor-remesh amplification, and make recently
departed exact strips immediately reusable without changing generated blocks,
textured section semantics, or GPU renderer ownership.

The completed slice must:

- keep canonical generation and mesh construction in a persistent Web Worker;
- construct meshes through the shared production `mclone-mesh` catalogue and
  builder;
- transfer an explicit, versioned section-bundle representation rather than
  Rust/Wasm objects;
- leave WebGPU surface, arenas, upload, culling, and drawing on the renderer
  thread;
- batch raw compilation before deduplicating new and boundary mesh targets;
- retain up to 64 recently departed exact chunks in inactive GPU residency;
- pace both transferred uploads and warm activations through the existing
  cancelable admission scheduler;
- report generation, worker mesh, transfer, main decode, GPU upload, target
  fan-out, and warm activation evidence separately; and
- prove exact fixed-chunk mesh parity and complete desktop/phone pixels.

## Originating Direction

The radius-15 proof completed 961 exact chunks but measured about 123 seconds
for the initial footprint and 6.5--7.2 seconds for either a generated
one-chunk shift or a raw-cache return. The cached return performs no terrain
generation, so it isolates the remaining problem: raw cache entries still
enter `CanonicalTerrainLab.acceptChunk` on the UI thread, where each arrival
may rebuild itself and four cardinal neighbors before upload.

The existing canonical Web Worker is real but narrow. It owns
`CanonicalTerrainCompiler` and transfers block/biome arrays. It does not load
the textured mesh catalogue, retain the active neighbor halo, build render
sections, or produce GPU-ready payloads.

## Current Cost Shape

The current path is:

```text
Worker: generate chunk -> transfer blocks + biomes
Main:   copy raw -> presentation conversion
        -> mesh arrival plus up to four neighbors
        -> encode vertices/indices -> GPU upload -> draw
```

For an entering 31-chunk edge, independently accepting every result can
rebuild adjacent chunks repeatedly. Pacing at one admission per animation
frame limits event-loop monopolization but does not reduce the duplicated CPU
work inside each admission.

## Product And Runtime Contract

### Persistent Canonical Worker

One Worker session survives ordinary center-chunk and radius changes while
profile, seed, checkpoint, and asset-pack identity stay unchanged. A hard
identity change replaces the Worker and all worker-owned raw/mesh state.

Every desired-set update carries a monotonically increasing epoch,
presentation revision, complete desired coordinates, water/vegetation
visibility, and center-first missing work. Results repeat both revisions.
The main thread rejects obsolete bundles before decode or upload. A Worker can
finish its current synchronous Rust call, but no stale result may mutate GPU
residency.

### Batched And Deduplicated Mesh Work

The Worker compiles a small bounded coordinate batch into raw chunks first.
It then forms one target set containing:

- each newly available requested coordinate; and
- already available active cardinal neighbors whose shared boundary changed.

The set is deduplicated before calling the shared builder. Section results are
grouped by owning chunk so the main thread can continue admitting one logical
chunk update at a time. The first batch remains deliberately small for early
pixels; later batches may grow under bounded backpressure.

### Transfer Bundle

The cross-Wasm boundary uses a versioned packed section bundle containing:

- section key and visibility bits;
- solid and opaque index boundaries;
- packed textured vertex bytes;
- little-endian `u32` index bytes;
- packed grass-patch bytes; and
- empty-section records required to remove obsolete GPU ranges.

Encoding and decoding live beside shared mesh data rather than in React.
Fixed synthetic and production chunks must round-trip exactly. Browser
messages transfer the backing `ArrayBuffer` values.

The main renderer exposes an upload-only entry point. It must not receive raw
blocks or invoke the block mesher on the UI thread.

### Warm Inactive Residency

Renderer residency distinguishes:

- active chunks, which are traversal-ready and drawable;
- warm chunks, which retain GPU section ranges but are not traversal-ready;
  and
- evicted chunks, whose section ranges are released.

Up to 64 departed chunks remain warm, enough for two 31-chunk edges. Returning
to a warm coordinate schedules a lightweight activation through the ordinary
one-per-frame admission queue and performs no generation, mesh construction,
transfer, decode, or GPU upload.

Warm residency is presentation-specific. A profile, seed, checkpoint,
water/vegetation change, asset change, or explicit cold-cache action clears or
invalidates it. Memory diagnostics report active and warm mesh use together
and expose warm chunk count separately.

### Scheduling

The UI thread remains responsible for:

- desired-set diff and current epoch;
- worker backpressure;
- one logical chunk admission per animation frame;
- stale-bundle rejection;
- packed-bundle decode and GPU upload;
- warm activation/deactivation;
- rendering and evidence publication.

Admission later may become byte/time-budgeted, but this slice keeps the
existing one-logical-chunk maximum and measures decode/upload duration so a
follow-up can choose a defensible budget.

## Ownership

- `mclone-mesh` owns packed section encoding/decoding and parity tests.
- `mclone-terrain-lab` owns the Worker-side canonical mesh compiler and the
  renderer upload/active/warm facade.
- `canonical-worker.ts` owns persistent Worker session messages and
  transferable buffers.
- `CanonicalTerrainCanvas` owns epochs, bounded batch dispatch, frame-paced
  upload/activation, and product diagnostics.
- `mclone-render` continues to own WebGPU arenas and drawing.

No gameplay, server, persistence, native render distance, or authoritative
lighting contract changes.

## Implementation Slices

1. Land this tactical contract.
2. Add packed shared section bundles with lossless round-trip tests.
3. Add a Worker-side canonical mesher initialized from the first-party packs.
4. Add an upload-only canonical renderer method and remove main-thread raw
   meshing from ordinary admission.
5. Make the canonical Worker persistent and batch/deduplicate mesh targets.
6. Add 64-chunk inactive renderer residency and paced warm activation.
7. Add stage timing, fan-out, warm-hit, and main-admission evidence.
8. Extend the 9x9 and 31x31 browser regressions and inspect desktop/phone
   output.
9. Deploy only `/terrain/`, byte-verify immutable assets, run hosted standard
   and maximum proofs, and record the receipt.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.

## Acceptance

- A fixed section bundle round-trips keys, vertices, indices, layer counts,
  grass patches, visibility, and empty sections exactly.
- Browser canonical admission never calls the raw-block mesh entry point.
- Worker mesh output matches the former in-process production builder for
  fixed Mclone and vanilla chunks.
- One Worker persists across a one-chunk move and return.
- Mesh target counts are deduplicated per batch and reported.
- A generated radius-15 shift retains 930 chunks and publishes 31 entering
  chunks without UI-thread mesh construction.
- The immediate return records 31 warm activations, zero worker meshes, and
  zero GPU uploads for those chunks.
- Warm residency never exceeds 64 chunks.
- Maximum main-thread admission duration is reported separately from Worker
  mesh time.
- Existing presentation toggles, cache-off behavior, profile switching,
  cancellation, map/3D navigation, and exact textures remain correct.
- Local and hosted desktop/phone headed-WebGPU runs complete with zero browser
  errors and inspected pixels.

## Explicit Follow-Ups

This slice uses one canonical Worker. It improves responsiveness and removes
duplicated work but does not promise linear wall-time acceleration. A worker
pool or shared-memory Wasm threads is a later measured step if Worker mesh
time remains dominant.

GPU upload remains on the renderer thread because WebGPU buffers and the
existing surface/arena owner are not transferable. Moving the full renderer
to an `OffscreenCanvas` Worker is a different platform experiment, not part of
this tactical.

## Local Implementation Receipt

Canonical exact generation, presentation conversion, textured section
construction, and section packing now run in one persistent
`CanonicalTerrainMeshSession` inside the existing Web Worker. The browser
sends center-first batches of `1`, `2`, `4`, `8`, then at most `16`
coordinates under the existing pending-result high-water mark. The Rust
session compiles each raw batch before forming one deduplicated requested plus
cardinal-neighbor target set and invoking the production
`build_textured_render_sections_for_chunk_set` builder once.

`mclone-mesh` now owns a `MCLMSH01` little-endian transfer format. Its decoder
rejects bad magic, unsupported versions, truncation, trailing bytes, invalid
layer boundaries, and non-quad index counts. The lossless tests cover section
keys, traversal visibility, solid/opaque counts, complete packed vertices and
indices, grass patches, and explicit empty sections.

The main-thread canonical renderer accepts only packed mesh admissions on the
ordinary browser path. It decodes the bundle, updates shared GPU mesh arenas,
and marks the requested chunk active. Departed chunks keep their GPU ranges
but leave traversal readiness; a bounded LRU retains at most 64 of these warm
chunks. A one-chunk return schedules 31 activation-only admissions and performs
no Worker mesh, transfer, decode, or upload work.

The evidence panel now separates:

- Worker generation, presentation, mesh, pack, and transfer time;
- main-thread packed decode and GPU upload time;
- maximum complete main-thread admission duration;
- deduplicated Worker mesh-target fan-out;
- Worker-owned raw cache bytes and chunk count;
- main-thread raw bytes, which remain zero on the packed path; and
- warm activation hits and inactive warm chunk count.

The focused `9x9` regression proves nonzero Worker mesh and main decode work
for a generated entering strip, fewer deduplicated mesh targets than the old
five-target-per-arrival upper bound, and zero Worker mesh/decode work on the
warm return. Cache-off replaces both the Worker session and renderer
residency, so it remains a genuinely cold control.

Local validation passed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh
  -p mclone-terrain-lab -p mclone-terrain-view --lib`: 122 passed;
- `cargo check --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --target wasm32-unknown-unknown`;
- `pnpm --dir tools/terrain-lab test`: 21 passed;
- `pnpm --dir tools/terrain-lab typecheck`;
- `pnpm host:check -- --probe-browser-webgpu`; and
- the full headed-Wayland browser matrix: 14 passed and two
  platform-inapplicable desktop cases skipped in 8.0 minutes.

The maximum proof passed independently on both desktop and phone in about
2.5 minutes per lane. Each run progressively published 961 chunks, retained
930 across a one-chunk shift, admitted the 31 generated chunks one per frame,
kept 31 departed chunks warm, then returned through 31 activation-only frames
with zero Worker mesh and zero main decode time. The completed local captures
were inspected at:

- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-961.png`; and
- `/tmp/mclone-terrain-lab-phone-chrome-canonical-961.png`.

The single Worker does not materially reduce initial 961-chunk wall time.
That result is expected: production generation and mesh construction still
perform comparable total math. This slice instead removes that math from the
UI thread, deduplicates within batches, eliminates main-thread raw copies, and
makes immediate return work activation-only. Hosted stage timings will decide
whether the next throughput experiment should be a small Worker pool.
