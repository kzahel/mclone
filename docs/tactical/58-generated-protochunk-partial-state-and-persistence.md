# Tactical 58 - Generated protochunk partial state and persistence

Make generated chunks status-shaped in storage and memory so partial work can be saved, loaded, and resumed like vanilla `ProtoChunk` state.

Status: first in-memory slice landed. This follows or overlaps Tactical [`57`](57-generated-chunk-holder-status-futures.md) and completes the data-shape half of Tactical [`49`](49-vanilla-status-futures-and-partial-chunks.md).

## Goal

Move from "published snapshots are cached" toward "the latest generated `ChunkAccess` state is owned by the host":

- metadata-only statuses can exist without block sections
- terrain/status progress can survive route changes and unload
- saved chunks include an explicit generated status and content version
- dirty/generated-cache policy distinguishes user mutations from deterministic generated cache
- `FULL` snapshots remain a client transport/cache product, not the only durable generated state

Acceptable means full parity with vanilla's reuse shape, not necessarily Mojang NBT byte layout. The browser/Node storage format can stay engine-native if it preserves the same status facts and resume behavior.

## Source files

Read these before writing code:

| Source | Purpose |
|---|---|
| [`ChunkAccess.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkAccess.java) | shared proto/full interface |
| [`ProtoChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java) | partial chunk fields: status, sections, starts, references, masks, ticks, postprocessing, lights |
| [`LevelChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java) | full chunk conversion target |
| [`ImposterProtoChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ImposterProtoChunk.java) | proto-shaped wrapper for already-full chunks |
| [`ChunkSerializer.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java) | vanilla load/save status fields and chunk data shape |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | unload/save path and `chunkToSave` usage |
| [`../loading-persistence.md`](../loading-persistence.md) | current storage delta and target separation of durable state from generated cache |
| [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) | status ordering and metadata-only dependency contract |

## Current gap

The current generated host can label chunks by status in memory, but persisted generated data is only written after a publishable snapshot is built. Chunks outside the authority window can lose partial terrain/features/status records. That is not vanilla's shape: a holder tracks the latest generated `ChunkAccess` through `chunkToSave`, and `ProtoChunk` can carry meaningful status metadata before it is full or publishable.

Tactical [`59`](59-generated-holder-residency-and-save-queue.md) owns holder residency, unload/save queueing, and lazy generated-cache writes. This tactical owns the data shape that those queues should eventually save.

This matters for performance and parity:

- metadata rings for structures should not require terrain sections
- canceled or rerouted generation should not throw away partial status work that vanilla would retain/save
- future structures, carving masks, postprocessing, proto ticks, and light-correct state need a home before `FULL`
- saved generated-cache records need content/status versions so algorithm changes can invalidate deterministic cache safely

## Landed first slice

The generated host now has a vanilla-shaped in-memory holder slot:

- [`GeneratedProtoChunk`](../../src/world/level/generated-proto-chunk.ts) records chunk position, completed status, section presence, dirty/unsaved state, and a generated-content version.
- `GeneratedChunkHolder.chunkToSave` points at the latest proto/full generated access while the holder is resident, mirroring vanilla `ChunkHolder.chunkToSave` at the ownership level.
- Status tasks update the holder access when work completes or when an already-completed status is reused.
- `FULL` promotion converts the proto access to a full/level access; published snapshots are still transport/cache products.
- Debug records expose exact holder-side access counts for tests.

This does **not** persist partial chunks to disk yet. It gives the save queue a status-shaped object to save later instead of forcing persistence to be derived from packed published snapshots.

Observed flat-grass radius-0 crossing counts are now tested:

| Step | Holder access records | Records with sections | Full/level records | Exact statuses |
|---|---:|---:|---:|---|
| Initial center `(0, 0)` | 625 | 121 | 49 | 504 `STRUCTURE_STARTS`, 40 `LIQUID_CARVERS`, 32 `FEATURES`, 49 `FULL` |
| Move to `(1, 0)` | 625 | 132 | 56 | 493 `STRUCTURE_STARTS`, 42 `LIQUID_CARVERS`, 34 `FEATURES`, 56 `FULL` |

The same test still verifies generation call deltas: moving one published grid over publishes 5 new chunks but currently materializes 11 new terrain chunks, decorates 9 chunks, and marks 7 additional chunks full because vanilla status dependencies require surrounding work beyond the 5 visible chunks.

## Implementation plan

1. **Define `GeneratedProtoChunk` - first slice landed**
   - Include chunk position, completed status, optional block sections, biome container, heightmaps, structure starts, structure references, carving masks, postprocessing offsets, proto tick lists, generated light positions, dirty/unsaved flags, and content-version metadata.
   - Allow metadata-only records with no materialized sections.
   - Keep a clear conversion boundary to full/published chunk data.
   - Landed so far: position, status, section presence, unsaved flag, content version, proto-to-full conversion marker.

2. **Make status tasks mutate the proto record - first slice landed**
   - `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES` remain no-op or placeholder metadata until structures land, but they should advance real proto status.
   - Terrain statuses fill sections only when the status requires sections.
   - `FEATURES` writes into the target 3x3 through a `WorldGenRegion`-style view while preserving deterministic commit order.
   - Landed so far: status tasks and full promotion update `GeneratedChunkHolder.chunkToSave`; terrain/features still write block data through `GeneratedRenderLevel` until the full proto-section move lands.

3. **Add status-shaped storage**
   - Load stored records at `EMPTY` before generating missing statuses.
   - Save generated partial records with status and content version.
   - Treat generated-clean records as discardable cache and dirty/user-mutated records as durable state.
   - Keep packed client snapshots as derived cache after the proto/full state is authoritative, using the Tactical 59 lazy generated-cache queue.

4. **Wire unload and save policy - partly covered by Tactical 59**
   - Add a `chunkToSave`-style latest partial/full reference to the generated holder.
   - Save unsaved partials on unload or host flush.
   - Do not delete partial records merely because a chunk left the current visual square if the storage policy says vanilla would save or keep it pending.
   - Landed so far: holder reference exists in memory. Saving unsaved partial records remains open.

5. **Version and invalidate cache**
   - Store generator profile, seed, dimension, content version, and lighting/status version.
   - On mismatch, discard deterministic generated cache but preserve dirty durable state according to the save policy.

## Non-goals

- Implementing actual structures.
- Matching Mojang Anvil/NBT bytes.
- Trusting hydrated light before the lighting docs define the version/trust policy.
- Reducing the normal-publication status closure below the vanilla gates.

## Tests

Add focused tests for:

- a metadata-only `STRUCTURE_STARTS` or `STRUCTURE_REFERENCES` record has no block sections and still satisfies metadata dependencies - first coverage exists through the chunk-crossing holder access counts
- a chunk saved at `LIQUID_CARVERS` reloads and resumes at `FEATURES` without rerunning terrain
- a chunk saved at `FEATURES` reloads and can advance to `FULL` without rerunning decoration
- dirty/user-mutated chunks are not discarded as generated cache on content-version mismatch
- route changes preserve or save partial status progress instead of forcing a cold rerun
- packed published snapshots are rebuilt from stored proto/full state when needed

## Validation

Required:

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts` - first in-memory slice passes
- focused storage tests for partial save/load/resume
- `pnpm perf:worldgen:flyby -- --preset flat_grass --radius 1`
- `pnpm perf:worldgen:flyby -- --preset default --radius 1`
- `pnpm typecheck`
- `git diff --check`
