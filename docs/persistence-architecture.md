# Persistence Architecture

Shared proposal for durable world storage, generated caches, entity storage,
player data, and platform-specific backends.

This document is architectural guidance, not a tactical implementation slice.
[`loading-persistence.md`](loading-persistence.md) remains the detailed chunk
loading/save-policy reference. [`entities.md`](entities.md) owns entity runtime
semantics. The first implementation tracker is
[`tactical/134-shared-persistence-architecture.md`](tactical/134-shared-persistence-architecture.md).

## Core Rule

The authoritative host owns persistence policy. Storage adapters persist logical
world records; they do not own simulation, generation, renderer state, entity
truth, or platform-specific gameplay behavior.

Memory-only worlds remain a supported mode. They must be explicit
`Null`/`Transient` stores, not an accidental fallback that lets persistence-ready
gameplay silently lose state on unload.

## Why This Matters Now

The current native runtime can regenerate chunks and keep many gameplay facts in
memory. That is useful for fast smoke tests and temporary worlds, but it blurs
entity and spawning semantics:

- generation-time animals become ambiguous if chunk unload deletes them
- live natural spawning cannot match vanilla caps/despawn behavior without
  durable entity records
- killed generated animals must not reappear just because the same seed is
  loaded again
- scheduled fluid/block ticks need save/load semantics rather than load-time
  settling
- player inventory, spawn, world time, and future block entities need durable
  records outside render snapshots

Persistence should become a shared server/runtime facility before more gameplay
systems depend on it.

## Vanilla Reference Map

Read these before implementing persistence behavior:

| Concern | Java source |
|---|---|
| chunk residency, save, unload | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java` |
| main chunk access/save entrypoints | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java` |
| async chunk IO worker and pending writes | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java` |
| chunk physical region file access | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/RegionFileStorage.java` |
| chunk payload read/write | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` |
| entity lifecycle/load/save manager | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` |
| entity persistent storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/EntityStorage.java` |
| entity section index | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySectionStorage.java` |
| world-level saved data | `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/DimensionDataStorage.java` |
| saved data dirty flag | `reference/minecraft-1.17.1/src/net/minecraft/world/level/saveddata/SavedData.java` |
| player data files | `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/PlayerDataStorage.java` |

The important parity target is the lifecycle and ownership shape, not literal
Anvil file compatibility in the first native implementation.

## Vanilla Shape To Preserve

Minecraft Java 1.17.1 uses an authoritative server even in singleplayer. The
server owns chunk residency, dirty state, entities, scheduled ticks, and disk
IO. The client receives packets and renders a replica.

Key properties to preserve:

- storage lookup happens before generation
- stored chunks can resume from a status instead of always starting empty
- foreground loads are prioritized over background stores
- same-chunk pending writes are visible to loads before the backend is queried
- dirty chunks save before eviction and world close
- scheduled ticks are packed into chunk records before save and unpacked into
  server queues when chunks become accessible
- entity storage is logically separate from block chunk storage
- entity visibility and ticking depend on full chunk status, not renderer demand
- empty entity chunks are valid stored facts
- world/player/saved-data records have their own dirty lifecycle

## Target Runtime Shape

Use one shared logical interface, backed by platform adapters:

```text
authoritative host
  owns loaded chunks, entities, players, ticks, dirty state, and save policy

persistence actor
  owns encode/decode, pending write coalescing, backend calls, flush, close

physical backend
  IndexedDB, SQLite, filesystem records, memory, or future region files
```

The persistence actor may run as:

- a native thread or task on desktop, dedicated server, and Android
- a browser worker or host-worker-owned async adapter on web
- an in-process deterministic fake for unit tests

The first contract slice ran the actor same-thread and poll-driven behind the
identical completion contract. Native targets now also have an opt-in threaded
mailbox for `Send` stores; tests, compatibility stores, and WASM can keep the
inline backend. The first durable native backend should use the threaded path by
default, because real disk IO on the server thread would reintroduce exactly
the blocking this design exists to avoid.

The host should integrate completed loads and save acknowledgements during its
normal tick/poll path. It should not synchronously wait for disk or IndexedDB
during command handling, rendering, or chunk-view updates.

