# Entities0 - Runtime entity foundation

Standing after the durable entity architecture in [`../entities.md`](../entities.md). This is the first runtime entity tactical. It builds host-owned entity state and vanilla-shaped section/tick lifecycle before creature spawning, AI, or rendering.

## Goal

Add a minimal TypeScript entity runtime foundation that mirrors the vanilla 1.17.1 entity manager shape:

- entities are authoritative host state
- entities live in `16x16x16` entity sections
- chunk full-status changes drive tracked vs ticking state
- add/remove/move semantics match vanilla's immediate callbacks
- tick-list mutation during iteration is stable
- entity chunk persistence has a logical record shape separate from block chunks

At the end of `Entities0`, mclone should be able to own, move, tick, track, untrack, save, unload, and reload simple synthetic entity records in tests. It should not spawn mobs, render mobs, or simulate AI.

## Implementation Status

Landed as the first runtime foundation pass:

- `src/world/level/entity/` now contains vanilla-shaped full chunk status, visibility, entity access, section storage, visible lookup, tick-list, logical persistent storage, and persistent section manager modules.
- `src/runtime/host/entity-runtime.ts` provides a narrow host-owned wrapper that owns the manager plus `EntityTickList`.
- Unit coverage exercises tracking/ticking status transitions, always-ticking entities, duplicate UUID rejection, id/UUID and AABB lookup, section/chunk movement, removal reasons, pending load/unload, `saveAll()`, and tick-list copy-on-write mutation.
- The slice intentionally does not add `NaturalSpawner`, creature spawn tables, AI/pathfinding, renderer ingestion, entity model loading, or protocol consumption.

## Why This Slice Exists

`Creatures0` is the oracle fixture slice. It teaches us how to observe vanilla entity output from official server worlds.

`Entities0` is the runtime architecture slice. It gives later creature work somewhere correct to put entities after spawn logic exists.

Keep those risks separate:

1. `Creatures0`: can we read vanilla generated entity facts correctly?
2. `Entities0`: can the host own entity lifecycle correctly?
3. `Creatures1`: can generation-time creature spawning produce matching entity facts?
4. Later slices: can live spawning, despawn, AI, and rendering build on that state?

## Reference Source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` | entity manager construction, tick loop, callbacks into tick list and tracking |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` | add/remove/move/load/unload/status transition semantics |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySectionStorage.java` | section indexing, chunk grouping, AABB query shape |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntitySection.java` | section-owned entity collection |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java` | copy-on-write mutation during tick iteration |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/Visibility.java` | `FullChunkStatus` to entity visibility mapping |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityLookup.java` | visible entity lookup by id and UUID |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/LevelEntityGetterAdapter.java` | query facade over lookup and section storage |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/EntityStorage.java` | persistent chunk entity record shape |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` | `BORDER`, `TICKING`, `ENTITY_TICKING` status model |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java` | status publication and entity tracking hooks |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java` | player-ticket context for future entity-ticking and spawning |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` | identity, position callback, saved fields, removal reasons |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` | persistence flags and non-goal AI/despawn surface |

Do not port `NaturalSpawner` in this slice.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Entity status enums | add `FullChunkStatus`, `Visibility`, and `visibilityFromFullChunkStatus(...)` with vanilla mapping |
| 2 | Entity access records | add a minimal runtime entity interface/record: id, UUID, type id, position, rotation, bounding box, removed reason, save/tick flags |
| 3 | Level callback | add `EntityInLevelCallback` equivalent and route position/removal changes through it |
| 4 | Entity section | add section-owned entity collection with add/remove, typed iteration if needed, and empty checks |
| 5 | Entity section storage | add packed-section-key storage, chunk grouping, and accessible AABB query helpers |
| 6 | Entity lookup | add visible entity lookup by numeric id and UUID |
| 7 | Entity getter adapter | add a query facade over visible lookup and accessible sections |
| 8 | Entity tick list | add vanilla copy-on-write tick list semantics for add/remove during iteration |
| 9 | Persistent section manager | add host-owned entity manager with add, legacy/worldgen add, status updates, move callback, remove callback, pending load/unload, saveAll/autoSave |
| 10 | Entity storage interface | add logical `EntityPersistentStorage` / `ChunkEntities` records and a memory implementation for tests |
| 11 | Host integration seam | add a small host-owned wrapper or construction seam so `GeneratedWorldHost`/Node can later own an entity manager without renderer access |
| 12 | Protocol type placeholders | add logical `entity_snapshot` / `entity_delta` types only if they stay shape-only and do not force renderer ingestion |

