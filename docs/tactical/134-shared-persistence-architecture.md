# 134: Shared Persistence Architecture

Status: active; Slice 1 landed, Slice 2 next.

## Purpose

Turn persistence from a chunk-only synchronous scaffold into a shared
host-owned world-storage system that can support chunks, entity chunks, player
data, saved data, browser IndexedDB, native/Android durable storage, and
explicit transient worlds.

This is shared implementation, desktop validation first. App crates may choose
world roots, browser storage handles, Android app-private paths, and lifecycle
entrypoints. They must not own gameplay persistence policy.

Durable architecture lives in
[`../persistence-architecture.md`](../persistence-architecture.md).

## Reference Source Read

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/RegionFileStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/EntityStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/DimensionDataStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/PlayerDataStorage.java`

## Current Native State

Live Rust after Slice 1 has:

- `native/crates/mclone-server/src/persistence.rs`
- `ChunkSnapshotStore`
- `NullChunkSnapshotStore`
- `FilesystemChunkSnapshotStore`
- `WorldStore`
- `ChunkRecord`
- `PersistenceActor` / `PersistenceMailbox`
- `NullWorldStore` / `MemoryWorldStore`
- `ChunkSnapshotWorldStore`
- `SynchronousPersistenceFacade`
- synchronous `load_chunk` / `save_chunk`
- `ChunkScheduler` dirty chunk tracking and save-before-unload for current block
  mutations

This is useful but still too narrow:

- actor execution is still same-thread/in-process
- the scheduler still reaches persistence through a blocking compatibility
  facade
- dirty/unload holder state does not yet wait on actor acknowledgements
- no entity chunk storage
- no player/world/saved-data records
- no web IndexedDB adapter
- no Android or dedicated-server world-dir wiring
- no clear durable-vs-cache priority at the actor/backend boundary

`docs/loading-persistence.md` contains richer target vocabulary than the live
Rust path. Treat live Rust as the source of current behavior and reconcile the
docs as part of this workstream.

## Target Shape

```text
mclone-server authoritative host
  chunks, entities, players, ticks, dirty flags, save policy

shared persistence actor
  request queue, pending writes, encode/decode, flush/close, world-scoped
  lifetime

WorldStore backend
  memory/null, SQLite/file, IndexedDB, Android app-private storage
```

The host should enqueue persistence work and integrate completed results. It
should not block input, chunk-view updates, or host ticks on slow storage.

## Scope Boundaries

In scope:

- shared persistence contract
- actor/mailbox semantics
- memory/null backends
- adapter shim for current chunk snapshot storage
- explicit transient-world mode
- entity chunk logical records
- dirty entity save/load/unload semantics
- desktop/dedicated world-dir wiring
- browser IndexedDB adapter after the contract lands
- Android app-private storage adapter after the native contract lands

Out of scope for the first slice:

- literal Anvil `.mca` compatibility
- full DataFixer migration
- block entities beyond record-slot preparation
- inventories beyond player-record slot preparation
- saved light hydration as trusted authoritative state
- natural spawning enablement before entity persistence works

## Implementation Plan

### Slice 0: Documentation Reconciliation

Status: this document.

Deliverables:

- architecture proposal in `docs/persistence-architecture.md`
- tactical tracker in this file
- README/tactical index links
- a short note in `loading-persistence.md` pointing to the broader target if
  needed by follow-up cleanup

### Slice 1: Shared Contract And Actor Skeleton

Status: landed 2026-07-03.

Add a storage-facing contract without changing physical storage yet.

The contract is completion-based, not blocking: request and completion enums
are the shared surface, because web/IndexedDB cannot serve a blocking call
from inside the host worker. Blocking convenience wrappers are native-only
helpers. The scheduler keeps calling through a synchronous facade in this
slice; async load integration through the holder state machine is Slice 2.

Actor lifetime equals world lifetime: the actor is created at world open and
torn down at world close, so there is no session-epoch protocol. Dropping load
results after chunk interest changes is host bookkeeping in Slice 2.

Landed shape:

- add the `WorldStore` request/completion contract in `mclone-server`; extract
  a `mclone-persistence` crate only when a non-server consumer appears
- define request/completion enums for chunk records first, with reserved
  variants for entity/player/saved-data records
- add `PersistenceActor` / `PersistenceMailbox` with:
  - pending same-key write map
  - per-key write ordering
  - revision-based write precedence with durable tie-break
  - cache vs durable save priority lanes
  - `load_chunk`
  - `save_chunk`
  - `flush`
  - `close`
- implement `NullWorldStore`
- implement `MemoryWorldStore`
- adapt current `ChunkSnapshotStore` as a compatibility backend or shim
- add the light algorithm/version stamp to the chunk record while the format
  change is cheap, per the light policy in `persistence-architecture.md`

Implementation notes:

- `ChunkScheduler` now owns `SynchronousPersistenceFacade`; existing callers can
  still pass `Box<dyn ChunkSnapshotStore>` through `ChunkScheduler::with_store`
  and `IntegratedServer::with_chunk_store`.
- `FilesystemChunkSnapshotStore` remains the compatibility file scaffold and
  also implements `WorldStore`.
- The binary snapshot shim now writes format v4 with an optional
  `light_algorithm_version` on `ChunkRecord`; legacy v1-v3 files still read.
- Reserved entity/player/saved-data request keys exist in the public contract,
  but they fail explicitly until their record families land.

Validation landed:

- unit tests for load-after-pending-write
- same-key newer-revision write supersedes stale queued write
- older-revision cache write cannot overwrite a queued higher-revision durable
  write; equal revisions resolve durable-over-cache
- durability affects priority lanes and shutdown-skip only, never write-wins
- flush waits for pending durable writes
- close drains required durable writes; post-close requests fail explicitly
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`

