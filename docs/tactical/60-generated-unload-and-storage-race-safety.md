# Tactical 60 - Generated unload and storage race safety

Make generated chunk unload/save/load ordering predictable like vanilla `ChunkMap`, so async persistence cannot cause duplicate generation, stale reloads, or older partial records overwriting newer chunk state.

Status: pending-unload resurrection and generated-record write-version slices landed. The generated host keeps pruned holders pending while their generated-record save is queued; if the same chunk is requested before that save completes, the holder is resurrected and its in-memory `chunkToSave` record is restored instead of reading stale storage or regenerating. Generated partial/full records also carry per-chunk `writeVersion` values so an older queued generated-record write cannot overwrite a newer full/dirty generated record.

## Vanilla source anchors

| Source | Purpose |
|---|---|
| [`ChunkMap.updateChunkScheduling(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | resurrects holders from `pendingUnloads` when tickets return |
| [`ChunkMap.processUnloads(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | moves holders through `toDrop`, `pendingUnloads`, and `unloadQueue` with budgets |
| [`ChunkMap.scheduleUnload(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | waits for `chunkToSave`; reschedules if `chunkToSave` changes before unload completes |
| [`ChunkHolder.updateChunkToSave(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | chains latest successful `ChunkAccess` futures into the save target |
| [`ChunkMap.save(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | dirty-gated save; avoids proto-over-existing-full cases |

## Race risks to close

1. **Read while save is pending**
   - Risk: a chunk leaves authority, a generated record save is queued, and the same chunk is requested before the save reaches storage.
   - Vanilla shape: the holder remains in `pendingUnloads` and can be resurrected.
   - Current first slice: `GeneratedWorldHost` keeps pending-unload holders until save completion and resurrects them on `getChunkHolder(...)`.

2. **Stale partial write after newer state**
   - Risk: an older generated partial save writes after a newer full/dirty state.
   - Current protection: generated partial records, generated-clean packed snapshot writes, and dirty saves share a per-chunk generated-record write-version stream. Storage adapters reject lower-version generated-record writes.
   - Remaining gap: packed snapshot bytes still rely on the host's generated-cache stale guards and the serialized queue; a later keyed storage queue should make same-chunk ordering explicit.

3. **Hydrate after interest changed**
   - Risk: async preload completes after the chunk is no longer in the current authority window.
   - Existing protection: cooperative chunk-view jobs use revisions around the outer loops.
   - Remaining gap: storage preload restore should re-check authority/revision after each async read before hydrating into `GeneratedRenderLevel`.

4. **Clear/reset while writes are queued**
   - Risk: a save reset or world clear runs while old queued writes are still alive, then those writes recreate old data.
   - Remaining gap: storage sessions need an epoch/generation token so old queued writes are rejected after clear/close/reopen.

5. **Global queue instead of per-chunk ordering**
   - Risk: the serialized global queue is safe but blunt; it can block unrelated chunks behind one slow write and does not encode same-key dependencies explicitly.
   - Remaining gap: introduce per-chunk storage operations or keyed barriers so loads wait for same-key pending saves without blocking unrelated chunks.

6. **Authority-square unload instead of ticket unload**
   - Risk: pruning is immediate and deterministic rather than ticket-level driven with budgets.
   - Remaining gap: move pending unloads onto a ticket-shaped unload queue with pass budgets and backlog counters.

## Landed first slice

- `GeneratedWorldHost` now tracks `pendingUnloadChunkHolders`.
- Pruning a holder outside the authority window queues its `chunkToSave` generated record and parks the holder until the queued save finishes.
- `getChunkHolder(...)` checks pending unloads first; a re-request cancels the pending unload, restores the generated record into the level, and reuses the same holder/status jobs.
- New counters: `chunk_holders_pending_unload_scheduled`, `chunk_holders_pending_unload_resurrected`, `chunk_holders_pending_unload_completed`, `chunk_holders_pending_unload_current`, and `chunk_holders_pending_unload_max`.
- Regression coverage blocks generated-record saves, moves away, immediately moves back, and asserts the original chunk is not regenerated while storage still has no saved generated record.

## Landed second slice

- `GeneratedChunkStorageRecord` now includes a per-chunk `writeVersion`.
- `GeneratedWorldHost` owns a `generatedChunkRecordWriteVersions` map and bumps it for generated partial saves, generated-clean full cache writes, and dirty full saves.
- Storage adapters use compare-and-skip semantics for generated records: `MemoryWorldStorage`, `FileWorldStorage`, and `IndexedDbWorldStorage` reject an incoming generated record when storage already has a higher `writeVersion`.
- Legacy generated records without `writeVersion` hydrate as version `0`, so the next host write advances them instead of treating old records as authoritative over new writes.
- Regression coverage writes a newer generated record, then an older generated record for the same chunk, and verifies storage keeps the newer status/version.

## Next implementation slices

1. Add authority/revision checks after generated-record and legacy snapshot preload reads.
2. Add a storage/session epoch to reject queued writes after clear/reset/close.
3. Split the global storage queue into keyed same-chunk ordering plus bounded global concurrency.
4. Replace authority-square pruning with a ticket-shaped pending unload queue and processing budget.

## Validation

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts`
- `pnpm vitest run test/runtime/generated-world-persistence.test.ts test/runtime/file-world-storage.test.ts`
- `pnpm typecheck`
- `git diff --check`
