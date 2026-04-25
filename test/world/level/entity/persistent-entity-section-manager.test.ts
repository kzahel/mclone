import { describe, expect, test } from "vitest";
import { FullChunkStatus } from "../../../../src/world/level/entity/full-chunk-status";
import {
  createChunkEntities,
  entityChunkKey,
  type ChunkEntities,
} from "../../../../src/world/level/entity/chunk-entities";
import {
  EntityRemovalReason,
  SyntheticRuntimeEntity,
  entityRemovalReasonShouldDestroy,
  entityRemovalReasonShouldSave,
} from "../../../../src/world/level/entity/entity-access";
import {
  MemoryEntityPersistentStorage,
  type EntityPersistentStorage,
} from "../../../../src/world/level/entity/entity-persistent-storage";
import {
  ChunkLoadStatus,
  PersistentEntitySectionManager,
  type LevelCallback,
} from "../../../../src/world/level/entity/persistent-entity-section-manager";

function entity(
  id: number,
  x = 1,
  y = 64,
  z = 1,
  options: { readonly uuid?: string; readonly alwaysTicking?: boolean; readonly save?: boolean } = {},
): SyntheticRuntimeEntity {
  return new SyntheticRuntimeEntity({
    id,
    uuid: options.uuid ?? `uuid-${id}`,
    x,
    y,
    z,
    alwaysTicking: options.alwaysTicking,
    save: options.save,
  });
}

function createManager(storage: EntityPersistentStorage<SyntheticRuntimeEntity> = new MemoryEntityPersistentStorage()) {
  const events: string[] = [];
  const callbacks: LevelCallback<SyntheticRuntimeEntity> = {
    onCreated: (current) => events.push(`created:${current.id}`),
    onDestroyed: (current) => events.push(`destroyed:${current.id}`),
    onTickingStart: (current) => events.push(`ticking_start:${current.id}`),
    onTickingEnd: (current) => events.push(`ticking_end:${current.id}`),
    onTrackingStart: (current) => events.push(`tracking_start:${current.id}`),
    onTrackingEnd: (current) => events.push(`tracking_end:${current.id}`),
  };

  return {
    events,
    manager: new PersistentEntitySectionManager<SyntheticRuntimeEntity>(callbacks, storage),
  };
}

class ManualEntityStorage implements EntityPersistentStorage<SyntheticRuntimeEntity> {
  public readonly stored: ChunkEntities<SyntheticRuntimeEntity>[] = [];
  private readonly pendingLoads = new Map<string, (chunk: ChunkEntities<SyntheticRuntimeEntity>) => void>();

  public loadEntities(chunkX: number, chunkZ: number): Promise<ChunkEntities<SyntheticRuntimeEntity>> {
    return new Promise((resolve) => {
      this.pendingLoads.set(entityChunkKey(chunkX, chunkZ), resolve);
    });
  }

  public resolveLoad(chunkX: number, chunkZ: number, entities: readonly SyntheticRuntimeEntity[] = []): void {
    const key = entityChunkKey(chunkX, chunkZ);
    const resolve = this.pendingLoads.get(key);
    if (resolve === undefined) {
      throw new Error(`No pending entity load for ${key}`);
    }

    this.pendingLoads.delete(key);
    resolve(createChunkEntities(chunkX, chunkZ, entities));
  }

  public storeEntities(chunk: ChunkEntities<SyntheticRuntimeEntity>): void {
    this.stored.push(createChunkEntities(chunk.chunkX, chunk.chunkZ, chunk.entities));
  }

  public async flush(_sync: boolean): Promise<void> {}

  public async close(): Promise<void> {}
}

