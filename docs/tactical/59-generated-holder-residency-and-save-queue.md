# Tactical 59 - Generated holder residency and save queue

Make generated chunk residency and storage IO closer to vanilla `ChunkMap`: memory-first holder reuse, bounded holder eviction, dirty-gated durable saves, and lazy discardable generated-cache writes.

Status: holder residency gauges, named player/generation residency-ticket sources, budgeted holder unloads, pending-unload resurrection, and keyed storage side effects have landed. Generated-clean publish cache writes are queued instead of blocking publication; dirty durable saves still run before the host forgets dirty chunk data.

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

The generated host now has a small ticket-set primitive and installs separate `player_view` and `generation_dependency` tickets. The generation ticket still covers the same deterministic authority square used before, intentionally large enough for `FULL`, `LIGHT`, and `FEATURES` dependencies. That is acceptable until the remaining runtime subsystems own their ticket sources, but holder lifetime is now bounded by ticketed residency rather than ad hoc radius checks.

Landed so far:

- `GeneratedWorldHost` queues `GeneratedChunkHolder` records outside the current authority window, processes normal unload passes with a 200-holder budget, and drains that queue during explicit flush.
- `GeneratedChunkTicketSet` records named square tickets; host holder residency and `GeneratedRenderLevel` authority checks now go through `player_view` and `generation_dependency` tickets.
- Worldgen perf counters report resident holder counts, unload-queue backlog, processed/cancelled holder drops, pending unloads, and storage side-effect depth/activity.
- Storage side effects are keyed by chunk coordinate and run with bounded global concurrency.
- Generated-clean cache writes from publish are queued lazily.
- Dirty durable saves still run before dirty chunk data is discarded.
- Queued generated-clean cache writes carry a per-chunk version and skip if a later dirty/save-worthy write supersedes them.
- Tactical 58 now gives resident holders a proto/full `chunkToSave` access record and saves those generated records on host flush or holder pruning.
- Tactical [`60`](60-generated-unload-and-storage-race-safety.md) owns the race-safety follow-up: pending unload holder resurrection, stale generated-record write protection, storage epochs, and ticket-shaped unload queues.

## Remaining work

1. **Ticket-shaped residency**
   - Landed: visible player interest and hidden generation dependency interest are separate `player_view` and `generation_dependency` tickets.
   - Remaining: add explicit lighting, entity, and future forced-chunk tickets.
   - Keep the existing radius helpers as derived policy until each runtime subsystem owns its ticket source.

2. **Unload queue parity**
   - Landed: holder drops use a budgeted unload queue similar to vanilla's `processUnloads(...)`.
   - Keep dirty-save-before-forget semantics for dirty chunks.
   - Remaining: drive the queue from multiple explicit ticket levels instead of only unioned square tickets.
   - Holders with queued generated-record saves stay in a pending-unload map and can be resurrected before the save reaches storage.

3. **Storage queue policy**
   - Landed: side effects are keyed by chunk coordinate, globally bounded, and visible to same-host preloads through a pending-write read-through map.
   - Add a public host/protocol flush or close operation so tests, Node hosts, and browser shutdown can wait for dirty saves deliberately.
   - Separate discardable generated-cache writes from durable dirty saves at the storage adapter boundary if the adapter needs different priorities.

4. **Integrate Tactical 58 - partial save/load landed**
   - Save `chunkToSave` proto/full generated chunk records on unload or explicit flush.
   - Save unsaved partial records on unload/flush; treat generated-clean partial records as versioned cache.
   - Remaining: move the unload trigger from authority-square pruning to future ticket levels and add adapter-level priority if generated-cache pressure becomes visible.

## Tests

Coverage should include:

- generated-clean publish does not wait for blocked storage writes
- dirty save before eviction still persists the dirty block
- holder count stays bounded after a route change
- queued stale generated-cache writes cannot overwrite a later dirty save
- storage side-effect queue counters expose depth, completion, and failures