Recommended file layout:

```text
src/world/level/entity/
  chunk-entities.ts
  entity-access.ts
  entity-in-level-callback.ts
  entity-lookup.ts
  entity-section.ts
  entity-section-storage.ts
  entity-tick-list.ts
  entity-persistent-storage.ts
  full-chunk-status.ts
  level-entity-getter-adapter.ts
  persistent-entity-section-manager.ts
  visibility.ts

src/runtime/host/
  entity-runtime.ts
```

The exact TypeScript names can follow local style during implementation. Keep the semantics close enough that future ports can grep vanilla names and find the matching module.

## Minimal Entity Record

Start with a simple synthetic record, not a full `Entity` port:

```ts
interface RuntimeEntityAccess {
  readonly id: number;
  readonly uuid: string;
  readonly typeId: string;
  readonly isAlwaysTicking: boolean;
  readonly shouldBeSaved: boolean;
  readonly boundingBox: AABB;
  readonly position: { readonly x: number; readonly y: number; readonly z: number };
  readonly rotation: { readonly yaw: number; readonly pitch: number };
  readonly removalReason?: EntityRemovalReason;

  blockPosition(): BlockPos;
  setLevelCallback(callback: EntityInLevelCallback): void;
  setPosition(x: number, y: number, z: number): void;
  setRemoved(reason: EntityRemovalReason): void;
}
```

Implementation can use a class for the test entity if that makes callback wiring clearer. Avoid porting `Mob`, `LivingEntity`, data watchers, passengers, attributes, equipment, or AI here.

## Behavioral Requirements

### Visibility

Map full chunk status exactly:

| Full chunk status | Visibility |
|---|---|
| `INACCESSIBLE` | `HIDDEN` |
| `BORDER` | `TRACKED` |
| `TICKING` | `TRACKED` |
| `ENTITY_TICKING` | `TICKING` |

`isAlwaysTicking` upgrades effective visibility to `TICKING`, matching vanilla's player path.

### Add

Adding an entity should:

- reject duplicate UUIDs
- compute section key from block position
- insert into section storage
- install the level callback
- call creation callback unless loaded as legacy persistent data
- start tracking if effective visibility is accessible
- start ticking if effective visibility is ticking

### Move

Moving an entity should:

- update position and block position
- call the installed move callback
- move sections immediately if the section key changed
- remove empty old sections
- compare old and new effective visibility
- start/stop tracking and ticking on boundary changes

### Remove

Removing an entity should:

- remove from current section
- stop ticking if effective status was ticking
- stop tracking if effective status was accessible
- run destroyed callback only for destroy reasons
- remove UUID from the known set
- clear the level callback
- remove empty sections

Removal persistence flags should match vanilla:

| Reason | Destroy callback | Saved |
|---|---|---|
| `KILLED` | yes | no |
| `DISCARDED` | yes | no |
| `UNLOADED_TO_CHUNK` | no | yes |
| `UNLOADED_WITH_PLAYER` | no | no |
| `CHANGED_DIMENSION` | no | no |

### Tick List

`EntityTickList.forEach(...)` should reject nested iteration and preserve a stable iterator when entities are added or removed during the iteration.

Tests should prove that:

- adding during iteration does not tick the new entity in that same pass
- removing during iteration does not corrupt the active iterator
- the next pass sees the updated active set

### Load And Unload

The manager should support pending load and unload semantics:

- visible chunk status queues load if chunk entity state is fresh
- loaded entity chunks enter through `loadingInbox` and are added on `manager.tick()`
- hiding a chunk queues unload
- unload waits while the entity chunk load is pending
- successful unload stores saveable entities and removes them with `UNLOADED_TO_CHUNK`
- `saveAll()` drains pending loads and flushes storage

The memory test storage can resolve promises manually to make these states deterministic.

## Host And Storage Adaptation

`Entities0` should not force a physical Anvil writer. Add the logical storage interface first:

