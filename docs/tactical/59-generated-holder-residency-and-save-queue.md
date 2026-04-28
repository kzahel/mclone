# Tactical 59 - Generated holder residency and save queue

Make generated chunk residency and storage IO closer to vanilla `ChunkMap`: memory-first holder reuse, bounded holder eviction, dirty-gated durable saves, and lazy discardable generated-cache writes.

Status: first slice landed. The host now has holder residency gauges, prunes holders outside the authority window, and serializes storage side effects through a queue. Generated-clean publish cache writes are queued instead of blocking publication; dirty durable saves still run before the host forgets dirty chunk data.

## Source files

Read these before changing this area:

| Source | Purpose |
|---|---|
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | `pendingUnloads`, `unloadQueue`, `processUnloads(...)`, `scheduleUnload(...)`, `save(...)` |
| [`DistanceManager.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java) | ticket-driven chunk residency, player tickets, stale ticket purge |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | holder lifetime, status futures, `chunkToSave` |
| [`../loading-persistence.md`](../loading-persistence.md) | current storage delta and dirty/generated-cache terminology |
| [`58-generated-protochunk-partial-state-and-persistence.md`](58-generated-protochunk-partial-state-and-persistence.md) | proto-shaped partial state and `chunkToSave` follow-up |

## Vanilla comparison

Vanilla is not a simple chunk-count LRU. Residency comes from tickets and ticket levels. `ChunkMap` moves holders whose ticket level exceeds `MAX_CHUNK_DISTANCE` into `toDrop`, then `processUnloads(...)` moves them through `pendingUnloads` and an `unloadQueue`. The unload pass is throttled: normally up to `200` drops per pass, with backlog escape behavior above `2000`.

Saving is dirty-gated. `ChunkMap.save(ChunkAccess)` returns immediately if `!chunk.isUnsaved()`. A holder's `chunkToSave` future points at the latest generated `ChunkAccess`, and unload waits for that future before saving and releasing chunk state.

The useful parity target is therefore:

- keep hot status/partial work in memory while the holder is still ticketed/resident
- prune holders predictably when they leave the equivalent residency window
- serialize save/evict/light-removal side effects instead of starting unbounded writes
- never let generated-clean cache writes block normal chunk publication
- never let a stale generated-clean cache write overwrite a later dirty authoritative save

## Current `mclone` shape

The generated host still uses a deterministic authority square instead of vanilla tickets. For the current publication policy, that authority square is intentionally large enough for `FULL`, `LIGHT`, and `FEATURES` dependencies. That is acceptable until a real ticket manager exists, but holder lifetime must be bounded by the same authority window.

The first slice landed:

- `GeneratedWorldHost` prunes `GeneratedChunkHolder` records outside the current authority window.
- Worldgen perf counters report `chunk_holders_resident_current`, `chunk_holders_resident_max`, and `chunk_holders_pruned_outside_authority`.
- Storage side effects are serialized through a host queue with `storage_side_effects_*` and `storage_side_effect_queue_depth_*` counters.
- Generated-clean cache writes from publish are queued lazily.
- Dirty durable saves still run before dirty chunk data is discarded.
- Queued generated-clean cache writes carry a per-chunk version and skip if a later dirty/save-worthy write supersedes them.

## Remaining work

1. **Ticket-shaped residency**
   - Replace the single authority square with explicit host tickets for player view, lighting, generation, entities, and future forced chunks.
   - Keep the existing radius helpers as derived policy until tickets own all residency.

2. **Unload queue parity**
   - Add a real unload queue with per-tick/pass budgets similar to vanilla's `processUnloads(...)`.
   - Keep dirty-save-before-forget semantics for dirty chunks.
   - Add pressure/backlog counters and tests that route changes do not create unbounded holder growth.

3. **Storage queue policy**
   - Keep side effects serialized or otherwise bounded.
   - Add a public host/protocol flush or close operation so tests, Node hosts, and browser shutdown can wait for dirty saves deliberately.
   - Separate discardable generated-cache writes from durable dirty saves at the storage adapter boundary if the adapter needs different priorities.

4. **Integrate Tactical 58**
   - Move `chunkToSave` from packed snapshots to the proto/full generated chunk record.
   - Save unsaved partial records on unload/flush; treat generated-clean partial records as versioned cache.

## Tests

Coverage should include:

- generated-clean publish does not wait for blocked storage writes
- dirty save before eviction still persists the dirty block
- holder count stays bounded after a route change
- queued stale generated-cache writes cannot overwrite a later dirty save
- storage side-effect queue counters expose depth, completion, and failures