## Logical Store Contract

The shared contract must be completion-based, not blocking. On web the host
runs in a worker and IndexedDB is promise-only: a host blocked inside a
synchronous `flush()` can never observe IndexedDB completions and deadlocks.
The request and completion message enums are the contract itself; blocking
convenience wrappers may exist as native-only helpers, never as the shared
surface. The host enqueues requests and integrates completions during its
normal tick/poll path.

The request surface should be record-oriented:

```text
WorldStore requests
  open_world(metadata)
  load_chunk(pos)
  save_chunk(record, durability)
  load_entity_chunk(pos)
  save_entity_chunk(record, durability)
  load_player(player_id)
  save_player(record, durability)
  load_saved_data(key)
  save_saved_data(key, record, durability)
  flush()
  close()
  delete_world(world_id)

WorldStore completions
  world_opened(metadata) / world_open_failed(error)
  record_loaded(key, record | miss)
  save_acknowledged(key) / save_failed(key, error)
  flush_complete
  close_complete
```

Reset/migration decisions are host policy. The store exposes primitives —
metadata reads and `delete_world` — and the host decides whether an
incompatible world resets, migrates, or refuses to open. There is no
`reset_incompatible_world` in the store itself.

The first implementation can land a narrower subset, but the contract should
leave room for every record family from the start.

Save durability is explicit per write:

| Durability | Meaning |
|---|---|
| `Cache` | discardable generated or derived data; may be skipped on shutdown |
| `Durable` | user/world mutation state; must not be lost before eviction |

Durability controls scheduling priority and may-skip-on-shutdown policy only.
It never decides write-wins: precedence between writes for the same key is
revision-based (see actor semantics below). `flush` is a barrier request, not
a durability level; the backend should force durable writes where the platform
supports it.

## Record Families

### World Metadata

World metadata should include:

- save id and display name
- target Minecraft version and mclone storage schema
- seed and preset/world type
- min build height and world height
- content/registry version
- generation algorithm version
- optional light algorithm version
- game time/day time once world time is durable
- spawn position once player spawning is durable

Metadata compatibility controls reset/migration. Incompatible generated cache
records can be dropped. Incompatible durable records require a migration,
explicit reset, or an error path.

### Chunk Records

Chunk records store block/world facts, not meshes:

- chunk position
- chunk status
- revision or write version
- packed block sections
- biomes
- scheduled block ticks
- scheduled fluid ticks
- optional light sections with a trust/version marker
- future heightmaps
- future structures and references
- future block entities
- future proto status fields such as carving masks and postprocessing

Separate these meanings:

- **durable chunk state**: block edits, scheduled ticks, block entities, and
  other gameplay state that must survive reload
- **generated cache state**: deterministic generated chunks or partial statuses
  that can be discarded and regenerated when code/content versions change

If a generated chunk is mutated, durable semantics win.

Palette encoding: v1 chunk records may store numeric block-state ids, which
are only stable within one content/registry version. That is an explicit v1
compromise. The durable direction is stable name+properties palette
identifiers, as vanilla NBT uses, so registry changes stop being
save-breaking. Until then, a registry version mismatch blocks durable saves.

### Entity Chunk Records

Entity records are chunk-addressed but section-owned while loaded:

```text
EntityChunkRecord
  chunk_pos
  entities: EntitySaveRecord[]
```

Rules:

- block chunk records and entity chunk records are separate logical records
- records carry a persistent UUID, a stable kind code (never an enum ordinal),
  position, rotation, motion, on-ground, age, and per-kind save data such as
  item stacks or egg timers
- runtime entity ids are session-local and never persisted; loads assign fresh
  ids (vanilla saves the UUID, never the int id)
- vanilla-transient state is not saved; vanilla is the reference for what
  counts (for example sheep `eatAnimationTick` is transient, while `Sheared`
  and `Color` persist)
- generation-time original mobs become normal persisted entities after insertion
- empty entity chunks are valid and should not force a block chunk write
- dirty entity chunks save before eviction
- `UNLOADED_TO_CHUNK` style removal is saveable; killed/discarded entities are not
- natural spawning for persistent categories should stay gated until this path
  exists

