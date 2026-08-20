# LOD Terminology and Routing

Topic: `lod`

Status: canonical current terminology and document-routing entry point as of
2026-08-20. Detailed implementation status remains in
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
default.

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
