# Tactical 60 - Generated unload and storage race safety

Make generated chunk unload/save/load ordering predictable like vanilla `ChunkMap`, so async persistence cannot cause duplicate generation, stale reloads, or older partial records overwriting newer chunk state.

Status: pending-unload resurrection, generated-record write-version, stale-preload rejection, storage-session epoch, keyed storage side-effect, and budgeted holder-unload slices landed. The generated host keeps pruned holders pending while their generated-record save is queued; if the same chunk is requested before that save completes, the holder is resurrected and its in-memory `chunkToSave` record is restored instead of reading stale storage or regenerating. Generated partial/full records also carry per-chunk `writeVersion` values so an older queued generated-record write cannot overwrite a newer full/dirty generated record. Async preloads now re-check chunk-view revision and authority before hydrating storage results. Storage adapters reject operations from superseded sessions after reopen/reset/close. Queued same-chunk side effects stay ordered without blocking unrelated chunk writes. Holder drops now enter a `toDrop`-style queue and process with a vanilla-sized 200-holder normal budget before moving into pending unload/save.

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
   - Current protection: storage preload restore re-checks chunk-view revision and authority after generated-record reads and legacy packed snapshot reads before hydrating into `GeneratedRenderLevel`.
   - Remaining gap: status jobs still use a single holder-local pending slot per status; this is now revision-aware, but the future keyed queue should make same-chunk dependencies explicit outside generation status jobs.

4. **Clear/reset while writes are queued**
   - Risk: a save reset or world clear runs while old queued writes are still alive, then those writes recreate old data.
   - Current protection: memory, file, and IndexedDB storage adapters use per-save session epochs. Opening a save, resetting an incompatible save, or closing the current session supersedes older sessions; stale-session reads return missing and stale-session writes/evicts are skipped.
   - Remaining gap: this is process/adapter-local. It matches the local host lifecycle, but it is not yet a cross-tab/cross-process lock like vanilla's save-directory lock.

5. **Global queue instead of per-chunk ordering**
   - Risk: the serialized global queue is safe but blunt; it can block unrelated chunks behind one slow write and does not encode same-key dependencies explicitly.
   - Current protection: `GeneratedWorldHost` now keys queued side effects by chunk coordinate and runs up to four independent keys concurrently. Same-key operations retain FIFO order.
   - Remaining gap: adapter reads do not yet read through host-side pending write data the way vanilla `IOWorker.pendingWrites` does; current protection is ordering and stale/CAS guards, not full read-through.

6. **Authority-square unload instead of ticket unload**
   - Risk: pruning is immediate and deterministic rather than ticket-level driven with budgets.
   - Current protection: holders outside the derived authority window enter a `chunkHolderUnloadQueue`; normal passes process 200 holders, backlog above 2000 bypasses the normal budget, and flush drains the queue.
   - Remaining gap: the input policy is still a derived authority square rather than a real ticket graph with player, generation, lighting, entity, and forced tickets.

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

## Landed third slice

- `GeneratedWorldHost` captures the chunk-view revision for status jobs and preload jobs.
- Pending status/preload jobs coalesce only within the same chunk-view revision; a newer view does not wait on an obsolete pending job.
- `preloadStoredChunk(...)` returns `stale` instead of hydrating when the issuing revision is no longer current or the chunk left the current authority window.
- New counters: `storage_preload_skipped_stale`, `storage_preload_skipped_revision_changed`, `storage_preload_skipped_outside_authority`, `storage_preload_not_coalesced_revision_changed`, and `status_not_coalesced_revision_changed.<status>`.
- Regression coverage starts a flat-grass view, blocks its generated preload, walks one chunk over, verifies the old preload produces no stale chunk snapshots, then flushes storage and checks the settled status/access counts.

## Landed fourth slice

- `MemoryWorldStorage`, `FileWorldStorage`, and `IndexedDbWorldStorage` now assign an adapter-local session epoch for each opened save.
- Opening the same save again supersedes the previous session; resetting incompatible metadata also supersedes old sessions before clearing chunk records.
- Closing the current session advances the epoch, so late queued operations from that session become no-ops.
- Stale-session loads return missing; stale-session `saveChunk(...)`, `saveGeneratedChunk(...)`, and `evictChunk(...)` calls are skipped.
- Regression coverage verifies memory and file storage ignore writes from superseded sessions, including stale writes attempted after incompatible reset.

## Landed fifth slice

- `GeneratedWorldHost` replaced its single global `storageSideEffectQueue` with a keyed storage side-effect scheduler.
- Chunk cache writes, generated-record saves, evicts, dirty-before-evict work, and per-chunk light-removal messages use the chunk coordinate as their key.
- Tasks for the same key chain behind the previous same-key task; unrelated keys enter a ready queue and run with bounded concurrency (`STORAGE_SIDE_EFFECT_MAX_CONCURRENCY = 4`).
- `flushStorageSideEffects()` waits until all queued/running side effects drain, including side effects queued by side effects.
- New counters: `storage_side_effects_active_current`, `storage_side_effects_active_max`, `storage_side_effect_keys_current`, and `storage_side_effect_keys_max`.
- Regression coverage blocks the first generated-cache save, verifies another chunk cache save completes before the first one is released, then flushes and confirms all 25 published cache writes landed.

## Landed sixth slice

- `GeneratedWorldHost` replaced immediate holder deletion outside the current authority window with a `chunkHolderUnloadQueue`.
- Normal unload passes process up to `CHUNK_HOLDER_UNLOADS_PER_PASS = 200`, matching vanilla's normal `processUnloads(...)` drop budget; backlog above `2000` can keep draining.
- Holders that are queued but not yet processed remain resident and can be cancelled if interest returns. Cancellation restores the holder's `chunkToSave` record back into the generated level when a removed source chunk is available.
- Processed holders still move through the existing `pendingUnloadChunkHolders` save/resurrection path, so same-chunk return before save completion continues to reuse the holder.
- `flushStorageSideEffects()` drains the holder-unload queue before queuing resident generated-record saves and waiting for storage side effects.
- New counters: `chunk_holders_unload_queued`, `chunk_holders_unload_processed`, `chunk_holders_unload_cancelled`, `chunk_holders_unload_queue_current`, `chunk_holders_unload_queue_max`, `chunk_holders_unload_restored`, `chunk_holders_unload_restore_skipped`, `chunk_holders_unload_restore_rejected`, and `chunk_holders_unload_skipped_stale`.
- Regression coverage walks far enough to queue 625 holder drops, verifies only 200 process before settling, then flushes and verifies the expected 625 current holder records remain.

## Next implementation slices

1. Add host-side pending-write read-through for same-key loads if future load paths can overlap queued saves.
2. Replace the derived authority-square residency input with explicit ticket sources once generation, lighting, entities, forced chunks, and player interest can express independent tickets.
3. Consider a cross-tab/cross-process save lock for IndexedDB/file storage if we start supporting multiple authorities against the same save.

## Validation

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts`
- `pnpm vitest run test/runtime/generated-world-persistence.test.ts test/runtime/file-world-storage.test.ts`
- `pnpm typecheck`
- `git diff --check`