Invariant: if any entity chunk record exists for a chunk — including an empty
record — generation-time entity placement must not run for that chunk. This
holds even when the block chunk record was discarded as incompatible generated
cache and the terrain regenerated. Without this rule, a cache reset would
resurrect killed animals.

The existing `entity_snapshot`, `entity_update`, and `entity_remove` protocol
messages remain separate from storage records.

### Player Records

Player records should be separate from entity chunk records:

- stable player id
- dimension/world id
- position and rotation
- spawn/bed data later
- selected hotbar slot
- inventory and carried item later
- game mode/abilities later
- appearance/preferences that are world/session state

Singleplayer can still treat the local player specially, but the physical save
shape should not block future dedicated-server and remote play.

### Saved Data Records

Saved data is the catch-all for named world data:

- maps later
- forced chunks later
- scoreboard-like data later
- command schedules later
- world-level counters and ids

This should look like a typed key-value store with dirty tracking, not a chunk
side channel.

## Persistence Actor Semantics

Actor lifetime equals world lifetime. Like vanilla's per-world `IOWorker`, the
actor is created at world open and closed at world close; world replacement
tears the actor down and builds a new one. This removes the need for a
session-epoch protocol in the store. Dropping load results that complete after
chunk interest goes away is host bookkeeping, not store protocol.

The actor must provide these observable rules:

- same-key writes are ordered, and a newer write supersedes a stale queued
  write for the same record
- loads see pending writes before querying the backend
- write precedence is revision-based: a queued write is superseded only by a
  write with a higher revision for the same key, and durability breaks
  revision ties in favor of `Durable`. A blanket durable-blocks-cache rule
  would be wrong: after a durable save, a later status promotion can
  legitimately produce a cache save that carries the durable edits plus new
  generated state at a higher revision, and it must win.
- unload keeps resident state alive until required durable saves acknowledge
- explicit flush waits for all required dirty records and then asks the backend
  to flush if supported
- close drains required durable writes; requests submitted after close fail
  explicitly instead of being silently dropped
- errors surface to the host and UI instead of silently switching to transient
  mode

Suggested priority lanes:

| Priority | Work |
|---|---|
| foreground | world open, chunk/entity loads needed by active interest |
| durable save | dirty records required for unload, explicit save, close |
| cache save | generated clean chunks or other discardable cache records |
| maintenance | vacuum/compaction/diagnostic metadata |

## Backend Plan

### Memory And Null

Keep both:

- `NullWorldStore`: explicit transient mode, always misses loads and accepts
  no-op saves
- `MemoryWorldStore`: deterministic in-memory backend for unit tests and actor
  semantics, including entity chunk records

The difference matters. `Null` is "throw everything away"; `Memory` is "test a
real persistence lifecycle without disk/browser IO."

### Native Filesystem And Dedicated Server

The current `FilesystemChunkSnapshotStore` is a useful compatibility scaffold
and now implements `WorldStore`, but it remains chunk-only and synchronous.
Likewise, `ChunkSnapshotWorldStore` is snapshot-only compatibility glue and does
not advertise entity chunk support.
Native `SqliteWorldStore` is the first durable block/entity chunk backend behind
the same logical contract. It is not yet wired into app or dedicated world-dir
startup paths.

Current durable native backend:

- one SQLite database per world
- blob columns for engine-native chunk/entity records
- player and saved-data placeholder tables
- metadata and schema tables with `PRAGMA user_version`
- WAL mode where supported
- checkpoint on flush/close

SQLite is a pragmatic starting point because it gives indexing, transactions,
schema migration, and fewer small-file problems. `rusqlite` with the bundled
feature cross-compiles cleanly to Android and Quest, and web uses IndexedDB,
so no wasm SQLite build is needed. A custom region-file backend can still be
added later if measurements justify it.

The native IO-thread foundation exists as `PersistenceMailbox::threaded`. The
durable backend should run through that worker-thread mailbox by default so
real disk IO stays behind the same request/completion contract and never runs
inline on the server thread.

