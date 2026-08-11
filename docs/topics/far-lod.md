# Retired Chunk-Based Far LOD

Topic: `retire-chunk-far-lod`

Status: chunk-based in-game Far LOD rejected and removed on 2026-07-25 by
Tactical
[`245`](../tactical/245-retire-chunk-far-lod-runtime.md).
The replacement is the current shared procedural-horizon geometry-clipmap,
recorded in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). It now runs
through World Explorer, Terrain Lab, and the live game.

In this document, **Far LOD** means only the retired chunk-based system.
Unqualified **LOD** in current project discussion means the replacement; see
the canonical terminology and routing page in [`lod.md`](lod.md). Do not use
this retirement record as current implementation guidance.

## Decision

Record the removal of the synthetic Far LOD vertical feature and prevent its
accidental revival.

The experiment is irreducibly chunk-granular:

```text
LodTileKey {
  chunk: ChunkPos,
  level: u8,
}

one tile footprint = one 16 x 16 block chunk
level changes sample spacing inside that footprint
```

This can reduce vertices per chunk, but coverage work, lifecycle state,
compilation requests, and arbitration still scale with the number of chunks in
the covered area. It is not a viable basis for a 10–100 km horizon.

Terrain Lab demonstrated the desired scale property: a bounded sample and
resident-tile budget spans increasingly large world-space footprints by
raising sample spacing. The replacement clipmap follows that direction and is
not constrained by the retired runtime's `ChunkPos + level` identity.

## Removal Result

Commit `ab1750de` deleted the complete vertical feature:

- both app-runtime producer/coverage modules and the renderer/settle modules;
- `LodTileKey`, chunk-band policy, caches, arbitration, and CPU tile payloads;
- native and browser compile work/result branches and packed codecs;
- scene/runtime preparation, prewarm, timing, diagnostics, and draw plumbing;
- desktop, web, Android, and XR settings, receipts, probes, and scripts; and
- the LOD material sidecar, palette, inventory entry, and Texture Lab
  placeholder family.

The implementation removal deleted 12,722 lines across 77 files. An exhaustive
non-document search now finds no old feature symbols or asset paths.

Do not land further:

- coverage-defect fixes;
- detail/range modes;
- reduced-real chunk sources;
- persisted LOD;
- chunk-clipped vegetation changes;
- new platform evidence; or
- refactors whose purpose is to make this producer easier to extend.

## Historical Value

The experiment established useful facts:

- distant presentation must remain non-authoritative;
- complete coarse coverage is more valuable than isolated fine patches;
- a replacing representation must be drawable before its parent disappears;
- exact and approximate terrain need explicit spatial overlap accounting;
- source revisions, cancellation, stale-result rejection, bounded residency,
  and device rebuilds must be first-class;
- optional work needs cooperative CPU/upload/frame budgeting;
- per-view and multiview rendering must use the same semantic source; and
- rendered depth/coverage evidence is stronger than queue-empty or color-only
  assertions.

It also produced an independently valid real-renderer fix: empty
high-altitude camera sections no longer become the sole visibility-graph seed.
That correction remains because it improves ordinary exact terrain.

Detailed execution history remains in:

- [Tactical 121](../tactical/121-surface-lod-first-slice.md);
- [Tactical 162](../tactical/162-real-chunk-lod-reduction-draft.md);
- [Tactical 166](../tactical/166-shared-resident-tile-substrate.md);
- [Tactical 172](../tactical/172-far-lod-settle-contract-and-detail-modes.md);
  and
- [Tactical 244](../tactical/244-lod-native-vegetation-presentation.md).

Those are historical records, not active implementation directions.

## Removal Boundary

Delete:

- `LodTileKey` and all chunk-level/band/guard policy;
- both app-runtime Far LOD modules;
- CPU Far LOD mesh generation and surface/vegetation worker caches;
- the native/browser Far LOD compile lane and packed browser codec;
- the renderer module, shaders, region arenas, resources, and frame payload;
- scene/runtime preparation, prewarm, coverage, settle, and pending-work
  integration;
- startup arguments, UI controls, capabilities, settings, and effects;
- LOD-only budget families, metrics, receipts, probes, fixtures, and scripts;
  and
- the Far LOD material sidecar and LOD-only asset-tool residue.

Keep because they have active non-LOD consumers:

- the normal render-section worker pool;
- the generic frame-budget controller and its real work families;
- the resident upload coordinator and render-section metadata cache;
- normal chunk authority, persistence, streaming, meshing, and rendering;
- generic depth readback, capture, movement, and frame-accounting tools;
- shared mono/per-eye/multiview frame composition;
- worldgen point/region samplers and structured semantic records;
- Terrain Lab's bounded viewport/residency pipeline; and
- Mclone forest intent, stable tree records, exact realization, and Terrain
  Lab vegetation presentation.

Retained generic machinery must lose its Far LOD variants and comments. The
current resident key abstraction maps a tile to one `ChunkPos`; it survives as
real-section implementation detail, not as the accepted multiscale terrain
substrate.

## Replacement Boundary

This section records the acceptance boundary established when Tactical 245
removed the old system. Tactical 245 intentionally left normal render distance
as the complete in-game terrain presentation; it did not install a placeholder
or hidden replacement in the same slice.

The later architecture study starts from
[`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) and measured Terrain
Lab behavior. The 2026-07-25 synthesis selected a toroidal geometry clipmap as
the preferred first in-game proof because fixed storage, draw shape, and
incremental strip updates fit XR frame predictability. A quadtree remains an
adaptive Lab/map comparator and a possible later hybrid, not a separate
content pipeline. The detailed ring, seam, exact-mask, vegetation, natural-only
v1, and shared-ownership contracts live in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md).

The replacement was required to prove:

1. bounded work by visible sample/patch budget rather than covered chunk area;
2. increasing world-space node extent with distance;
3. complete valid coverage before a finer ring, quadtree child set, or exact
   chunk representation replaces it;
4. direct untouched-terrain sampling without full `GeneratedChunk`
   materialization at every far node;
5. footprint-aware terrain, water, material, and vegetation summaries;
6. exact/procedural arbitration by spatial extent;
7. stable source identity and cancellation across all first-class clients;
   and
8. a renderer path whose CPU/GPU data movement is justified by measurements.

## Validation Result

The retirement is complete:

- every first-class client builds without Far LOD types or host plumbing;
- normal exact terrain rendered in inspected flat, synthetic-stereo, and
  headed-Wayland browser captures;
- flat Android and Android XR APK builds passed through their scripted NDK
  lanes;
- the serial full workspace suite, real-section worker, residency, upload,
  frame-budget, asset-pack, Texture Lab, Wasm, and adapter-purity gates passed;
- asset preparation no longer produces or expects the LOD material sidecar;
- no UI, CLI, preference, diagnostic, or package-script surface can enable the
  feature; and
- old-system names remain only in intentional historical documentation.

Current commands and slice ordering live in Tactical 245.
