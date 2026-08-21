# LOD Terminology and Routing

Topic: `lod`

Status: canonical current terminology and document-routing entry point as of
2026-08-21. Detailed implementation status remains in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md); this page does
not duplicate its execution ledger.

## Default Meaning

Unqualified **LOD** or **LOD system** means the current shared
procedural-horizon geometry-clipmap system in `mclone-terrain-view`. It was
first proven in World Explorer and is now consumed by World Explorer, Terrain
Lab, and the live game.

The player-facing settings name for the same system is **Distant Terrain**.
World Explorer is a standalone host and proof surface, not the implementation
owner and not a separate generation of the LOD system. A request for the
"World Explorer LOD" therefore routes to the current shared system unless the
request explicitly narrows itself to World Explorer host behavior.

## Required Qualification

Use these names consistently:

| Term | Meaning and route |
|---|---|
| **LOD**, **LOD system**, **current LOD** | Current procedural-horizon geometry clipmap; read [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). |
| **Distant Terrain** | Player-facing name for the current Off/Low/Medium/High LOD quality setting. |
| **World Explorer** | Standalone native/browser consumer and proof host over the shared system. |
| **retired chunk-based Far LOD**, **legacy/old LOD**, or **`LodTileKey` system** | Deleted chunk-granular experiment; read [`far-lod.md`](far-lod.md) only for removal boundaries and historical lessons. |
| **vanilla terrain LOD sampler** | Java 1.17.1 presentation sampler used by Terrain Lab; read [`vanilla-terrain-lod.md`](vanilla-terrain-lod.md). |
| **vegetation/model LOD** or **texture mip LOD** | Narrow representation-specific meanings; always keep the qualifier. |

Do not infer the retired chunk-based system merely because a user says LOD.
Do not call that experiment simply "Far LOD" in current planning, because
the current system also presents far terrain. The distinguishing property is
its retired chunk-based identity.

## Worldgen Product Boundary

LOD does not choose Mclone's geography. The accepted continental and
ecoregional direction is owned by
[`continental-ecoregion-planning.md`](continental-ecoregion-planning.md) and
is judged first through plan maps, regional three-dimensional exploration,
and walking-scale places. The current clipmap is a downstream consumer that
must present accepted terrain cheaply and consistently.

Tactical
[`325`](../tactical/325-continental-exact-lod-review.md) proves that boundary
with a detached continental candidate: one worldgen source feeds ordinary
exact chunks and every procedural level, while the existing focus-connected
coverage, frontier, water, and tree-ownership machinery composes them. It does
not make LOD the geography owner or change `mclone-overworld-v1`.

Do not require a world-generation mechanism to adopt the visually
disappointing Tactical 272/273 semantic terrain reconstruction merely because
it provides direct parent/child LOD. Reuse direct coarse queries, conservative
summaries, cache independence, and stable semantic identities where they help
the accepted geography. Camera scale, clipmap residency, and requested LOD
remain presentation inputs only.

## Current Ownership

The shared implementation routes through:

```text
mclone-worldgen semantic terrain sources
                  |
                  v
 mclone-terrain-view procedural horizon and composition
                  |
        +---------+---------+
        |         |         |
 World Explorer  Terrain Lab  live game / mclone-scene
```

`mclone-terrain-view` owns the shared clipmap, terrain-view, composition, and
detached exact-view contracts. `mclone-scene` owns live-game orchestration and
exact/procedural arbitration. App crates own only their platform or product
hosting responsibilities.

Current detailed status, limitations, evidence, and next work live in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). In particular,
that topic records the supported local `mclone-overworld-v1` source, the live
`Distant Terrain: Off / Low / Medium / High` control, platform defaults,
physical Quest evidence, and the sole direct exact-to-smooth frontier. Human
Review 1 accepted that direction, and the former spacing-one voxel shell plus
its capture comparison are deleted. Full-frame XR multiview shares the same
terrain contract but remains an opt-in diagnostic path rather than the
default. The exact frontier now uses a bounded 32-tile spacing-one support
belt with a complete resolution-aware land/water fallback, generation-coherent
admission, and shared mono/per-eye/multiview ownership; the detailed contract
and its platform bounds live only in the canonical topic. Tactical 323 makes
support suppression constant time and materially improves Quest Low/RD8, but
the preferred full-tile belt still misses the strict 72 Hz p95 gate. It does
reach 72 submissions per second in the accepted stationary sample, and the
Android XR unset default remains Low as established by Tactical 320; neither
fact is a stricter p95 lock. Tactical 327 makes live preset changes staged,
direct, atomic, and recoverable without changing those preset meanings or
defaults. Its physical Quest 3 closeout applies Off -> Low -> Medium -> High
-> Low through the shared settings actions in one process; all enabled targets
reach complete preferred-frontier receipts and the process ends normally at
Low.

Tactical 328 adds source-side frequency filtering for the continental
candidate: spacing-one queries preserve exact terrain, while coarse preview
queries remove shore/local/walking/micro detail below their representable
footprint. This corrects diagonal alias grain in broad continental views
without making the clipmap or camera scale an input to geography.

## Historical Routes

- [`far-lod.md`](far-lod.md) is the retirement record for the removed
  chunk-based runtime.
- [`../lod-architecture.md`](../lod-architecture.md) is its archived original
  architecture, not current guidance.
- Tacticals 121, 162, 172, 244, and 245 are historical execution records for
  that retired system and its removal.
- Current implementation tacticals are indexed from
  [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) and its
  coordinating Tactical 261.
