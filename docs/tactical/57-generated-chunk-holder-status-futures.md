# Tactical 57 - Generated chunk-holder status futures

Replace the generated host's view-level status batching with per-chunk, per-status request coalescing shaped like vanilla `ChunkHolder`.

Status: implemented. The generated host now has holder-owned status jobs plus recursive `ensureStatus(...)` scheduling for the current terrain, features, and no-light full paths. Partial `ProtoChunk` data and save/resume remain Tactical 58.

## Goal

Make status requests durable enough in memory that walking across a chunk boundary does not duplicate work:

- one holder record per generated chunk key while the chunk is resident or pending unload
- one pending/completed job slot per `ChunkStatus`
- repeated requests return the existing pending/completed status result
- view changes reprioritize/cancel future work without rerunning a status already in flight for the same chunk
- instrumentation reports requested, coalesced, reused, generated, canceled, and failed counts by status

This tactical does not try to reduce the vanilla-shaped closure for normal publication. For the current 5x5 normal publication square, the acceptable warm one-chunk move still creates `5` published snapshots, `7` `FULL`, `9` `FEATURES`, `11` materialized terrain chunks, and `25` metadata-status records if each status is needed once.

## Landed

The generated host now has a `GeneratedChunkHolder` table with:

- a coalesced preload/load future, corresponding to vanilla's `EMPTY` load side of the status graph
- per-status job slots for `LIQUID_CARVERS`, `FEATURES`, and no-light `FULL` promotion
- recursive status requests that gather `FEATURES` dependencies through `getGeneratedChunkDependencyStatus(...)`
- metadata-only `STRUCTURE_STARTS`, `STRUCTURE_REFERENCES`, and `BIOMES` advancement for outer dependency rings
- chunk-view scheduling expressed as current-view status targets instead of a flattened preload/terrain/features job list
- worldgen counters for status requests, pending coalesces, completed reuses, job starts, completions, skips, and failures

The implementation still stores generated terrain/features in the existing `GeneratedRenderLevel` records. It does not persist partial proto-style status data, carving masks, structure starts/references, or `chunkToSave` state; that is the Tactical 58 storage slice.

The focused test [`generated-world-host-chunk-crossing.test.ts`](../../test/runtime/generated-world-host-chunk-crossing.test.ts) now covers both:

- settled radius-0 crossing counts: `5` publish, `7` full, `9` features, `11` terrain, and no duplicate terrain/decor calls
- repeated cooperative requests for the same view where pending `FEATURES` status work coalesces and terrain/decor calls remain unique

## Source files

Read these before writing code:

| Source | Purpose |
|---|---|
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | per-status futures, full/ticking futures, `chunkToSave` update hook |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | `schedule(...)`, `scheduleChunkGeneration(...)`, `getChunkRangeFuture(...)`, `getDependencyStatus(...)` |
| [`ServerChunkCache.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java) | hot read cache and calls into `ChunkHolder.getOrScheduleFuture(...)` |
| [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) | status chain, range values, and `STATUS_BY_RANGE` |
| [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) | canonical status dependency and publication count contract |
| [`../../src/runtime/host/generated-world-host.ts`](../../src/runtime/host/generated-world-host.ts) | current cooperative chunk-view job runner |
| [`../../src/world/level/generated-render-level.ts`](../../src/world/level/generated-render-level.ts) | current status records and generated chunk storage |
| [`../../test/runtime/generated-world-host-chunk-crossing.test.ts`](../../test/runtime/generated-world-host-chunk-crossing.test.ts) | current flat-world chunk-crossing counter test |

## Vanilla comparison

Vanilla has a small `ServerChunkCache` four-entry hot read cache, but that is not the main generation reuse mechanism. The important coalescing happens in `ChunkHolder`: each holder owns an `AtomicReferenceArray` of status futures, and `getOrScheduleFuture(status, chunkMap)` returns the existing future unless it is absent or already represents an unloaded/failure result.

The mclone target is the same semantic shape in TypeScript:

```text
ensureStatus(pos, status)
  -> get/create GeneratedChunkHolder(pos)
  -> if holder.statusJobs[status] is pending/completed, return it
  -> request parent/dependencies
  -> schedule one cooperative status job
  -> store the pending job before running it
```

The scheduler may stay cooperative and browser/Node-owned. The request graph, dependency statuses, and coalescing behavior should match vanilla.

## Implementation plan

1. **Introduce `GeneratedChunkHolder`** - landed
   - Store chunk key, preload/load job state, and one status job slot per generated status.
   - Keep holder lifetime separate from the current visible snapshot set.
   - Keep existing status and chunk data records during migration; Tactical 58 will move partial/full chunk data into a proto-shaped holder record.

2. **Add recursive `ensureStatus(...)`** - landed
   - Implement parent recursion and dependency gathering from `ChunkMap.getDependencyStatus(...)`.
   - Store the pending job before awaiting dependencies so duplicate calls coalesce.
   - Request mixed dependency statuses instead of building a flattened view batch.

3. **Wrap existing stage methods** - landed for current generated stages
   - Start by routing existing terrain, features, full, lighting-disabled, and snapshot paths through status requests.
   - Preserve current status order and the exact flat-world crossing counts.
   - Leave real partial persistence to Tactical 58, but expose the hook where `chunkToSave` will attach.

4. **Replace view-level job collection** - landed
   - Convert chunk-view changes into status requests for the chunks that must become publishable.
   - Let dependency requests fan out through `ensureStatus(...)`.
   - Keep cooperative yields by status quantum, not by a synthetic "terrain all, then features all" phase.

5. **Instrument coalescing** - landed
   - Count status requests, pending coalesces, completed reuses, generated executions, cancellations, failures, and unload skips.
   - Include the counters in `world_perf` and host fly-by reports.

## Tests

Focused coverage:

- a repeated settled view schedules no new terrain/features/full/snapshot work
- the flat-world one-chunk crossing preserves the expected `5/7/9/11/25` warm deltas
- repeated cooperative requests for the same view coalesce pending `FEATURES` status work
- `FEATURES` dependencies at radii `2..8` remain metadata-only and do not request terrain statuses
- scheduler progress still streams snapshots through the cooperative path and drops stale snapshots after a newer view

## Validation

Required:

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts`
- `pnpm vitest run test/runtime/generated-world-host-scheduler.test.ts test/world/level/generated-chunk-status.test.ts`
- `pnpm perf:worldgen:flyby -- --preset flat_grass --radius 1`
- `pnpm typecheck`
- `git diff --check`

Browser validation is only needed if this slice changes the visible publish path or render-facing snapshot shape.
