# L6 - Lighting scheduler and status integration

Standing after [`L5-live-light-deltas.md`](L5-live-light-deltas.md), the worker-backed lighting optimization commit, and the chunk-status research in [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md).

Status: proposed next lighting tactical.

## Goal

Make initial lighting a first-class `LIGHT` status scheduling problem instead of a late whole-view service phase.

The current runtime keeps lighting opt-in because the worker-backed vanilla path is still too expensive for default use. The target here is to overlap and prioritize lighting so `lightingMode: "vanilla17"` can eventually become practical again: enqueue `LIGHT(C)` as soon as C has a completed `3x3 FEATURES` dependency, let the lighting worker make bounded progress, and publish each chunk after accepted light without waiting for unrelated chunks.

## Source files

Read these before changing the scheduler:

| Java source | Why it matters |
|---|---|
| [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) | `LIGHT` parent/range, `STATUS_BY_RANGE`, `isLighted(...)` |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | `schedule(...)`, `TicketType.LIGHT`, range futures, ticking/full publication |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | one future per status, full/ticking promotion, dirty light-section broadcast |
| [`ThreadedLevelLightEngine.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java) | chunk-priority light tasks, `PRE_UPDATE`, propagation, `POST_UPDATE`, light ticket release |
| [`ServerChunkCache.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java) | `onLightUpdate(...)` and `tryScheduleUpdate()` during task polling |

Durable docs to keep aligned:

- [`../lighting.md`](../lighting.md)
- [`../lighting-worker-architecture.md`](../lighting-worker-architecture.md)
- [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md)
- [`49-vanilla-status-futures-and-partial-chunks.md`](49-vanilla-status-futures-and-partial-chunks.md)

## Current Findings

Recent measurement with worker-backed lighting:

| Mode | radius 1 | radius 2 |
|---|---:|---:|
| lighting disabled | 1.59s | 2.26s |
| lighting enabled before optimization | 18.70s | 27.75s |
| lighting enabled after optimization | 5.41s | 8.02s |

The optimization removed the worst path: empty poll spam, repeated dependency initialization, and high allocation overhead inside propagation. The remaining cost is mostly real propagation plus the fact that the host starts lighting after collecting the current view's full-status chunks.

## Current Divergences

Vanilla:

- requests `LIGHT` through `ChunkHolder` status futures.
- adds a `TicketType.LIGHT` ticket while `LIGHT` is outstanding.
- schedules light tasks through `ChunkTaskPriorityQueueSorter` using chunk queue level.
- runs `PRE_UPDATE`, propagation, then `POST_UPDATE`.
- completes `lightChunk(...)` in `POST_UPDATE`, marks the chunk light-correct, releases the light ticket, and lets the status future advance.
- publishes chunk data and initial light together after the ticking/full send gate.

Current `mclone`:

- correctly requires `3x3 FEATURES` before initial lighting.
- uses a dedicated lighting worker and revisioned `chunk_light_ready` results.
- publishes chunks incrementally as light results arrive.
- still collects a set of chunks for a "Computing light" phase after decoration work for the current view, instead of enqueueing `LIGHT` as each chunk becomes dependency-ready.
- batches ready initial-light requests and runs propagation for the union, which is fast, but not yet shaped like vanilla's priority/status task queue.
- does not have a light-ticket equivalent beyond view revision and stale-result rejection.

## Scope

In scope:

- add explicit host-side `LIGHT` scheduling state per chunk-view revision.
- enqueue `LIGHT(C)` immediately when C's `3x3 FEATURES` dependency becomes ready.
- continue batching overlapping initial-light work, but bound batches by priority and budget.
- prioritize visible/near chunks over halo/cache-warming chunks.
- expose timing for `FEATURES ready`, `LIGHT queued`, `LIGHT accepted`, `FULL`, and `published`.
- make progress reporting distinguish generation, decoration, lighting queue, propagation, and publication.
- keep an explicit unlit mode available, but make the lit path performant enough that opting in is not punitive.

Out of scope:

- the full partial-`ProtoChunk` status future model from tactical 49.
- changing light propagation math.
- changing renderer packed-light consumption.
- trusting persisted light from storage beyond the current light-correct policy.

## Implementation Plan

1. **Add light scheduling records**
   - Track per chunk: current chunk revision, light input revision, `LIGHT` pending/queued/accepted state, and last dependency revision set.
   - Keep stale-result rejection by chunk-view revision and chunk revision.

2. **Schedule from `FEATURES` completion**
   - After each `FEATURES` job completes, inspect the affected 3x3 area.
   - For any chunk whose 3x3 neighborhood is now `FEATURES`-stable, upsert current light inputs and enqueue initial `LIGHT`.
   - Do not wait for all chunks in the publish ring to finish decoration before starting lighting.

3. **Keep batching, add priority**
   - Preserve the current worker-side union initialization win for adjacent chunks.
   - Bound each batch by visible priority, max command count, and max propagation budget.
   - Use `set_light_view` priority origin and publish radius to order ready work.

4. **Mirror vanilla task phases**
   - Treat chunk input hydration, section status, sky-source enablement, and emitter setup as `PRE_UPDATE`.
   - Treat live `checkBlock` repair and initial-light completion as `POST_UPDATE`.
   - Drain dirty light sections only after visible light data has swapped.

5. **Expose instrumentation**
   - Extend `world_perf` / traversal output with per-stage lighting counts and durations.
   - Record queue latency separately from propagation time.
   - Keep radius-1/radius-2 benchmark output comparable with the measurements above.

6. **Publish progressively**
   - Keep snapshots lit-first.
   - Publish a chunk as soon as its accepted light and publication gate are satisfied.
   - Do not let slower outer-ring light block already-ready near chunks.

## Tests

Add or extend focused tests for:

- a chunk cannot enqueue `LIGHT` before its `3x3 FEATURES` neighborhood is complete.
- completing one `FEATURES` job schedules only newly stable `LIGHT` candidates.
- a stale `chunk_light_ready` result cannot mark the chunk `FULL`.
- lighting starts before unrelated chunks in the same view finish decoration.
- worker batching preserves the same accepted light snapshots as individual requests.
- near chunks are requested before halo chunks when both are ready.
- live block-light batches still reject stale revisions after the scheduler change.

## Validation

Required:

```bash
pnpm test -- test/runtime/generated-world-boundary.test.ts test/runtime/generated-world-host-lighting-delta.test.ts test/runtime/lighting-worker-client.test.ts test/world/lighting/level-light-engine.test.ts
pnpm typecheck
pnpm -s perf:worldgen -- --radius 1 --direct-modes none --host-modes cooperative --lighting-mode vanilla17 --timeout-ms 300000
pnpm -s perf:worldgen -- --radius 2 --direct-modes none --host-modes cooperative --lighting-mode vanilla17 --timeout-ms 600000
pnpm test:browser
git diff --check
```

Run the no-light radius-1/radius-2 benchmark too when comparing ratios.

## Done When

- `LIGHT` is scheduled per dependency-ready chunk, not as an all-view phase.
- lighting progress and timing are visible without inferring from publication stalls.
- radius-1/radius-2 lighting-on generation time improves or clearly attributes the remaining cost to propagation.
- `lightingMode: "vanilla17"` remains practical for normal world loading.
- the docs above accurately describe any remaining divergence from vanilla scheduling.
