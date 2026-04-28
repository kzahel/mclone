# Tactical 57 - Generated chunk-holder status futures

Replace the generated host's view-level status batching with per-chunk, per-status request coalescing shaped like vanilla `ChunkHolder`.

Status: partial. The first behavior-preserving coalescing slice is implemented around the existing host phases. Recursive `ensureStatus(...)` scheduling is still the follow-up before Tactical 57 is complete.

## Goal

Make status requests durable enough in memory that walking across a chunk boundary does not duplicate work:

- one holder record per generated chunk key while the chunk is resident or pending unload
- one pending/completed job slot per `ChunkStatus`
- repeated requests return the existing pending/completed status result
- view changes reprioritize/cancel future work without rerunning a status already in flight for the same chunk
- instrumentation reports requested, coalesced, reused, generated, canceled, and failed counts by status

This tactical does not try to reduce the vanilla-shaped closure for normal publication. For the current 5x5 normal publication square, the acceptable warm one-chunk move still creates `5` published snapshots, `7` `FULL`, `9` `FEATURES`, `11` materialized terrain chunks, and `25` metadata-status records if each status is needed once.

## Landed first slice

The generated host now has a `GeneratedChunkHolder` table with:

- a coalesced preload/load future, corresponding to vanilla's `EMPTY` load side of the status graph
- per-status job slots for `LIQUID_CARVERS`, `FEATURES`, and no-light `FULL` promotion
- worldgen counters for status requests, pending coalesces, completed reuses, job starts, completions, skips, and failures

The current implementation deliberately keeps the existing view-level job collection underneath this holder layer. That means overlapping cooperative view jobs no longer duplicate completed preload work and same-status jobs have a place to coalesce, but the scheduler still has not been inverted into a recursive `ensureStatus(pos, status)` graph.

The focused test [`generated-world-host-chunk-crossing.test.ts`](../../test/runtime/generated-world-host-chunk-crossing.test.ts) now covers both:

- settled radius-0 crossing counts: `5` publish, `7` full, `9` features, `11` terrain, and no duplicate terrain/decor calls
- a superseded cooperative nearby view where preload results are reused and terrain/decor calls remain unique across overlapping jobs

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

1. **Introduce `GeneratedChunkHolder`** - first slice landed
   - Store chunk key, completed status, current partial/full chunk data reference, ticket/interest metadata, and one status job slot per status.
   - Keep holder lifetime separate from the current visible snapshot set.
   - Keep existing status records during migration, but make holder state the authoritative runtime source.

2. **Add recursive `ensureStatus(...)`** - remaining
   - Implement parent recursion and dependency gathering from `ChunkMap.getDependencyStatus(...)`.
   - Store the pending job before awaiting dependencies so duplicate calls coalesce.
   - Return mixed-status chunk records to status tasks instead of building a flattened view batch.

3. **Wrap existing stage methods** - first slice partly landed
   - Start by routing existing terrain, features, full, lighting-disabled, and snapshot paths through status requests.
   - Preserve current status order and the exact flat-world crossing counts.
   - Leave real partial persistence to Tactical 58, but expose the hook where `chunkToSave` will attach.

4. **Replace view-level job collection** - remaining
   - Convert chunk-view changes into status requests for the chunks that must become publishable.
   - Let dependency requests fan out through `ensureStatus(...)`.
   - Keep cooperative yields by status quantum, not by a synthetic "terrain all, then features all" phase.

5. **Instrument coalescing** - first slice landed
   - Count status requests, pending coalesces, completed reuses, generated executions, cancellations, failures, and unload skips.
   - Include the counters in `world_perf` and host fly-by reports.

## Tests

Add focused tests for:

- two same-turn `ensureStatus(chunk, FEATURES)` calls schedule one status job
- a repeated settled view schedules no new terrain/features/full/snapshot work
- the flat-world one-chunk crossing preserves the expected `5/7/9/11/25` warm deltas
- a route reversal reuses retained holder status results while the holder is still resident
- `FEATURES` dependencies at radii `2..8` remain metadata-only and do not request terrain statuses
- cancellation of a superseded view does not discard an already-completed status future

## Validation

Required:

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts`
- `pnpm vitest run test/runtime/generated-world-host-scheduler.test.ts test/world/level/generated-chunk-status.test.ts`
- `pnpm perf:worldgen:flyby -- --preset flat_grass --radius 1`
- `pnpm typecheck`
- `git diff --check`

Browser validation is only needed if this slice changes the visible publish path or render-facing snapshot shape.
