# Tactical 60 - Generated unload and storage race safety

Make generated chunk unload/save/load ordering predictable like vanilla `ChunkMap`, so async persistence cannot cause duplicate generation, stale reloads, or older partial records overwriting newer chunk state.

Status: pending-unload resurrection, generated-record write-version, stale-preload rejection, storage-session epoch, keyed storage side-effect, budgeted holder-unload, pending-write read-through, first ticket-source slices, tracked/entity-ticking split, and holder-owned full-status entity updates landed. The generated host keeps pruned holders pending while their generated-record save is queued; if the same chunk is requested before that save completes, the holder is resurrected and its in-memory `chunkToSave` record is restored instead of reading stale storage or regenerating. Generated partial/full records also carry per-chunk `writeVersion` values so an older queued generated-record write cannot overwrite a newer full/dirty generated record. Async preloads now re-check chunk-view revision and authority before hydrating storage results. Storage adapters reject operations from superseded sessions after reopen/reset/close. Queued same-chunk side effects stay ordered without blocking unrelated chunk writes, and host preloads read through same-chunk pending writes before adapter loads. Holder drops now enter a `toDrop`-style queue and process with a vanilla-sized 200-holder normal budget before moving into pending unload/save. Host residency now checks named `player_view`, `entity`, and `generation_dependency` ticket sources instead of direct radius math, and generated holders own the full-status value used to update entity visibility/ticking.

## Vanilla source anchors