describe("PersistentEntitySectionManager", () => {
  test("drives tracking and ticking from full chunk status", () => {
    const { manager, events } = createManager();
    const sheep = entity(1);

    expect(manager.addNewEntity(sheep)).toBe(true);
    expect(events).toEqual(["created:1"]);
    expect(manager.getEntityGetter().get(1)).toBeUndefined();

    manager.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    expect(events).toEqual(["created:1", "tracking_start:1"]);
    expect(manager.getEntityGetter().get(1)).toBe(sheep);

    manager.updateChunkStatus(0, 0, FullChunkStatus.TICKING);
    expect(events).toEqual(["created:1", "tracking_start:1"]);

    manager.updateChunkStatus(0, 0, FullChunkStatus.ENTITY_TICKING);
    expect(events).toEqual(["created:1", "tracking_start:1", "ticking_start:1"]);

    manager.updateChunkStatus(0, 0, FullChunkStatus.TICKING);
    expect(events).toEqual(["created:1", "tracking_start:1", "ticking_start:1", "ticking_end:1"]);
    expect(manager.getEntityGetter().get("uuid-1")).toBe(sheep);

    manager.updateChunkStatus(0, 0, FullChunkStatus.INACCESSIBLE);
    expect(events).toEqual([
      "created:1",
      "tracking_start:1",
      "ticking_start:1",
      "ticking_end:1",
      "tracking_end:1",
    ]);
    expect(manager.getEntityGetter().get(1)).toBeUndefined();
  });

  test("keeps always-ticking entities visible and ticking even in hidden chunks", () => {
    const { manager, events } = createManager();
    const playerLike = entity(1, 1, 64, 1, { alwaysTicking: true });

    expect(manager.addNewEntity(playerLike)).toBe(true);
    expect(events).toEqual(["created:1", "tracking_start:1", "ticking_start:1"]);
    expect(manager.getEntityGetter().get(1)).toBe(playerLike);

    manager.updateChunkStatus(0, 0, FullChunkStatus.INACCESSIBLE);
    expect(events).toEqual(["created:1", "tracking_start:1", "ticking_start:1"]);
    expect(manager.getEntityGetter().get(1)).toBe(playerLike);
  });

  test("rejects duplicate UUIDs before creating section entries", () => {
    const { manager, events } = createManager();

    expect(manager.addNewEntity(entity(1, 1, 64, 1, { uuid: "same" }))).toBe(true);
    expect(manager.addNewEntity(entity(2, 1, 64, 1, { uuid: "same" }))).toBe(false);
    expect(events).toEqual(["created:1"]);
    expect(manager.getSectionStorage().count()).toBe(1);
  });

  test("moves entities across sections and chunk visibility boundaries", () => {
    const { manager, events } = createManager();
    manager.updateChunkStatus(0, 0, FullChunkStatus.ENTITY_TICKING);
    manager.updateChunkStatus(1, 0, FullChunkStatus.BORDER);
    const sheep = entity(1, 1, 64, 1);
    manager.addNewEntity(sheep);
    events.length = 0;

    sheep.setPosition(2, 64, 2);
    expect(manager.getSectionStorage().count()).toBe(1);
    expect(events).toEqual([]);

    sheep.setPosition(17, 64, 2);
    expect(manager.getSectionStorage().count()).toBe(1);
    expect(events).toEqual(["ticking_end:1"]);
    expect(manager.getEntityGetter().get(1)).toBe(sheep);

    sheep.setPosition(33, 64, 2);
    expect(events).toEqual(["ticking_end:1", "tracking_end:1"]);
    expect(manager.getEntityGetter().get(1)).toBeUndefined();
  });

  test("applies removal reason callbacks and persistence flags", () => {
    expect(entityRemovalReasonShouldDestroy(EntityRemovalReason.KILLED)).toBe(true);
    expect(entityRemovalReasonShouldSave(EntityRemovalReason.KILLED)).toBe(false);
    expect(entityRemovalReasonShouldDestroy(EntityRemovalReason.UNLOADED_TO_CHUNK)).toBe(false);
    expect(entityRemovalReasonShouldSave(EntityRemovalReason.UNLOADED_TO_CHUNK)).toBe(true);

    const { manager, events } = createManager();
    manager.updateChunkStatus(0, 0, FullChunkStatus.ENTITY_TICKING);
    const sheep = entity(1);
    manager.addNewEntity(sheep);
    events.length = 0;

    sheep.setRemoved(EntityRemovalReason.KILLED);
    expect(events).toEqual(["ticking_end:1", "tracking_end:1", "destroyed:1"]);
    expect(manager.isLoaded("uuid-1")).toBe(false);
    expect(manager.getSectionStorage().count()).toBe(0);
    expect(sheep.shouldBeSaved()).toBe(false);
  });

  test("applies pending loads only on manager tick", async () => {
    const storage = new ManualEntityStorage();
    const { manager, events } = createManager(storage);
    const loaded = entity(1);

    manager.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    expect(manager.getChunkLoadStatusFor(0, 0)).toBe(ChunkLoadStatus.PENDING);
    storage.resolveLoad(0, 0, [loaded]);
    await Promise.resolve();

    expect(manager.getEntityGetter().get(1)).toBeUndefined();
    manager.tick();

    expect(manager.getChunkLoadStatusFor(0, 0)).toBe(ChunkLoadStatus.LOADED);
    expect(manager.getEntityGetter().get(1)).toBe(loaded);
    expect(events).toEqual(["tracking_start:1"]);
  });

  test("waits for pending load before hidden chunk unload stores saveable entities", async () => {
    const storage = new ManualEntityStorage();
    const { manager } = createManager(storage);
    const loaded = entity(1);

    manager.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    manager.updateChunkStatus(0, 0, FullChunkStatus.INACCESSIBLE);
    manager.tick();

    expect(storage.stored).toEqual([]);
    expect(manager.getChunkLoadStatusFor(0, 0)).toBe(ChunkLoadStatus.PENDING);

    storage.resolveLoad(0, 0, [loaded]);
    await Promise.resolve();
    manager.tick();

    expect(storage.stored).toHaveLength(1);
    expect(storage.stored[0]?.entities.map((current) => current.id)).toEqual([1]);
    expect(loaded.removalReason).toBe(EntityRemovalReason.UNLOADED_TO_CHUNK);
    expect(manager.getChunkLoadStatusFor(0, 0)).toBe(ChunkLoadStatus.FRESH);
    expect(manager.getSectionStorage().count()).toBe(0);
  });

  test("saveAll persists loaded visible chunks", async () => {
    const storage = new MemoryEntityPersistentStorage<SyntheticRuntimeEntity>();
    const { manager } = createManager(storage);
    const sheep = entity(1);

    manager.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    await Promise.resolve();
    manager.tick();
    expect(manager.areEntitiesLoaded(0, 0)).toBe(true);

    manager.addNewEntity(sheep);
    await manager.saveAll();

    expect(storage.getStoredChunk(0, 0)?.entities.map((current) => current.id)).toEqual([1]);
  });
});
