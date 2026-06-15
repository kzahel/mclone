# D8: Dirty / Cache Save Policy

This slice follows [`../loading-persistence.md`](../loading-persistence.md) and lands the first host-side distinction between discardable generated cache writes and dirty authoritative saves.

## Scope

Add:

- explicit host terminology for generated-clean cache writes versus dirty durable saves
- dirty marking for host-owned block mutations
- dirty-save-before-evict behavior for chunks leaving the authoritative view
- focused persistence coverage for a mutation that happens after initial publish and before unload

Do not add:

- full vanilla `ChunkStatus` persistence
- lazy cache eviction heuristics
- Anvil/region physical storage
- DataFixer-style migrations
- entity, player, inventory, block-entity, structure, heightmap, or carving-mask persistence
- a new public close/flush protocol
- trusted persisted-light hydration

## Reference Shape

Vanilla keeps chunk dirtiness as server state. A chunk save is not just a render-cache write: block mutations, scheduled ticks, entities, block entities, and other durable facts mark the chunk unsaved, and the server saves dirty chunks before unload/flush/close.

`mclone` still uses engine-native packed snapshots instead of Anvil NBT. Tactical 59 moved generated-clean publish cache writes onto a lazy host queue; the important D8 distinction remains that dirty mutations use durable-save semantics before eviction.

## Landed Shape

`GeneratedWorldHost` now maintains two separate dirty concepts:

- `dirtyDurableChunks`: chunk content changed and must be saved before the chunk can be forgotten.
- `dirtyChunksForPublication`: changed chunks that need a coarse replacement `chunk_snapshot` while the current protocol has no granular block delta.

Storage writes now flow through policy-named helpers:

- `cacheGeneratedChunkSnapshot(...)` writes a discardable generated-clean snapshot.
- `saveDirtyChunkSnapshot(...)` writes a dirty authoritative snapshot and clears the durable dirty marker after a successful write.
- `writeChunkSnapshotByPolicy(...)` chooses between those paths for normal publish/flush writes.
- `saveDirtyChunkBeforeUnload(...)` captures dirty chunk state before eviction.

The storage adapter interface is unchanged. Browser IndexedDB and Node/file storage still see `saveChunk(...)`; the host owns the policy above that adapter.

## Behavior

- Initial generated-clean chunks are queued as discardable cache writes.
- Host block mutations mark the owning chunk dirty.
- Dirty chunks flushed for publication are saved through the dirty path.
- Dirty chunks leaving the sync view are saved before `evictChunk(...)`.
- Dirty chunks leaving the cooperative view queue an async save-before-evict side effect, using a snapshot captured before runtime chunk state is discarded.
- Stored light remains omitted from storage records per D7; client snapshots may still include light.

## Validation

Added `GeneratedWorld persistence > saves dirty authoritative chunks before eviction`.

The test publishes a chunk, mutates it through the host-owned world level, moves the view far enough to unload that chunk before a tick flush, and then verifies the evicted storage record contains the mutation.

Validation run:

- `pnpm typecheck`
- `pnpm test -- test/runtime/generated-world-persistence.test.ts test/runtime/generated-world-host-liquid.test.ts test/runtime/generated-world-host-scheduler.test.ts`

## Remaining Gaps

- No public `closeWorld` / `flushWorld` command exists yet, so dirty-on-close parity is still deferred.
- Generated-clean cache writes are now lazy host side effects, but there is still no public flush/close protocol that matches vanilla save/stop semantics.
- There is still no persisted `ChunkStatus`, partial-generation resume, or status-aware save pipeline.
- Dirty state currently covers host block mutations and liquid-driven scheduled updates, not future entities/block entities/inventories.

## Next

The next loading/persistence slice should add a host flush/close lifecycle command if gameplay state starts accumulating outside chunk-view changes. If staying on worldgen/content next, the runtime docs no longer block returning to the underground feature path.