| Source | Purpose |
|---|---|
| [`ChunkMap.updateChunkScheduling(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | resurrects holders from `pendingUnloads` when tickets return |
| [`ChunkMap.processUnloads(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | moves holders through `toDrop`, `pendingUnloads`, and `unloadQueue` with budgets |
| [`ChunkMap.scheduleUnload(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | waits for `chunkToSave`; reschedules if `chunkToSave` changes before unload completes |
| [`ChunkHolder.updateChunkToSave(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | chains latest successful `ChunkAccess` futures into the save target |
| [`ChunkMap.save(...)`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | dirty-gated save; avoids proto-over-existing-full cases |
| [`IOWorker.pendingWrites`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java) | queued stores are visible to reads before region-file writes finish |

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
   - Current protection: `GeneratedWorldHost` now keys queued side effects by chunk coordinate and runs up to four independent keys concurrently. Same-key operations retain FIFO order. Pending generated-record and packed-snapshot writes are also registered in host memory and consulted before adapter reads, mirroring vanilla `IOWorker.pendingWrites`.
   - Remaining gap: pending-write visibility is currently host-local. That matches the single-authority host shape, but it is not a cross-process shared pending-write table.

6. **Authority-square unload instead of ticket unload**
   - Risk: pruning is immediate and deterministic rather than ticket-level driven with budgets.
   - Current protection: holders outside the current union of `player_view` and `generation_dependency` residency tickets enter a `chunkHolderUnloadQueue`; normal passes process 200 holders, backlog above 2000 bypasses the normal budget, and flush drains the queue.
   - Remaining gap: the input policy still lacks real ticket levels and independent lighting/forced tickets. The `entity` source is now split from `player_view`, and holders now own full status, but the split is still helper-derived rather than ticket-level-derived.

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

## Landed seventh slice

- `GeneratedWorldHost` now tracks `pendingStorageWrites` keyed by chunk coordinate.
- Generated-record saves, generated-clean cache writes, direct generated-clean snapshot saves, and dirty snapshot saves register their latest generated record and/or packed snapshot before the storage adapter write reaches disk/IndexedDB/memory.
- `preloadStoredChunk(...)` checks pending generated records before `loadGeneratedChunk(...)` and pending packed snapshots before `loadChunk(...)`.
- Pending generated-cache writes include a stale predicate, so a later dirty/save-worthy version stops old cache data from being served.
- Pending entries clear only if the completing write still owns the key, so an older queued write cannot clear a newer pending write.
- New counters: `storage_pending_writes_registered`, `storage_pending_writes_cleared`, `storage_pending_writes_current`, `storage_pending_writes_max`, `storage_pending_writes_dropped_stale`, `storage_pending_writes_read_generated_chunk`, `storage_pending_writes_read_chunk`, `storage_preload_loaded_pending_generated_chunk`, and `storage_preload_loaded_pending_chunk`.
- Regression coverage blocks the first generated-cache save, removes that chunk from the in-memory level, then verifies preload restores it from the host pending write without calling adapter `loadGeneratedChunk(...)` or `loadChunk(...)`.

## Landed eighth slice

- Added `GeneratedChunkTicketSet`, a small named square-ticket collection that can hold multiple sources without changing unload/storage policy again.
- `GeneratedWorldHost` now installs a `generation_dependency` residency ticket for the current view and uses that ticket set for holder authority checks.
- `GeneratedRenderLevel` uses the same ticket primitive for its larger internal authority window, preserving structure-reference headroom while making the source explicit.
- New counters: `chunk_residency_tickets_updated`, `chunk_residency_tickets_current`, `chunk_residency_ticketed_chunks_current`, and `chunk_residency_ticketed_chunks_max`.
- Regression coverage verifies the radius-0 flat-grass view exposes one `generation_dependency` ticket covering 625 resident holder chunks, and that a one-chunk walk updates the ticket without changing the expected `5/7/9/11` generation deltas.

## Landed ninth slice

- `GeneratedWorldHost` now installs a `player_view` ticket for the published 5x5 radius-0 grid in addition to the hidden `generation_dependency` ticket.
- `GeneratedRenderLevel` also records both sources, so visible publication interest and hidden generation-retention interest can diverge in later slices.
- `GeneratedChunkTicketSet` now reports covered chunk count as a union, so overlapping ticket counters remain at 625 resident chunks for the radius-0 flat-grass case instead of summing 625 + 25.
- Regression coverage verifies the two ticket records and preserves the existing one-chunk walk generation deltas.

## Landed tenth slice

- `GeneratedWorldHost` now installs an `entity` ticket for the current entity-ticking window.
- Entity chunk status is synchronized from `entity` and `player_view` ticket state into `PersistentEntitySectionManager` instead of being promoted from chunk publication.
- `GeneratedRenderLevel` records the same `entity` source in its authority ticket set, keeping visible, entity, and generation-dependency ownership explicit.
- New counters: `entity_chunk_statuses_current`, `entity_chunk_statuses_ticking_current`, `entity_chunk_statuses_tracked_current`, and `entity_chunk_status_updates.<status>`.
- Regression coverage verifies radius-0 flat-grass still keeps 625 resident holder chunks, now with three explicit ticket sources, and that a one-chunk walk settles to the expected entity-status window.

## Landed eleventh slice

- The `entity` ticket now covers the two-ring interior of the published `player_view` ticket, matching vanilla's `viewDistance - 2` player-ticket shape.
- Radius-0 flat-grass still publishes a 5x5 `player_view`, but entity status now settles to 25 tracked chunks: 1 `ENTITY_TICKING` center chunk and 24 `BORDER` tracked-only chunks.
- `GeneratedRenderLevel` records the same smaller `entity` source so host and render-level authority debug data stay aligned.
- Regression coverage keeps the 5/7/9/11 chunk-generation walk counts unchanged and adds a visible-but-not-ticking moving-cow case for the border ring.

## Landed twelfth slice

- `GeneratedChunkHolder` now stores current `FullChunkStatus`, and entity manager updates are emitted from holder full-status transitions.
- Current holder full-status counters expose the radius-0 distribution as 625 holders: 600 `INACCESSIBLE`, 24 `BORDER`, and 1 `ENTITY_TICKING`.
- New transition coverage walks one chunk through `BORDER -> ENTITY_TICKING -> BORDER -> INACCESSIBLE` and verifies the entity status view follows the holder-owned state.
- This closes direct publication/radius promotion as the entity status owner; the remaining parity gap is replacing helper-derived statuses with vanilla-like ticket-level propagation.

## Next implementation slices

1. Add explicit lighting and forced-ticket sources, then start replacing derived radius helpers with subsystem-owned ticket updates and vanilla-like ticket-level propagation.
2. Add entity persistence adapters with pending-load/unload and pending-write read-through parity.
3. Add a public host/protocol flush or close acknowledgement so browser/Node shutdown can deliberately wait for dirty saves.
4. Consider a cross-tab/cross-process save lock for IndexedDB/file storage if we start supporting multiple authorities against the same save.

## Validation

- `pnpm vitest run test/runtime/generated-world-host-chunk-crossing.test.ts`
- `pnpm vitest run test/runtime/generated-world-host-entities.test.ts`
- `pnpm vitest run test/runtime/generated-world-persistence.test.ts test/runtime/file-world-storage.test.ts`
- `pnpm typecheck`
- `git diff --check`
