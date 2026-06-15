# L2 - Light solver foundation

Standing after [`L1-light-data-foundation.md`](L1-light-data-foundation.md) and the durable reference in [`../lighting.md`](../lighting.md). This slice ports the vanilla incremental lighting solver far enough to compute block and sky light in synthetic runtime levels.

## Goal

Land the pure solver and section-storage stack before wiring generated chunks or meshing:

- port the 16-queue incremental fixed-point graph solver
- port light section lifecycle storage, visible/updating maps, dirty section notification, and queued `DataLayer` acceptance
- port block-light source propagation and source removal
- port sky-light source sections, downward full-light propagation, and horizontal decay
- expose a vanilla-shaped `LevelLightEngine` coordinator over a small `LightChunkGetter`
- prove the solver with focused synthetic levels

At the end of `L2`, the runtime can compute stored sky/block light for active sections, but generated chunks still do not publish light in snapshots.

## Reference source

Read these before modifying the slice:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java` | queue ordering and two-direction light repair |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/SectionTracker.java` | section lifecycle graph behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DataLayerStorageMap.java` | visible/updating light data maps |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java` | queued section data, dirty sections, and notifications |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java` | block emission propagation |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java` | sky source bookkeeping |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java` | sky propagation rules |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java` | block/sky coordinator and section padding API |

## Scope

| # | Module | Result |
|---|---|---|
| 1 | packed coordinate helpers | `BlockPos` / `SectionPos` decode, offset, section-relative helpers, and `Direction.fromNormal` |
| 2 | graph primitives | `DynamicGraphMinFixedPoint` and `SectionTracker` |
| 3 | storage | `DataLayerStorageMap`, block/sky storage maps, queued data, dirty notifications |
| 4 | engines | `LayerLightEngine`, `BlockLightEngine`, `SkyLightEngine`, `LevelLightEngine` |
| 5 | adapter contract | `LightChunkGetter` and layer event listener interfaces |
| 6 | tests | synthetic level coverage for block and sky behavior |

## Explicit non-goals

- no generated chunk lighting pass yet
- no scan of generated chunks for emitting blocks
- no `lightCorrect` snapshot publication from the host
- no render-world light cache
- no mesh packed-light replacement
- no light deltas for live edits across the host/client protocol
- no browser visual validation

## Implementation status

L2 is landed:

- block light propagates from emitting states, crosses active section boundaries, and repairs after source removal
- sky light initializes from enabled chunk-column sources, stays full down open air columns, and darkens enclosed opaque rooms
- dirty light sections call `LightChunkGetter.onLightUpdate`
- `LevelLightEngine.getRawBrightness` combines sky and block layers with `skyDarken`
- `LevelLightEngine` preserves vanilla light-section padding through the L1 helpers

Known parity gap:

- Current block states do not expose vanilla voxel face occlusion shapes. The solver uses vanilla opacity and emission values, and shape-based occlusion is represented by a narrow approximation until shape APIs are ported.

## Validation

Focused validation:

```bash
pnpm test -- test/world/lighting/level-light-engine.test.ts test/world/data-layer.test.ts test/world/light-section.test.ts test/world/packed-chunk-snapshot.test.ts test/runtime/packed-chunk-wire.test.ts
```

`pnpm typecheck` was also run, but the current worktree contains an unrelated in-progress ore slice under `src/worldgen/levelgen/feature/configurations/ore-configuration.ts` that fails typecheck independently of lighting.

## Next

`L3` should wire initial chunk lighting into the authoritative host path:

- activate light sections for generated non-empty chunk sections
- enable sky sources for published chunk columns
- scan generated chunks for emitting states and call `onBlockEmissionIncrease`
- run lighting before publishing chunk snapshots
- populate `ChunkSnapshot.light` and `lightCorrect`
- compare the first lit generated/test chunk against committed oracle light bytes where block contents are known to match
