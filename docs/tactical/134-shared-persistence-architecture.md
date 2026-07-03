# 134: Shared Persistence Architecture

Status: completed first pass; shared persistence architecture, native SQLite
world-dir wiring, browser IndexedDB chunk/entity persistence, autosave/reload,
and async load-miss bridge landed. Follow-up world lifecycle/UI and non-chunk
record expansion continue in
[`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md).

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

Live Rust after Slice 6C has:

- `native/crates/mclone-server/src/persistence.rs`
- `ChunkSnapshotStore`
- `NullChunkSnapshotStore`
- `FilesystemChunkSnapshotStore`
- `WorldStore`
- `ChunkRecord`
- `EntityChunkRecord` / `EntitySaveRecord`
- `PersistenceActor` / `PersistenceMailbox`
- optional native-thread `PersistenceMailbox` backend for `Send` stores
- external-load `PersistenceMailbox` backend for host-owned async stores such
  as browser IndexedDB
- `NullWorldStore` / `MemoryWorldStore`
- native `SqliteWorldStore` for durable block/entity chunk records
- native `SqliteWorldStore::open_world_dir` using `world.sqlite3`
- `IntegratedServer::try_with_threaded_sqlite_world_dir`
- `ChunkSnapshotWorldStore`
- `SynchronousPersistenceFacade`
- synchronous `load_chunk` / `save_chunk`
- `ChunkScheduler` dirty chunk tracking and save-before-unload for current block
  mutations
- entity chunk load/save request and completion types in the shared mailbox
- entity-store pack/hydrate helpers for Cow, Chicken, and Item records
- `WorldStore::supports_entity_chunks()` so shared world stores can opt into
  entity records while snapshot-only compatibility stores remain chunk-only
- scheduler entity chunk load completions, dirty entity chunk saves, and
  save-before-holder-unload acknowledgement handling
- integrated-host entity record hydration, dirty entity chunk tracking, and
  chunk-local entity removal after holder unload
- `ChunkScheduler::close_persistence` and `IntegratedServer::shutdown_persistence`
  for clean host shutdown
- dedicated-server `--world-dir`, `--world-root` / `--world-name`, and
  `--transient` startup modes backed by threaded SQLite persistence
- native local-integrated runner storage selection through
  `NativeIntegratedServerWorldStorage`
- desktop native-client `--world-dir PATH` and explicit `--transient` startup
  modes for local integrated worlds
- browser worker `worldStorage=indexeddb` / `worldId=...` startup selection
- worker-owned IndexedDB object stores for chunk and entity chunk blobs
- wasm-visible shared chunk/entity record binary encode/decode helpers
- wasm `WorldStore` adapter for IndexedDB dirty-record mirrors and the preload
  compatibility fallback
- `WebChunkRenderSession.shutdownAsync()` for IndexedDB writeback completion
- browser IndexedDB dirty chunk/entity chunk autosave on command/tick responses
- browser IndexedDB chunk/entity chunk load misses serviced through explicit
  request/completion records instead of whole-world startup preload
- page-level browser block edit, IndexedDB write, reload, and exact block-state
  verification smoke coverage

This is useful but still too narrow:

- desktop local persistence is startup/CLI-wired but does not yet have an
  in-game world browser or named-world UI
- no player/world/saved-data records
- browser IndexedDB still writes through the current dirty-record response
  bridge instead of a fully external async save backend
- no Android app-private world-dir wiring

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
helpers. Slice 2 moved the scheduler to the mailbox path; the synchronous
facade remains only as compatibility/testing glue.

Actor lifetime equals world lifetime: the actor is created at world open and
torn down at world close, so there is no session-epoch protocol. Dropping load
results after chunk interest changes is host bookkeeping in the scheduler.

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

- `ChunkScheduler` now owns `PersistenceMailbox`; existing callers can still
  pass `Box<dyn ChunkSnapshotStore>` through `ChunkScheduler::with_store` and
  `IntegratedServer::with_chunk_store`, while `IntegratedServer::with_world_store`
  opens the shared record path directly.
- `FilesystemChunkSnapshotStore` remains the compatibility file scaffold and
  also implements `WorldStore`.
- The binary snapshot shim now writes format v5 with optional
  `light_algorithm_version` and scheduled tick arrays on `ChunkRecord`; legacy
  v1-v4 files still read.
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

Landed shape:

- `ChunkScheduler` schedules chunk loads through `PersistenceMailbox` and
  integrates completions during `poll`.
- load misses are batched back through ticket reconciliation before generation,
  preserving center-first worldgen/light behavior.
- load completions are ignored when interest has disappeared before the result
  is integrated.
- dirty holders queue durable saves and remain resident while a required save
  acknowledgement is pending.
- returning interest removes the pending-unload marker and continues using the
  resident holder, including dirty edits whose durable save has not completed.
- generated-clean publish paths queue discardable cache saves instead of marking
  generated chunks dirty.
- foreground loads see actor-pending same-chunk writes before backend reads via
  `PersistenceActor::load_chunk`.
- explicit `save_dirty_chunks()` flushes durable writes through the actor lane.
- scheduler/runtime callers receive persistence errors through the existing
  `ChunkStoreResult` path.
- `ChunkRecord` format v5 carries scheduled block/fluid tick arrays; older
  record versions read with empty tick arrays.
- generated feature/light cache records preserve generated block/fluid ticks, so
  the later light cache write does not supersede the feature cache write with an
  empty tick list.
- integrated-host durable saves pack live chunk-local block/fluid ticks with
  remaining delays, loaded records hydrate them back into host tick queues, and
  fully unloaded holders prune live queues after the record is already saved.
- `IntegratedServer::with_world_store` opens the shared record path directly;
  `with_chunk_store` remains snapshot-only compatibility glue.

Validation landed:

- dirty block edit survives unload/reload through memory store
- clean generated cache miss regenerates instead of failing the world
- stale cache write cannot overwrite a later dirty block edit
- pending-unload holder is resurrected when interest returns before save ack
- generated-clean cache writes are distinguishable from durable dirty saves in
  scheduler tests
- existing filesystem reload and fluid pending-unload tests were updated for
  actor acknowledgements
- generated light cache records keep generated block/fluid ticks
- loaded chunk records hydrate scheduled block/fluid ticks into
  `IntegratedServer`
- scheduled fluid ticks survive full unload/reload through file storage without
  manual rescheduling
- block tick list packing/removal and loaded block-tick hydration are covered by
  server tests
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`

Entity chunk load/save has moved into the scheduler holder lifecycle in Slice
3B; generated-original entity placement and tombstone suppression remain future
work because the current live natural spawning path is intentionally volatile.

### Slice 3: Entity Chunk Record Foundation

Add logical entity persistence while keeping physical storage in memory first.

Status: Slice 3A foundation landed 2026-07-03; Slice 3B scheduler/host
integration landed 2026-07-03.

Landed shape:

- `EntityChunkRecord`
- `EntitySaveRecord` with the field set and per-kind payloads from the
  provisional Entity Save Record V1 in Decisions
- stable UUID identity in saved records; runtime `EntityId` stays session-local
- chunk-addressed save/load API separate from block chunks
- empty entity chunk records
- `MemoryWorldStore` stores entity chunks; snapshot-only compatibility
  backends do not advertise entity chunk support, so existing snapshot-only
  worlds stay chunk-only instead of receiving entity record requests
- `PersistenceActor` applies pending-write visibility, revision replacement,
  durable priority, flush, and close semantics to entity chunk writes
- `ServerEntityStore` can pack/hydrate Cow, Chicken, and Item records with
  stable kind/item codes; Chicken saves egg time, Item saves stack and pickup
  delay, and fresh runtime ids preserve saved persistent ids
- `ChunkScheduler` schedules entity chunk loads for stores that advertise
  support, emits entity load completions, and includes entity record work in
  persistence pending counts
- `IntegratedServer` hydrates loaded entity records into the live entity store
  and tracking system
- persistent entity chunks are marked dirty when persistent entities spawn,
  move between chunks, update in place, or disappear
- pending-unload holders can queue one durable entity chunk save and remain
  resident until that save is acknowledged; holder unload then removes live
  chunk-local entities

Still deferred beyond Slice 3:

- generated original mobs enter the persistent entity path
- invariant: an existing entity chunk record, including empty, suppresses
  generation-time entity placement when generated-original entity placement
  exists
- regenerating a discarded cache block chunk must not resurrect killed generated
  animals when an entity chunk record exists

Validation landed:

- entity chunk actor load sees pending same-key writes
- newer entity chunk revision supersedes stale queued writes
- flush waits for durable entity chunk writes while cache entity writes remain
  pending but visible to loads
- close drains durable entity chunk writes and skips cache entity writes
- entity-store record pack/hydrate preserves persistent ids while assigning
  fresh runtime ids
- empty entity chunk hydration removes existing persistent entities in that
  chunk
- loaded entity chunk records hydrate through `IntegratedServer` into entity
  snapshots for tracking players
- entity chunk records survive holder unload/reload through the shared memory
  world store path
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`

Validation still needed in later entity/generation slices:

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

Status: Slice 4A native threaded mailbox landed 2026-07-03; Slice 4B SQLite
world-store foundation landed 2026-07-03. Dedicated and desktop local
world-dir wiring landed in Slices 5 and 5B; browser/Android adapters remain
pending.

Deliverables beyond the backend:

- persistence actor can run on its own native thread behind the same
  request/completion contract; this has landed as
  `PersistenceMailbox::threaded` for native `Send` stores
- host tick integrates completions without blocking on storage

Landed shape:

- `PersistenceMailbox` now has inline and native-thread backends; the inline
  backend remains the default so existing non-`Send` test stores and WASM paths
  are unchanged.
- Native callers can opt into the threaded backend through
  `PersistenceMailbox::threaded`,
  `ChunkScheduler::try_with_threaded_world_store`, and
  `IntegratedServer::try_with_threaded_world_store`.
- The threaded backend forwards the same `WorldStoreRequest` values to a worker
  actor and returns the same `WorldStoreCompletion` values to the host poll
  path.
- Same-key pending-write visibility, durable/cache ordering, flush, close, and
  entity chunk support stay owned by the shared actor.
- A blocked or slow native store write no longer blocks host-side mailbox
  polling.

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

Landed backend shape:

- `SqliteWorldStore` is a native `WorldStore` backed by one SQLite database.
- It stores versioned engine-native chunk blobs using the existing
  `ChunkRecord` binary codec.
- It stores versioned entity chunk blobs using the new `EntityChunkRecord`
  binary codec for Cow, Chicken, and Item save records.
- It creates metadata, chunk, entity chunk, player placeholder, and saved-data
  placeholder tables, and records schema version through `PRAGMA user_version`.
- It enables WAL where supported and checkpoints on `flush` / `close`.
- The backend is native-only and exported from `mclone-server`; WASM builds do
  not pull in SQLite.

Alternative allowed if SQLite integration is blocked:

- filesystem records under a world root, still behind the same `WorldStore`
  contract; writes must be temp-file-then-atomic-rename (the current snapshot
  store's bare `File::create` corrupts a chunk on mid-write crash)

Validation:

- direct SQLite store reopen preserves chunk and entity chunk records
- threaded mailbox plus SQLite store reopen preserves durable chunk and entity
  chunk writes
- dedicated server wiring creates a named world dir/db
- chunk edit persists across dedicated server process restart
- desktop native local-integrated runner wiring creates a world dir/db through
  `--world-dir`
- chunk edit persists across desktop local-integrated restart through the
  runner path
- generated animal persists across process restart after generated-original
  entity persistence lands
- killed generated animal remains gone across process restart after tombstone
  suppression lands
- explicit transient world still discards all state
- a deliberately slow store fake does not inflate host tick time; this is
  covered for the threaded mailbox foundation

### Slice 5: Dedicated Server World Dir

Wire durable persistence into the dedicated server app without changing the
shared policy.

Status: landed 2026-07-03.

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

Landed shape:

- `SqliteWorldStore::open_world_dir` maps a world directory to
  `world.sqlite3`.
- `IntegratedServer::try_with_threaded_sqlite_world_dir` constructs the
  dedicated host with the native threaded persistence actor and SQLite store.
- `ChunkScheduler::close_persistence` and
  `IntegratedServer::shutdown_persistence` drain dirty durable writes and close
  the actor on clean shutdown.
- `mclone-dedicated-server` accepts `--world-dir PATH`, `--world-root PATH`
  plus `--world-name NAME`, and explicit `--transient`.
- The dedicated loop opens the world inside the loop thread, preserving the
  current non-`Send` server internals while still keeping SQLite IO off the
  server tick thread.
- A dedicated restart test breaks a block through the native TCP client path,
  closes the server, reopens the same world directory, and verifies the block
  edit from the reloaded snapshot.

### Slice 5B: Desktop Native Local World Dir

Wire durable persistence into desktop local-integrated startup without moving
storage policy into the desktop app crate.

Status: landed 2026-07-03.

Deliverables:

- storage selection on `NativeIntegratedServerRunnerConfig`
- shared app-runtime local scene option for transient vs persistent storage
- desktop native-client `--world-dir PATH` and explicit `--transient`
- reject `--world-dir` with `--remote-addr`
- clean runner shutdown keeps draining dirty writes through
  `IntegratedServer::shutdown_persistence`

Landed shape:

- `NativeIntegratedServerWorldStorage::{Transient, Persistent { dir }}`
  selects runner storage.
- Persistent local worlds open `SqliteWorldStore::open_world_dir` through the
  same native threaded persistence actor as the dedicated path.
- `LocalSingleViewSceneOptions` carries the storage choice, so window,
  screenshot, perf, and other desktop local scene startup paths share the same
  runner wiring.
- `mclone-native-client --world-dir PATH` opens a SQLite-backed local
  integrated world; no world argument, or `--transient`, keeps the existing
  transient behavior.
- Remote dedicated scenes clear local `world_dir` state and the CLI rejects
  direct `--world-dir` / `--remote-addr` combinations.

Validation:

- native runner restart test breaks a generated block, shuts down, reopens the
  same world directory, and verifies the block remains air from the reloaded
  snapshot
- desktop CLI tests cover `--world-dir`, `--transient` conflict, and
  `--world-dir` with remote rejection
- `cargo test --manifest-path native/Cargo.toml -p mclone-server
  runner::native::tests::native_runner_persistent_world_dir_survives_restart`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

### Slice 6: Browser IndexedDB Backend

Add browser singleplayer storage behind the same logical contract.

Status: Slice 6A preload/writeback bridge landed 2026-07-03; Slice 6B
autosave/reload validation landed 2026-07-03; Slice 6C async load-miss bridge
landed 2026-07-03.

Deliverables:

- IndexedDB object stores for chunks and entity chunks landed; metadata,
  players, and saved data remain placeholders for later record families
- host-worker-owned adapter landed in the integrated-server worker
- clear-world/debug reset path landed through `clearWorldStorage`
- adapter lifetime equals world lifetime; world replacement tears down and
  recreates the adapter instead of epoch-stamping requests
- no main-thread chunk/entity decode; TypeScript stores raw `Uint8Array` blobs
  and Rust owns the shared binary codec

Landed shape:

- `mclone-server` exports chunk/entity record encode/decode helpers that compile
  for wasm as well as native.
- `IntegratedServer` / `ChunkScheduler` can construct a local integrated server
  over an arbitrary `WorldStore` while still using wasm worldgen/light job
  workers.
- The web integrated-server worker opens `mclone-web-worlds` IndexedDB and uses
  `(worldId, x, z)` keys for `chunks` and `entityChunks` stores.
- Current startup with `worldStorage=indexeddb` opens/clears the selected
  `worldId` without loading every record. The old preload constructor remains
  as a compatibility fallback if the external-load constructor is unavailable.
- The shared external-load mailbox backend keeps cached loaded records and
  save revision precedence in Rust, emits chunk/entity chunk load requests for
  host-owned storage, and accepts matching load completions back through the
  scheduler.
- During the session, dirty saves still mirror committed writes into wasm dirty
  maps for TypeScript writeback; load misses are serviced from IndexedDB through
  request/completion arrays.
- `shutdownAsync()` waits for the worker to call
  `IntegratedServer::shutdown_persistence`, receives dirty encoded records, and
  writes them back to IndexedDB before resolving.
- `worldStorage=indexeddb`, `worldId=...`, and `clearWorldStorage=1` are parsed
  from the browser query string and passed through the shared web runner config.
- The web worker now asks the shared server to save dirty chunks after command
  handling and ticks, drains pending persistence save completions, attaches the
  resulting dirty IndexedDB records to normal worker responses, and clears the
  wasm dirty-record mirrors after attaching them to the response.
- The TypeScript integrated-server worker writes dirty IndexedDB records after
  command, poll, tick, and shutdown responses for the current browser world.
- The TypeScript integrated-server worker also drains `indexedDbLoadRequests`,
  performs keyed IndexedDB `get([worldId, x, z])` calls, feeds completions back
  into Rust, and repeats until both persistence and worldgen/light work drain.
- `WebChunkRenderSession.blockStateAt()` exposes a narrow diagnostic read path
  so browser smoke tests can verify exact world state across page reloads.
- Runner diagnostics and web smoke predicates now include pending persistence
  load/save counts.

Validation:

- `pnpm native:web:smoke` includes an IndexedDB probe that creates a persistent
  browser world, clears existing records, streams a small view, awaits
  `shutdownAsync()`, verifies chunk records exist in IndexedDB, restarts the
  same world, and streams through IndexedDB load completions
- `pnpm native:web:smoke -- --indexeddb-reload-probe` places a dirt block in an
  IndexedDB-backed browser world, waits for browser chunk records, reloads the
  page without clearing storage, and verifies the same block position is still
  dirt through the shared client state with persistence load/save counts drained
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-web-client`
- `pnpm native:web:build`

Deferred beyond Slice 6C:

- fully external async save/writeback path instead of the current dirty-record
  response bridge
- metadata/player/saved-data object stores wired to live record families
- in-app browser world list / create / delete UI
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

## Closeout

This tactical is closed as the architecture bring-up tracker. The core shared
host-owned persistence path exists across memory/null, native SQLite, dedicated
server, desktop local integrated, and browser IndexedDB chunk/entity records.

Remaining work is intentionally moved to
[`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md):

- catalog/world identity and UI lifecycle
- browser world list/create/delete and metadata records
- player/world/saved-data persistence
- Android app-private world roots and lifecycle validation
- cleanup of the current browser dirty-record response writeback bridge

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
pnpm native:web:smoke -- --indexeddb-reload-probe
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