If a plain filesystem-records fallback is ever used instead, writes must be
temp-file-then-atomic-rename. The current snapshot store writes through bare
`File::create`, so a mid-write crash corrupts the chunk file.

### Web

Use IndexedDB for browser singleplayer. The host worker or persistence worker
owns the adapter. The browser main thread must not decode chunk/entity records
or run storage policy.

Use logical object stores such as:

```text
worlds
chunks
entity_chunks
players
saved_data
```

OPFS remains a possible later backend, but IndexedDB is the most practical first
portable browser path.

### Android

Use app-private storage. The platform app supplies the world root/open handle;
shared persistence code owns record format and policy.

SQLite per world is also the likely first Android backend. It avoids exposing
gameplay policy to Android app glue and maps well to app-local storage.

## Versioning And Migration

Do not use one version number for everything long-term. Track at least:

- physical database/schema version
- chunk record codec version
- entity record codec version
- player record codec version
- content/registry version
- generation output version
- light trust/algorithm version
- target Minecraft version

General principle: cache records may be dropped with a log line; durable
records are never silently dropped — they migrate or fail open with an
explicit user-facing reset choice. Pre-1.0, offering reset instead of writing
migrations is acceptable; silent loss is not.

Default policy per version stamp:

| Version stamp changed | Default behavior |
|---|---|
| generation output version | reset cache-status chunk records only; durable-edited chunks are kept as-is (durable wins even if their generated portion is stale) |
| light algorithm version | never blocks; stored light becomes untrusted (`light_correct = false`) |
| schema / durable codec versions (chunk, entity, player) | newer-than-code always blocks; older-than-code blocks unless a migration exists |
| seed, min build height, world height | block — these change the meaning of every stored record; a different seed is a different world, not a reset condition |
| target Minecraft version | block until a migration exists |
| content/registry version | blocks durable saves and resets cache records until stable palette identifiers land (see Chunk Records) |

## Light Policy

Current behavior already serves stored light: snapshot format v3 persists
light sections plus `light_correct`, and the scheduler load path republishes
stored snapshots as-is, so reloaded light is trusted de facto today. The safe
default below must be made real, not assumed.

Pick one path before persisted light becomes authoritative:

1. Do not store trusted light. Recompute from persisted blocks on reload, and
   force `light_correct = false` on load until a trust protocol exists.
2. Store trusted light with `light_correct` plus a light algorithm/content
   version, hydrate it into the light engine, and invalidate stale records.

Either way, add the light algorithm/version stamp to the chunk record during
the contract slice while the format change is cheap, even if hydration lands
later.

## Scheduling And Ticks

Saved scheduled ticks should behave like vanilla:

- pending ticks are stored with the owning chunk record
- chunk load does not run ticks to completion
- accessible/full promotion hydrates chunk-local ticks into host tick queues
- save/unload packs outstanding queue entries back into chunk records

This applies to both fluid and future block ticks.

## Interaction With Spawning

Natural spawning should distinguish two modes:

- **volatile/debug spawning**: allowed with transient storage for visual or
  development scenarios
- **vanilla-profile persistent spawning**: requires entity persistence, live
  category counts, despawn rules, chunk activity state, and dirty entity saves

Persistent passive creatures must not disappear just because their chunk leaves
view. Killed creatures must not respawn from seed-time generation on reload.

## First Implementation Direction

The first useful chunk is not SQLite or IndexedDB. It is the shared contract and
actor boundary:

1. add the `WorldStore` request/completion contract and `PersistenceActor`
   inside `mclone-server`; extract a separate crate only when a non-server
   consumer appears
2. implement `NullWorldStore` and `MemoryWorldStore`
3. route the existing chunk snapshot load/save path through the actor behind a
   synchronous facade, without changing physical backends; async load
   integration through the holder state machine belongs to the dirty/unload
   slice
4. add tests for pending-write visibility, same-key ordering, revision-based
   precedence with durable tie-break, flush/close, and post-close request
   rejection
5. keep app wiring transient until the actor contract is proven

After that, the next high-value slice is entity chunk persistence for generated
passive mobs, because it unblocks clean spawning semantics.