Exit criteria:

- `ChunkScheduler` can still load/save chunk snapshots through the new boundary
  via the synchronous facade
- transient mode is explicit
- no app crate learns storage policy

### Slice 2: Host Dirty/Unload Integration Through Actor

Move current scheduler save-before-unload behavior onto actor acknowledgements.

Deliverables:

- scheduler loads become async through the holder state machine, replacing the
  Slice 1 synchronous facade
- load results that complete after chunk interest goes away are dropped by
  host bookkeeping
- resident dirty holders stay alive while required durable saves are pending
- returning interest resurrects a pending-unload holder instead of reading stale
  storage
- foreground loads consult pending writes before backend loads
- generated-clean cache saves are distinguishable from durable dirty saves
- pending scheduled fluid ticks (and future block ticks) pack into chunk
  records on save and hydrate into host tick queues on load/promotion, instead
  of load-time settling
- storage errors surface through scheduler/runtime diagnostics

Validation:

- dirty block edit survives unload/reload through memory store
- clean generated cache miss regenerates instead of failing the world
- stale cache write cannot overwrite a later dirty block edit
- pending-unload holder is resurrected when interest returns before save ack
- pending fluid tick survives unload/reload and resumes instead of re-settling

### Slice 3: Entity Chunk Record Foundation

Add logical entity persistence while keeping physical storage in memory first.

Deliverables:

- `EntityChunkRecord`
- `EntitySaveRecord` with the field set and per-kind payloads from the
  provisional Entity Save Record V1 in Decisions
- stable UUID identity in saved records; runtime `EntityId` stays session-local
- chunk-addressed save/load API separate from block chunks
- empty entity chunk records
- dirty entity chunk tracking
- save-on-unload semantics
- generated original mobs enter the persistent entity path
- invariant: an existing entity chunk record (including empty) suppresses
  generation-time entity placement, even when the block chunk record was
  discarded as incompatible cache and the terrain regenerated

Validation:

- generated passive animal survives chunk unload/reload
- killed generated passive animal does not reappear from seed-time generation
- moved entity saves under the destination chunk
- empty entity chunk suppresses seed respawn ambiguity
- regenerating a discarded cache block chunk does not resurrect killed animals
  when an entity chunk record exists
- natural spawning remains gated unless persistent mode is ready

### Slice 4: Native Durable Backend And IO Thread

Add the first real durable native backend, and move the actor off the server
thread in the same slice. Landing real disk IO without the offload would
reintroduce exactly the blocking this design exists to avoid.

Deliverables beyond the backend:

- persistence actor runs on its own native thread behind the same
  request/completion contract
- host tick integrates completions without blocking on storage

Recommended first backend:

- one SQLite database per world via `rusqlite` (bundled feature), which
  cross-compiles cleanly to Android and Quest
- metadata table
- chunk blob table
- entity chunk blob table
- player table placeholder
- saved-data table placeholder
- WAL mode where supported
- schema version and compatibility checks

Alternative allowed if SQLite integration is blocked:

