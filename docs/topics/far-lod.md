# Far Terrain Presentation

Topic: `far-lod-settle-contract`

Status: chunk-based in-game Far LOD rejected for product use on 2026-07-25.
Tactical
[`245`](../tactical/245-retire-chunk-far-lod-runtime.md)
owns its complete removal. No replacement in-game distant-terrain system is
currently selected or active.

## Decision

Remove the current synthetic Far LOD vertical feature instead of continuing to
fix, tune, or generalize it.

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

Terrain Lab demonstrates the desired scale property: a bounded sample and
resident-tile budget spans increasingly large world-space footprints by
raising sample spacing. The eventual game design should be informed by that
pipeline and by footprint-aware coarse summaries. It should not be constrained
by the current runtime's `ChunkPos + level` identity.

## Current Implementation Pending Removal

The rejected implementation is still present until Tactical 245 lands:

- `mclone-app-runtime::far_lod` owns chunk-band selection, 4/8/16 spacing,
  hysteresis, movement guards, startup prewarm, CPU tile compilation, retained
  cache, upload admission, and Mclone vegetation proxies;
- `mclone-app-runtime::lod_coverage` selects real or LOD visibility per
  `ChunkPos`;
- native and browser render compilers carry a second Far LOD work/result lane;
- `mclone-render::far_lod` owns fixed 16-by-16-chunk region arenas and
  mono/per-eye/multiview draw pipelines;
- `mclone-scene` owns Far LOD resources, frame preparation, arbitration,
  timing, and settle snapshots;
- desktop, web, Android, and XR hosts expose configuration, metrics, and proof
  plumbing; and
- a dedicated settle-probe suite and three checked-in fixture scripts validate
  the old product.

The direct implementation/probe files contain more than 8,000 lines, with
cross-cutting references in dozens of shared and platform files. This is a
vertical feature removal, not a one-file renderer deletion.

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

Tactical 245 intentionally leaves normal render distance as the complete
in-game terrain presentation. It does not install a placeholder or hidden
replacement.

The later architecture study starts from
[`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) and measured Terrain
Lab behavior. A geometry clipmap, quadtree, or hybrid remains a candidate, not
a decision.

The future system must prove:

1. bounded work by visible sample/patch budget rather than covered chunk area;
2. increasing world-space node extent with distance;
3. complete parent coverage before finer children replace it;
4. direct untouched-terrain sampling without full `GeneratedChunk`
   materialization at every far node;
5. footprint-aware terrain, water, material, and vegetation summaries;
6. exact/procedural arbitration by spatial extent;
7. stable source identity and cancellation across all first-class clients;
   and
8. a renderer path whose CPU/GPU data movement is justified by measurements.

## Validation Direction

The retirement is complete only when:

- every first-class client builds without Far LOD types or host plumbing;
- normal exact terrain renders in flat, synthetic-stereo, browser, Android,
  and XR boundaries selected by the removed APIs;
- real-section worker, residency, upload, and frame-budget tests remain green;
- asset preparation no longer produces or expects the LOD material sidecar;
- no UI, CLI, preference, diagnostic, or package-script surface can enable
  the feature; and
- an exhaustive search finds old-system names only in intentional historical
  documentation.

Current commands and slice ordering live in Tactical 245.
