# Entities

Durable architecture notes for Minecraft Java 1.17.1-style entity runtime ownership in `mclone`.

This document is about entity lifecycle, storage, ticking, host ownership, persistence, and protocol shape. Creature spawning remains covered by [`creatures.md`](./creatures.md), and the first fixture/oracle step remains [`tactical/Creatures0-generation-entity-oracle-foundation.md`](./tactical/Creatures0-generation-entity-oracle-foundation.md). The first runtime foundation slice, [`tactical/Entities0-runtime-entity-foundation.md`](./tactical/Entities0-runtime-entity-foundation.md), is landed.

## Current Status

`Entities0` has the first host-owned runtime container:

- vanilla-shaped `FullChunkStatus` to `Visibility` mapping
- minimal runtime entity access records and removal reasons
- section-owned entity storage and accessible AABB queries
- visible id/UUID lookup and entity getter facade
- copy-on-write `EntityTickList`
- `PersistentEntitySectionManager` add/move/remove/load/unload/save semantics
- logical chunk entity storage with a memory adapter for tests
- `EntityRuntime` host wrapper that owns the manager and tick list

`Entities0` itself stayed pre-creature. `Creatures1` feeds the runtime with generation-time passive entities through the host-owned worldgen entity sink. `Creatures2` wires that runtime into `GeneratedWorldHost` and publishes generated entities as protocol `entity_snapshot` records consumed by local and remote clients as data. `Creatures5` starts ticking generated cows in `ENTITY_TICKING` chunks and publishes movement through `entity_update`. `Creatures7` adds the first authoritative living-entity push pass for generated mobs and the local player.

Still deferred: full per-session tracking/revision policy, despawn, durable entity persistence adapters beyond the current in-memory runtime path, hard entity collision shapes for vehicles/special entities, full `deltaMovement` / `Entity.move(...)` parity, and most mob behavior beyond the first passive wander foundation.

## Scope

The first entity runtime foundation should make entities real authoritative host state. It should not yet make mobs intelligent, spawn them naturally, or render them.

In scope:

- vanilla-shaped entity identity, position, section ownership, removal, and persistence state
- tracked vs ticking lifecycle
- chunk status transitions through `BORDER`, `TICKING`, and `ENTITY_TICKING`
- host-owned mutation order for add, remove, move, load, save, and unload
- protocol-ready entity snapshot/delta facts
- browser worker and Node host adaptation

Explicit non-goals for the first runtime foundation:

- no natural spawning
- no generation-time original mob spawning
- no AI, goals, sensing, navigation, pathfinding, combat, breeding, taming, or despawn rules
- no renderer models, animations, particles, sounds, or entity selection UI
- no full `Mob`, `LivingEntity`, `Player`, inventory, equipment, passenger, leash, or brain port

## Reference Source Map

Read these before implementing entity runtime code:

| Concern | Vanilla source |
|---|---|
| Server-level ownership, entity tick loop, manager callbacks | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` |
| Entity manager, load/unload, section callbacks | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` |
| Section index and AABB queries | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySectionStorage.java` |
| Section-owned entity collection | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySection.java` |
| Copy-on-write ticking set | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java` |
| Entity visibility mapping | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/Visibility.java` |
| Visible entity id/UUID lookup | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityLookup.java` |
| Query facade over visible lookup and sections | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/LevelEntityGetterAdapter.java` |
| Persistent entity region storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/EntityStorage.java` |
| Full chunk status futures | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` |
| Chunk status publication and client tracking | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java` |
| Player distance tickets and natural-spawn tracker | `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java` |
| Base entity identity, position, save/load, remove callbacks | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Mob persistence flags, despawn, and AI hook surface | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |

The simulation-facing behavior should be direct translation where practical. Runtime scheduling and storage carriers may diverge for browser worker and Node constraints, but only behind the same logical lifecycle.

## Vanilla Ownership Model

`ServerLevel` owns entity runtime state through two cooperating structures:

- `PersistentEntitySectionManager<Entity>` owns loaded entity sections, visible entity lookup, persistent load/save state, and callbacks for tracking/ticking transitions.
- `EntityTickList` owns the current ticking set used by `ServerLevel`'s entity tick loop.

`ServerLevel` constructs `EntityStorage` at the dimension path's `entities` directory and passes `entityManager::updateChunkStatus` into `ServerChunkCache`. When `ChunkMap` promotes or demotes a full chunk status, that callback updates entity section visibility. Entity tracking and ticking are therefore consequences of chunk status, not renderer demand.

The tick order is:

1. `ServerChunkCache.tick(...)` advances chunk tickets, chunk ticks, natural spawning, and chunk publication.
2. `ServerLevel` iterates `entityTickList`.
3. Each ticking entity runs `checkDespawn()`, vehicle/passenger checks, then `tickNonPassenger(...)`.
4. `tickNonPassenger(...)` increments `tickCount`, calls `Entity.tick()`, then recursively ticks passengers that are players or already in `entityTickList`.
5. Block entities tick.
6. `entityManager.tick()` processes pending entity loads and chunk unloads.

For `Mob`, the full AI surface is under `LivingEntity.aiStep()` and `Mob.serverAiStep()`: sensing, target selector, goal selector, navigation, custom AI, movement/look/jump controls, and debug packets. That is intentionally outside the first runtime foundation.

## Entity Storage Vs Chunk Block Storage

Entities are not block-section data.

Vanilla block chunks are persisted under the dimension `region/` directory. They contain chunk sections, palettes, heightmaps, lighting, scheduled ticks, block entities, and generation metadata.

Vanilla 1.17.1 persistent entities are stored separately under the dimension `entities/` directory using region files named like:

```text
world/entities/r.<regionX>.<regionZ>.mca
```

Each entity-region chunk payload is a top-level NBT compound with:

- `DataVersion`
- `Position`: int array `[chunkX, chunkZ]`
- `Entities`: list of saved entity compounds

`EntityStorage` reads and writes that shape through `DataFixTypes.ENTITY_CHUNK`. Empty entity chunks may be stored as absent/null payloads, and vanilla keeps an in-memory empty-chunk cache.

During generation and proto/full conversion there can still be legacy/proto chunk `Entities` lists. `ChunkMap.postLoadProtoChunk(...)` passes those to `ServerLevel.addWorldGenChunkEntities(...)`, and the persistent section manager takes ownership from there. For `mclone`, generated chunk block payloads should remain block snapshots; generated entity records should enter an entity sink or entity manager, not the block mesh or packed chunk section format.

## Tracked Vs Ticking

Vanilla separates "queryable/tracked" from "ticking".

`EntityLookup` stores visible entities by integer id and UUID. `LevelEntityGetterAdapter.get(id)`, `get(UUID)`, and `getAll()` use that visible lookup. AABB queries scan accessible entity sections. Hidden sections are not visible to normal queries.

`EntityTickList` is separate. `ServerLevel` ticks only entities in that tick list, plus passenger recursion rules.

For ordinary entities:

| Entity visibility | Queryable/tracked | Ticking |
|---|---|---|
| `HIDDEN` | no | no |
| `TRACKED` | yes | no |
| `TICKING` | yes | yes |

`EntityAccess.isAlwaysTicking()` is the exception path. Base `Entity` returns `false`; `Player` returns `true`. `PersistentEntitySectionManager.getEffectiveStatus(...)` treats always-ticking entities as `TICKING` regardless of section status.

Tracking has more than lookup side effects. In `ServerLevel.EntityCallbacks`, tracking start calls `ChunkMap.addEntity(...)`, adds players to the player list, adds mobs to `navigatingMobs`, and registers dragon parts. Tracking end reverses those effects. In `mclone`, client subscription and protocol publication should hang off tracking, not ticking.

## Full Chunk Status And Entity Visibility

Vanilla full chunk statuses live in `ChunkHolder.FullChunkStatus`:

```text
INACCESSIBLE -> BORDER -> TICKING -> ENTITY_TICKING
```

`Visibility.fromFullChunkStatus(...)` maps them for entities:

| Full chunk status | Entity visibility | Meaning |
|---|---|---|
| `INACCESSIBLE` | `HIDDEN` | Not visible to normal entity queries and not ticking. |
| `BORDER` | `TRACKED` | Entity data is accessible/tracked, but ordinary entities do not tick. |
| `TICKING` | `TRACKED` | Chunk/block/random tick work can run, but ordinary entities still do not tick. |
| `ENTITY_TICKING` | `TICKING` | Ordinary entities are tracked and eligible for `ServerLevel` entity ticks. |

This distinction matters. A vanilla chunk can be loaded and block-ticking without ordinary mobs ticking. Natural spawning also checks `ServerLevel.isPositionEntityTicking(...)`, so loaded or block-ticking chunks are not enough for live mob spawning.

## Section Ownership And Chunk Crossing

Entities are stored in vertical sections, not only in flat chunks.

`PersistentEntitySectionManager.addEntity(...)` computes:

```text
sectionKey = SectionPos.asLong(entity.blockPosition())
```

It then inserts the entity into `EntitySectionStorage.getOrCreateSection(sectionKey)` and installs an `EntityInLevelCallback` on the entity.

`Entity.setPosRaw(...)` updates the entity position, block position, and calls `levelCallback.onMove()`. The manager callback recomputes the section key. If it changed:

1. remove from the current `EntitySection`
2. remove the old section if it is empty
3. create or find the destination section
4. add the entity to the destination section
5. compare old and new effective visibility
6. start/stop tracking and ticking as needed

Crossing a chunk boundary is just crossing from a section whose `x,z` chunk key belongs to one chunk to a section whose key belongs to another. Persistence ownership follows the destination section's chunk key. Vertical section changes within the same chunk still matter for AABB queries and section cleanup.

## Add, Remove, Move, Load, And Unload Semantics

Vanilla does not use one global "pending entity ops" queue for all mutation.

### Add

`ServerLevel.addFreshEntity(...)` calls `PersistentEntitySectionManager.addNewEntity(...)`. Add is immediate:

- UUID is inserted into `knownUuids`; duplicate UUIDs are rejected.
- Entity is inserted into its current section.
- The entity receives a level callback.
- Creation callback runs unless the add is from legacy persistent load.
- If effective visibility is accessible, tracking starts.
- If effective visibility is ticking, ticking starts.

If an entity is added while `EntityTickList` is being iterated, the tick list's copy-on-write behavior keeps the current iteration stable. The new entity is in the active set for later iterations, not injected into the iterator already in progress.

### Remove

`Entity.remove(...)`, `discard()`, and other removal paths call `Entity.setRemoved(...)`, which calls `levelCallback.onRemove(...)`.

The manager removal callback immediately:

- removes from the current section
- stops ticking if currently ticking
- stops tracking if currently accessible
- runs destroyed callbacks only if `RemovalReason.shouldDestroy()` is true
- removes the UUID from `knownUuids`
- clears the entity level callback
- removes the section if empty

`RemovalReason` controls persistence:

| Reason | Destroy callback | Saved |
|---|---|---|
| `KILLED` | yes | no |
| `DISCARDED` | yes | no |
| `UNLOADED_TO_CHUNK` | no | yes |
| `UNLOADED_WITH_PLAYER` | no | no |
| `CHANGED_DIMENSION` | no | no |

### Move

Move is immediate through the entity level callback. If the move crosses section or chunk visibility boundaries, tracking/ticking callbacks run during the move.

This is important for mclone's host lane. Entity movement should not be a renderer-local fact later reconciled into the manager. The authoritative record should move first; derived client presentation follows.

### Tick-List Mutation During Ticks

`EntityTickList` uses two linked maps named `active` and `passive`. When `add(...)` or `remove(...)` happens while `active` is being iterated, `ensureActiveIsNotIterated()` copies the active map into passive, swaps them, and mutates the new active map. Only one concurrent iteration is supported.

The observable rule to preserve is:

- entity ticking iteration is stable while it is in progress
- add/remove/ticking-status changes affect the active set after the current iterator snapshot
- nested tick-list iteration is invalid

### Load And Unload

Entity storage load is asynchronous in vanilla. `requestChunkLoad(...)` marks the chunk load status `PENDING`, calls `EntityPersistentStorage.loadEntities(...)`, and pushes completed `ChunkEntities` into `loadingInbox`. `entityManager.tick()` drains that inbox and adds entities.

Chunk hidden status schedules unload through `chunksToUnload`. `entityManager.tick()` tries to store the chunk's saveable entity sections. If load is still pending, unload waits. On successful unload, entities are removed with `UNLOADED_TO_CHUNK` and their callbacks are cleared.

`saveAll()` and `autoSave()` write entity chunks independently from block chunks.

## Query Model

There are two query paths:

- id/UUID/all visible entities through `EntityLookup`
- AABB and typed AABB queries through `EntitySectionStorage`

`EntitySectionStorage` keeps a map from packed section key to `EntitySection` and a sorted set of section ids. AABB queries expand the query bounds by 2 blocks, iterate relevant sections, and skip sections whose visibility is not accessible.

For `mclone`, this means collision/spawn/despawn systems should query authoritative entity sections, not client render caches. Hidden entity sections can exist for persistence and ownership without being visible to gameplay queries.

## Mclone Runtime Adaptation

The vanilla shape:

1. server tick thread owns authoritative entity mutation
2. chunk status futures call back into the entity manager
3. entity IO and chunk generation can complete asynchronously
4. tracking publishes server entity state to players
5. renderer/client consumes server state

The browser/Node adaptation should keep the ownership while changing the carrier:

| Vanilla concept | `mclone` adaptation |
|---|---|
| Server tick thread | host authority lane in browser worker or Node event loop |
| `PersistentEntitySectionManager` | shared simulation/runtime entity manager with vanilla-shaped callbacks |
| `EntityStorage` using `world/entities/*.mca` | logical entity storage adapter; browser/file physical layout may differ |
| `ChunkHolder.FullChunkStatus` futures | host chunk job/status records publishing full-status changes |
| `ChunkMap.addEntity/removeEntity` | protocol tracking publication and session filtering |
| `EntityTickList` | copy-on-write or equivalent stable tick-list mutation semantics |
| worldgen proto entity list | generation entity sink integrated on the host lane |

The allowed divergence is runtime orchestration:

- browser workers and Node cannot share vanilla JVM object graphs
- storage adapters may encode entity records in IndexedDB or file records instead of literal `.mca`
- chunk jobs may produce generated entity records off-thread
- protocol/wire codecs may use structured clone, JSON control envelopes, and binary payloads

The not-allowed divergence for vanilla-profile behavior:

- renderer-owned entity truth
- putting entities into block chunk snapshots or mesh payloads
- ticking ordinary entities in `BORDER` or `TICKING` chunks
- treating killed generation-time animals as seed-respawnable decorations
- ignoring section ownership during movement
- dropping duplicate-UUID and removal-persistence semantics
- changing spawn/despawn/category behavior later to fit a client cache

## Protocol Implications

`entity_snapshot`, `entity_update`, and `entity_remove` are separate host-to-client updates, not fields inside `chunk_snapshot`.

`chunk_snapshot` establishes block, biome, light, and scheduled-tick facts for meshing. Entity churn has a different cadence, tracking range, and ordering requirement. Meshing should not rebuild because a cow moved.

Suggested baseline facts for `entity_snapshot`:

- protocol type: `entity_snapshot`
- authoritative runtime id
- stable UUID-equivalent identity, if exposed to clients
- entity type id, such as `minecraft:cow`
- owning chunk and section
- position, rotation, and optional velocity
- pose or minimal flags needed by presentation
- initial tracked data subset

The current landed `entity_snapshot` is the first baseline form for generated original mobs: id/UUID, type/category, owning chunk, position/rotation, dimensions, on-ground, age, authoritative tick, and small type-specific data.

Current baseline facts for `entity_update`:

- authoritative runtime id
- optional position/rotation/velocity update
- optional owning section/chunk change
- optional tracked-data patch
- optional authoritative tick when the update carries changed entity facts

Current baseline facts for `entity_remove`:

- authoritative runtime id
- optional UUID-equivalent identity
- optional removal/untrack reason

Entity snapshots and updates now carry tick context for generated mob movement. Per-session entity revisions are still deferred.

Ordering rules:

- A client should receive an `entity_snapshot` before deltas for that entity.
- Removal should be explicit, not an implicit disappearance caused by a chunk mesh update.
- Entity updates should carry tick context now and should gain per-session/entity revision context before higher-frequency movement depends on reordering recovery.
- Entity updates should be filterable per session by tracking range and interest.
- `chunk_unload` should either be paired with entity removals for entities no longer tracked by that client, or client cache semantics must define that unloading a chunk invalidates its tracked entities.

Polling transports can carry these as ordinary `WorldHostMessage` updates. If entity update frequency makes polling too expensive, that is a transport decision; it does not change the logical protocol.

## Persistence Direction

The logical storage model should mirror vanilla even if the physical adapter differs:

```ts
interface EntityChunkRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly entities: readonly EntitySaveRecord[];
}
```

Important rules:

- entity storage is chunk-addressed but section-owned while loaded
- block chunk records and entity chunk records are separate logical records
- entity records carry stable ids/UUIDs and saved entity data, not renderer handles
- empty entity chunks are valid and should not force block chunk writes
- dirty entity chunks should be saved before eviction
- `UNLOADED_TO_CHUNK` entities are saveable; killed/discarded entities are not
- generation-time entity records become normal persisted entities after insertion

For browser storage, the first adapter can store entity chunk records beside packed chunk records in IndexedDB. For Node storage, the first adapter can store entity records beside chunk files. A literal Anvil `entities/*.mca` writer is not required for the first mclone runtime slice, but the logical shape should not block one later.

## Relationship To Creatures Work

`Creatures0` remains the official-server fixture slice. It proves that we can read and normalize vanilla-generated entity output from `world/entities/*.mca` and legacy/proto chunk entity lists.

`Entities0` provides the host-owned runtime container that later creature slices can feed. It does not port `NaturalSpawner`.

`Creatures1` now ports generation-time original passive mobs against the `Creatures0` sheep fixture and inserts them through an entity sink shaped like `ServerLevel.addWorldGenChunkEntities(...)`. The generated records become normal host-owned entities in `PersistentEntitySectionManager`.

Live natural spawning should come later, after entity ticking, lighting, chunk activity, mob caps, and despawn semantics have enough runtime support.