- filesystem records under a world root, still behind the same `WorldStore`
  contract; writes must be temp-file-then-atomic-rename (the current snapshot
  store's bare `File::create` corrupts a chunk on mid-write crash)

Validation:

- desktop native creates a named world dir/db
- chunk edit persists across process restart
- generated animal persists across process restart
- killed generated animal remains gone across process restart
- explicit transient world still discards all state
- a deliberately slow store fake does not inflate host tick time

### Slice 5: Dedicated Server World Dir

Wire durable persistence into the dedicated server app without changing the
shared policy.

Deliverables:

- `--world-dir` or equivalent startup arg
- world id/name selection
- explicit transient mode for tests
- save/flush on clean shutdown
- error reporting when a world cannot open

Validation:

- native dedicated smoke with persistent block edit
- native client reconnect sees saved state
- no renderer dependency in dedicated storage path

### Slice 6: Browser IndexedDB Backend

Add browser singleplayer storage behind the same logical contract.

Deliverables:

- IndexedDB object stores for metadata, chunks, entity chunks, players, saved data
- host-worker-owned adapter
- clear-world/debug reset path
- adapter lifetime equals world lifetime; world replacement tears down and
  recreates the adapter instead of epoch-stamping requests
- no main-thread chunk/entity decode

Validation:

- web smoke creates a world, edits a block, reloads page, sees edit
- generated animal survives page reload
- killed generated animal remains gone
- `clearWorldStorage=1` or successor path gives deterministic clean start
- remote web client remains unaffected by local IndexedDB

### Slice 7: Android App-Private Backend

Wire Android flat and Android XR through app-private storage, likely using the
same native SQLite backend as desktop.

Deliverables:

- app-local world root
- transient-vs-persistent launch mode
- save/flush lifecycle on pause/stop where practical
- no Android app-local gameplay policy

Validation:

- flat Android edit survives app restart
- Android XR local world edit survives app restart
- remote dedicated mode does not create accidental local authority saves

### Slice 8: World/Player/Saved-Data Expansion

Fill the non-chunk record families as gameplay needs them.

Deliverables:

- player position/spawn/hotbar record
- world time and spawn metadata
- inventory slots when real inventory lands
- saved-data key/value cache with dirty flags
- compatibility/migration story per record family

Validation:

- player resumes position in local world
- world time resumes
- inventory/hotbar persists after real item work lands

## First Good Implementation Chunk

Completed: **Slice 1: Shared Contract And Actor Skeleton**.

Reasoning:

- it is small enough to land without choosing SQLite or IndexedDB yet
- it preserves current chunk behavior while creating the async boundary needed
  by every platform
- it gives unit-testable semantics for pending writes, revision precedence,
  flush, and close before real IO makes failures harder to isolate
- it gives entity persistence a place to plug in next

Do not start with app wiring or a physical database. That would lock policy to
one platform before the shared host contract is clear.

## Next Likely Implementation Chunk

Proceed with **Slice 2: Host Dirty/Unload Integration Through Actor**.

Reasoning:

- Slice 1 has the shared contract, pending-write map, durability lanes, and
  synchronous compatibility facade.
- The remaining architectural risk is now scheduler residency: dirty holders
  should stay resident until durable save acknowledgements arrive, and async
  load completions must be integrated through holder state rather than direct
  blocking calls.
- This should land before entity chunk records or physical durable storage, so
  entity persistence and SQLite/IndexedDB do not inherit the current blocking
  save-before-unload behavior.

## Validation Gates

For contract-only slices:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
git diff --check
```

For any slice that changes rendered loaded state, run a native headless or
offscreen capture and inspect the image under `/tmp`.

For browser persistence slices:

```text
pnpm native:web:build
pnpm native:web:smoke
```

For durable native backend slices, add process-restart tests or smokes rather
than only same-process memory tests.

## Decisions

- The contract is completion-based request/completion enums; blocking wrappers
  are native-only helpers.
- Write precedence is revision-based with a durable tie-break; durability only
  controls priority lanes and may-skip-on-shutdown.
- Actor lifetime equals world lifetime; no session-epoch protocol. Stale load
  results after chunk interest changes are dropped by host bookkeeping.
- Shared code lives inside `mclone-server` until a consumer that does not
  already depend on it appears; extracting `mclone-persistence` later is cheap.
- The existing filesystem snapshot store is wrapped only as the Slice 1
  compatibility shim. Slice 4 goes straight to SQLite; do not invest in
  hardening the snapshot-file format.
- One SQLite database per world on all native/Android platforms. A small
  dump/inspect CLI is cheaper than maintaining a second filesystem backend for
  manual inspection.
- Metadata compatibility defaults are recorded in
  [`../persistence-architecture.md`](../persistence-architecture.md) under
  Versioning And Migration: cache records may be dropped with a log line;
  durable records never drop silently — they migrate or fail open with an
  explicit user-facing reset choice.

### Provisional: Entity Save Record V1

Discussed defaults so entity slices do not improvise; revisit at Slice 3 if
implementation contradicts them.

- common fields: persistent UUID, stable kind code, position, y/x rotation,
  delta movement, on-ground, `age_ticks`
- the runtime `EntityId` is never persisted; a fresh session id is assigned on
  load (vanilla saves the UUID, never the int id)
- kind is written as an explicit stable code, never the `EntityKind` enum
  ordinal, so reordering variants cannot corrupt saves
- per-kind payloads: Chicken `{ egg_time }` (vanilla `EggLayTime`); Item
  `{ stack, pickup_delay }` with despawn age covered by common `age_ticks`;
  Cow `{}` until love/breeding exists; Sheep later `{ sheared, color }`
- not saved yet: health, fire, air, fall distance — those systems do not exist
  in the native runtime, and the entity codec version covers adding them later
- `pending_egg_lays` is a within-tick handoff: drained the same tick, never
  persisted
- sheep eating state is not saved, matching vanilla: `eatAnimationTick` is
  transient and absent from `Sheep.addAdditionalSaveData`
- persistence categories: Cow, Chicken, Item persist; `DebugCube` is
  permanently volatile