```ts
interface EntityPersistentStorage {
  loadEntities(chunkX: number, chunkZ: number): Promise<ChunkEntities>;
  storeEntities(chunk: ChunkEntities): void | Promise<void>;
  flush(sync: boolean): Promise<void>;
  close(): Promise<void>;
}
```

For runtime storage, keep block chunk storage and entity chunk storage separate. A later adapter can add `WorldStorageSession.entities` beside `WorldStorageSession.chunks`, or an entity-specific storage session if that is cleaner. Do not put entity records inside `PackedChunkSnapshot`.

Host integration should be narrow:

- host authority lane constructs and owns the entity manager
- chunk status publication calls `updateChunkStatus`
- host tick calls `entityManager.tick()` and then any entity ticking phase once there are test entities
- renderer/main thread gets no direct mutable entity references

If integrating into `GeneratedWorldHost` is too noisy for this first slice, add a small `EntityRuntime` host-side wrapper with unit tests and leave full `GeneratedWorldHost` wiring to `Entities1`. Do not add renderer ingestion just to prove the data structure.

## Protocol Shape

If protocol types are included in this slice, keep them logical and inert:

```ts
interface EntitySnapshotMessage {
  readonly type: "entity_snapshot";
  readonly tick: number;
  readonly revision: number;
  readonly entity: EntitySnapshot;
}

interface EntityDeltaMessage {
  readonly type: "entity_delta";
  readonly tick: number;
  readonly revision: number;
  readonly entityId: number;
  readonly position?: Vec3Record;
  readonly rotation?: EntityRotationRecord;
  readonly removedReason?: EntityRemovalReason;
}
```

Do not wire renderer consumption, interpolation, or entity model loading in `Entities0`. The only acceptable protocol test is type/codec preservation if message types are added.

## Tests

Focused test files:

```text
test/world/level/entity/visibility.test.ts
test/world/level/entity/entity-tick-list.test.ts
test/world/level/entity/entity-section-storage.test.ts
test/world/level/entity/persistent-entity-section-manager.test.ts
test/runtime/entity-runtime.test.ts
```

Required cases:

- `BORDER` and `TICKING` are tracked but not ticking
- `ENTITY_TICKING` starts ticking
- demotion from `ENTITY_TICKING` stops ticking but may keep tracking
- demotion below `BORDER` stops tracking and queues unload
- `isAlwaysTicking` stays tracked and ticking
- duplicate UUID add fails and does not create a section entry
- id/UUID lookup only returns tracked entities
- AABB query only scans accessible sections
- moving within a section does not churn section storage
- moving across sections transfers ownership and removes empty sections
- moving across chunks changes tracking/ticking based on destination chunk visibility
- removal reasons drive destroyed callback and save eligibility
- tick-list add/remove during iteration matches vanilla copy-on-write behavior
- pending loads are applied only on manager tick
- hidden chunk unload waits for pending load and then stores saveable entities
- `saveAll()` flushes storage and persists loaded chunks

Validation commands:

```bash
pnpm test -- test/world/level/entity test/runtime/entity-runtime.test.ts
pnpm typecheck
```

No browser screenshot is required because `Entities0` must not produce pixels.

## Done When

- done: entity manager modules exist under `src/world/level/entity/`
- done: runtime entity records can be added, moved, removed, tracked, and ticked in unit tests
- done: full chunk status changes drive tracking/ticking exactly as vanilla visibility mapping requires
- done: tick-list mutation during iteration is covered by tests
- done: logical entity chunk storage exists separately from packed block chunk snapshots
- done: no `NaturalSpawner`, AI/pathfinding, renderer, model, or creature spawn code was added
- done: `Creatures0` remains an oracle fixture slice and is not folded into runtime work
- done: tests and typecheck pass

## Prepares For Creatures1

`Entities0` gives `Creatures1` the insertion and persistence target it needs.

`Creatures1` can port generation-time original creature spawning and insert results through a method shaped like `ServerLevel.addWorldGenChunkEntities(...)`. The result should be normal host-owned entities with UUIDs, section ownership, save eligibility, tracking status, and future protocol snapshots.

After that, later slices can add:

- `MobCategory` and spawn-count queries over visible entities
- biome spawn settings and spawn placement checks
- `NaturalSpawner.spawnMobsForChunkGeneration(...)`
- live natural spawning in `ENTITY_TICKING` chunks
- mob despawn and persistence flags
- AI/pathfinding
- renderer presentation
