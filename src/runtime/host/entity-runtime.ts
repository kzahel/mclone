import {
  EntityTickList,
} from "../../world/level/entity/entity-tick-list";
import {
  MemoryEntityPersistentStorage,
  type EntityPersistentStorage,
} from "../../world/level/entity/entity-persistent-storage";
import {
  PersistentEntitySectionManager,
  type LevelCallback,
} from "../../world/level/entity/persistent-entity-section-manager";
import type { FullChunkStatus } from "../../world/level/entity/full-chunk-status";
import type { RuntimeEntityAccess } from "../../world/level/entity/entity-access";

export interface EntityRuntimeOptions<T extends RuntimeEntityAccess> {
  readonly storage?: EntityPersistentStorage<T>;
  readonly callbacks?: Partial<LevelCallback<T>>;
  readonly tickEntity?: (entity: T) => void;
}

export class EntityRuntime<T extends RuntimeEntityAccess> {
  public readonly tickList = new EntityTickList<T>();
  public readonly manager: PersistentEntitySectionManager<T>;
  private readonly tickEntity: (entity: T) => void;

  public constructor(options: EntityRuntimeOptions<T> = {}) {
    const externalCallbacks = options.callbacks;
    this.tickEntity = options.tickEntity ?? (() => {});
    this.manager = new PersistentEntitySectionManager<T>(
      {
        onCreated: (entity) => externalCallbacks?.onCreated?.(entity),
        onDestroyed: (entity) => externalCallbacks?.onDestroyed?.(entity),
        onTickingStart: (entity) => {
          this.tickList.add(entity);
          externalCallbacks?.onTickingStart?.(entity);
        },
        onTickingEnd: (entity) => {
          this.tickList.remove(entity);
          externalCallbacks?.onTickingEnd?.(entity);
        },
        onTrackingStart: (entity) => externalCallbacks?.onTrackingStart?.(entity),
        onTrackingEnd: (entity) => externalCallbacks?.onTrackingEnd?.(entity),
      },
      options.storage ?? new MemoryEntityPersistentStorage<T>(),
    );
  }

  public addEntity(entity: T): boolean {
    return this.manager.addNewEntity(entity);
  }

  public addWorldGenChunkEntities(entities: Iterable<T>): void {
    this.manager.addWorldGenChunkEntities(entities);
  }

  public updateChunkStatus(chunkX: number, chunkZ: number, status: FullChunkStatus): void {
    this.manager.updateChunkStatus(chunkX, chunkZ, status);
  }

  public processLifecycle(): void {
    this.manager.tick();
  }

  public tick(): void {
    this.processLifecycle();
    this.tickList.forEach(this.tickEntity);
  }
}
