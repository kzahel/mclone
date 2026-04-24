# D7: Loading Persistence Hygiene

This slice follows the loading/persistence research in [`../loading-persistence.md`](../loading-persistence.md). It is deliberately small: make the current model honest and debuggable before attempting vanilla-style chunk status, dirty saving, or save migration work.

## Scope

Add:

- honest chunk-loading progress stages that distinguish saved-chunk lookup from terrain generation
- explicit IndexedDB reset/clean-start support for browser dev and worker-backed browser tests
- a clear policy for light in persisted chunk records
- storage-version comments that say when `GENERATED_WORLD_STORAGE_VERSION` must change

Do not add:

- vanilla `ChunkStatus` persistence
- lazy dirty-chunk saving
- DataFixer-style migrations
- Anvil region files
- entity, block-entity, player, or structure persistence
- persisted-light hydration or trusted-light reuse
- renderer-owned persistence

## Reference Shape

Vanilla Minecraft Java does storage lookup before missing-status generation. A stored chunk is not discarded just because a player refreshes, and generation resumes only for missing or insufficient status data.

Vanilla also persists light, but only through a validity protocol:

- chunk NBT carries light section bytes
- `isLightOn` records whether those bytes can be trusted
- the light engine either queues trusted stored light or rescans emitters and relights

`mclone` does not have that persisted-light validity path yet. Current host hydration restores block sections and pending ticks, then recomputes lighting before publishing. That means stored light bytes are currently cache baggage, not loaded authoritative state.

## Current Problem

The code mostly does the right load/generate order, but the operator-facing state is misleading:

- cooperative loading reports "Generating terrain chunks" while it is also checking IndexedDB/file storage and hydrating saved chunks
- refreshes can look like worldgen even when saved chunks are being used
- worker-backed browser runs can reuse old IndexedDB data unless the browser context or database is explicitly cleaned
- persisted chunk records can contain light bytes even though reload ignores them
- the storage version comment does not clearly tell future authors when to invalidate generated-world saves

This creates the stale-cache concern: even when runtime behavior recomputes light, the saved payload still looks like durable light state.

## Target Shape For This Slice

### Progress Stages

Split cooperative chunk-view work into visible phases:

```text
Checking saved chunks
Generating missing chunks
Decorating new chunks
Computing light
Publishing chunks
```

The first phase should attempt storage hydration for every requested job and count hits/misses. The generation phase should only count chunks that missed storage. Decoration should only count chunks that still require decoration. Publishing should count chunks whose snapshots are sent to the client.

It is acceptable for "Computing light" to be coarse in this slice if the light engine does not expose per-chunk progress cleanly. The important part is that storage lookup is no longer mislabeled as terrain generation.

### Browser Storage Hygiene

Provide an explicit clean-start path for IndexedDB-backed worlds:

- a browser/debug query or equivalent control that clears `mclone-world-storage` before opening a worker-hosted world
- a Playwright helper or fixture hook for worker-backed tests/probes that need deterministic empty storage
- documentation in the helper naming that this clears browser local saves for the current origin

Prefer one shared helper over scattered `indexedDB.deleteDatabase(...)` calls. Tests that intentionally verify persistence should opt out and use stable save identity; tests that only need a fresh generated world should clear storage or use unique save identity.

### Persisted-Light Policy

For now, do not persist packed light sections in chunk storage records.

Network/client snapshots should still include light, because the renderer needs authoritative light facts. The storage write path should save the same chunk blocks and pending ticks but omit `light` until the host can hydrate and validate persisted light with a vanilla-shaped validity rule.

This keeps reload semantics honest:

- stored blocks and pending ticks are durable state
- light is derived on open and publish
- changing the lighting algorithm does not leave old light bytes in durable records

If the implementation instead keeps persisted light, this slice must add an explicit light/content version to the persisted record and reset old records when it changes. The preferred tactical choice is omission because current hydrate already ignores light.

### Version Discipline

Update the comment near `GENERATED_WORLD_STORAGE_VERSION` so future changes know when to bump it. Bump the version when persisted generated-world meaning changes, including:

- terrain or decoration algorithms
- biome id meaning or biome source behavior
- block-state id encoding
- persisted tick schema
- any future trusted persisted-light schema
- save metadata compatibility rules

Do not bump for pure client rendering changes or wire-only protocol changes that do not alter stored chunk records.

## Implementation Notes

Likely touch points:

- [`GeneratedWorldHost`](../../src/runtime/host/generated-world-host.ts): split `runChunkViewJobs(...)` into lookup/generate/decorate/light/publish phases, and omit light before `chunkStorage.saveChunk(...)`
- [`world-messages`](../../src/runtime/protocol/world-messages.ts): add optional progress detail only if stage/count is not enough
- [`indexeddb-world-storage`](../../src/runtime/storage/indexeddb-world-storage.ts): export or centralize the database name if a shared browser reset helper needs it
- browser/debug bootstrap: consume the clean-start query/control before creating the worker host
- browser test fixtures/probes: use the shared reset helper for worker-backed runs that assume empty storage

Keep storage policy inside the host/storage layer. The renderer should not decide which chunk facts are durable.

## Validation

Run the smallest checks that exercise the changed path:

- `pnpm test`
- `pnpm test:browser`
- one worker-backed browser probe or debug run that verifies refresh progress says saved-chunk lookup instead of terrain generation for stored chunks

For the persisted-light policy, add a focused unit/integration assertion that the storage record written by the host does not contain `light`, while the client-published snapshot still does.

## Landed Shape

This slice is implemented.

- Cooperative chunk loading now reports `Checking saved chunks`, `Generating missing chunks`, `Decorating new chunks`, `Computing light`, and `Publishing chunks`.
- The saved-chunk phase reports stored/existing/missing detail.
- Client snapshots still include light; storage records omit `light`.
- `GENERATED_WORLD_STORAGE_VERSION` was bumped to invalidate older generated-world cache ids, and the code comment now states the bump rule.
- Browser and debug entrypoints accept `clearWorldStorage=1`, backed by the shared IndexedDB reset helper.
- Worker-backed browser probes that expect clean generated worlds pass `clearWorldStorage=1`.
- Cooperative publishing no longer waits for the storage write before queuing the snapshot to clients.
- Browser smoke navigation waits for page `load` and then waits on the explicit boot promise, rather than using `networkidle` while world polling is active.

Validation run:

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`

## Done When

- refreshing a stored browser world no longer reports all work as terrain generation
- deterministic worker-backed tests/probes can start from empty IndexedDB on demand
- stored chunk records no longer carry ignored light bytes, or they carry an explicit versioned validity policy
- the storage version comment explains the invalidation rule
- [`../loading-persistence.md`](../loading-persistence.md) remains the durable reference, and this doc stays tactical

## Next

After this hygiene pass, stale persisted light and misleading progress labels are handled. The follow-up dirty/cache terminology and dirty-save-before-evict slice landed in [`D8-dirty-cache-save-policy.md`](D8-dirty-cache-save-policy.md).
