import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";
import {
  createChunkEntities,
  entityChunkKey,
  entityChunkKeyFromBlockPos,
  parseEntityChunkKey,
  type ChunkEntities,
} from "./chunk-entities";
import {
  EntityRemovalReason,
  entityRemovalReasonShouldDestroy,
  type RuntimeEntityAccess,
} from "./entity-access";
import type { EntityInLevelCallback } from "./entity-in-level-callback";
import { NULL_ENTITY_IN_LEVEL_CALLBACK } from "./entity-in-level-callback";
import { EntityLookup } from "./entity-lookup";
import type { EntityPersistentStorage } from "./entity-persistent-storage";
import { EntitySection } from "./entity-section";
import { EntitySectionStorage } from "./entity-section-storage";
import { LevelEntityGetterAdapter } from "./level-entity-getter-adapter";
import type { FullChunkStatus } from "./full-chunk-status";
import {
  isVisibilityAccessible,
  isVisibilityTicking,
  visibilityFromFullChunkStatus,
  Visibility,
  type Visibility as VisibilityValue,
} from "./visibility";

export interface LevelCallback<T extends RuntimeEntityAccess> {
  onCreated(entity: T): void;
  onDestroyed(entity: T): void;
  onTickingStart(entity: T): void;
  onTickingEnd(entity: T): void;
  onTrackingStart(entity: T): void;
  onTrackingEnd(entity: T): void;
}

export const ChunkLoadStatus = {
  FRESH: "fresh",
  PENDING: "pending",
  LOADED: "loaded",
} as const;

export type ChunkLoadStatus = typeof ChunkLoadStatus[keyof typeof ChunkLoadStatus];

export class PersistentEntitySectionManager<T extends RuntimeEntityAccess> {
  private readonly knownUuids = new Set<string>();
  private readonly visibleEntityStorage = new EntityLookup<T>();
  private readonly sectionStorage: EntitySectionStorage<T>;
  private readonly entityGetter: LevelEntityGetterAdapter<T>;
  private readonly chunkVisibility = new Map<string, VisibilityValue>();
  private readonly chunkLoadStatuses = new Map<string, ChunkLoadStatus>();
  private readonly chunksToUnload = new Set<string>();
  private readonly loadingInbox: ChunkEntities<T>[] = [];

  public constructor(
    private readonly callbacks: LevelCallback<T>,
    private readonly permanentStorage: EntityPersistentStorage<T>,
  ) {
    this.sectionStorage = new EntitySectionStorage<T>((chunkX, chunkZ) => this.getChunkVisibility(chunkX, chunkZ));
    this.entityGetter = new LevelEntityGetterAdapter<T>(this.visibleEntityStorage, this.sectionStorage);
  }

  public static getEffectiveStatus<TAccess extends RuntimeEntityAccess>(
    entity: TAccess,
    visibility: VisibilityValue,
  ): VisibilityValue {
    return entity.isAlwaysTicking() ? Visibility.TICKING : visibility;
  }

  public addNewEntity(entity: T): boolean {
    return this.addEntity(entity, false);
  }

  public addLegacyChunkEntities(entities: Iterable<T>): void {
    for (const entity of entities) {
      this.addEntity(entity, true);
    }
  }

  public addWorldGenChunkEntities(entities: Iterable<T>): void {
    for (const entity of entities) {
      this.addEntity(entity, false);
    }
  }

  public updateChunkStatus(chunkX: number, chunkZ: number, status: FullChunkStatus): void {
    this.updateChunkVisibility(chunkX, chunkZ, visibilityFromFullChunkStatus(status));
  }

  public updateChunkVisibility(chunkX: number, chunkZ: number, visibility: VisibilityValue): void {
    const key = entityChunkKey(chunkX, chunkZ);
    if (visibility === Visibility.HIDDEN) {
      this.chunkVisibility.delete(key);
      this.chunksToUnload.add(key);
    } else {
      this.chunkVisibility.set(key, visibility);
      this.chunksToUnload.delete(key);
      this.ensureChunkQueuedForLoad(key);
    }

    for (const section of this.sectionStorage.getExistingSectionsInChunk(chunkX, chunkZ)) {
      const previous = section.updateChunkStatus(visibility);
      const wasAccessible = isVisibilityAccessible(previous);
      const isAccessible = isVisibilityAccessible(visibility);
      const wasTicking = isVisibilityTicking(previous);
      const isTicking = isVisibilityTicking(visibility);

      if (wasTicking && !isTicking) {
        for (const entity of section.getEntities()) {
          if (!entity.isAlwaysTicking()) {
            this.stopTicking(entity);
          }
        }
      }

      if (wasAccessible && !isAccessible) {
        for (const entity of section.getEntities()) {
          if (!entity.isAlwaysTicking()) {
            this.stopTracking(entity);
          }
        }
      } else if (!wasAccessible && isAccessible) {
        for (const entity of section.getEntities()) {
          if (!entity.isAlwaysTicking()) {
            this.startTracking(entity);
          }
        }
      }

      if (!wasTicking && isTicking) {
        for (const entity of section.getEntities()) {
          if (!entity.isAlwaysTicking()) {
            this.startTicking(entity);
          }
        }
      }
    }
  }

  public tick(): void {
    this.processPendingLoads();
    this.processUnloads();
  }

  public autoSave(): void {
    for (const key of this.getAllChunksToSave()) {
      if (this.getChunkVisibilityByKey(key) === Visibility.HIDDEN) {
        this.processChunkUnload(key);
      } else {
        this.storeChunkSections(key, () => {});
      }
    }
  }

  public async saveAll(): Promise<void> {
    const chunksToSave = this.getAllChunksToSave();
    while (chunksToSave.size > 0) {
      await this.permanentStorage.flush(false);
      this.processPendingLoads();

      for (const key of [...chunksToSave]) {
        const stored = this.getChunkVisibilityByKey(key) === Visibility.HIDDEN
          ? this.processChunkUnload(key)
          : this.storeChunkSections(key, () => {});
        if (stored) {
          chunksToSave.delete(key);
        }
      }

      if (chunksToSave.size > 0) {
        await Promise.resolve();
      }
    }

    await this.permanentStorage.flush(true);
  }

  public async close(): Promise<void> {
    await this.saveAll();
    await this.permanentStorage.close();
  }

  public isLoaded(uuid: string): boolean {
    return this.knownUuids.has(uuid);
  }

  public getEntityGetter(): LevelEntityGetterAdapter<T> {
    return this.entityGetter;
  }

  public isPositionTicking(pos: BlockPos): boolean;
  public isPositionTicking(chunkX: number, chunkZ: number): boolean;
  public isPositionTicking(first: BlockPos | number, second?: number): boolean {
    if (first instanceof BlockPos) {
      return isVisibilityTicking(this.getChunkVisibilityByKey(entityChunkKeyFromBlockPos(first)));
    }

    return isVisibilityTicking(this.getChunkVisibility(first, second ?? 0));
  }

  public areEntitiesLoaded(chunkX: number, chunkZ: number): boolean {
    return this.getChunkLoadStatus(entityChunkKey(chunkX, chunkZ)) === ChunkLoadStatus.LOADED;
  }

  public getSectionStorage(): EntitySectionStorage<T> {
    return this.sectionStorage;
  }

  public getChunkLoadStatusFor(chunkX: number, chunkZ: number): ChunkLoadStatus {
    return this.getChunkLoadStatus(entityChunkKey(chunkX, chunkZ));
  }

  public gatherStats(): string {
    return [
      this.knownUuids.size,
      this.visibleEntityStorage.count(),
      this.sectionStorage.count(),
      this.chunkLoadStatuses.size,
      this.chunkVisibility.size,
      this.loadingInbox.length,
      this.chunksToUnload.size,
    ].join(",");
  }

  private addEntity(entity: T, loadedFromDisk: boolean): boolean {
    if (!this.addEntityUuid(entity)) {
      return false;
    }

    const sectionKey = SectionPos.asLongFromBlockPos(entity.blockPosition());
    const section = this.sectionStorage.getOrCreateSection(sectionKey);
    section.add(entity);
    entity.setLevelCallback(this.createEntityCallback(entity, sectionKey, section));
    if (!loadedFromDisk) {
      this.callbacks.onCreated(entity);
    }

    const visibility = PersistentEntitySectionManager.getEffectiveStatus(entity, section.getStatus());
    if (isVisibilityAccessible(visibility)) {
      this.startTracking(entity);
    }

    if (isVisibilityTicking(visibility)) {
      this.startTicking(entity);
    }

    return true;
  }

  private addEntityUuid(entity: T): boolean {
    if (this.knownUuids.has(entity.uuid)) {
      return false;
    }

    this.knownUuids.add(entity.uuid);
    return true;
  }

  private createEntityCallback(entity: T, sectionKey: bigint, section: EntitySection<T>): EntityInLevelCallback {
    let currentSectionKey = sectionKey;
    let currentSection = section;
    return {
      onMove: () => {
        const nextSectionKey = SectionPos.asLongFromBlockPos(entity.blockPosition());
        if (nextSectionKey === currentSectionKey) {
          return;
        }

        const previousVisibility = currentSection.getStatus();
        currentSection.remove(entity);
        this.removeSectionIfEmpty(currentSectionKey, currentSection);

        const nextSection = this.sectionStorage.getOrCreateSection(nextSectionKey);
        nextSection.add(entity);
        currentSection = nextSection;
        currentSectionKey = nextSectionKey;
        this.updateEntityStatus(entity, previousVisibility, nextSection.getStatus());
      },
      onRemove: (reason) => {
        currentSection.remove(entity);
        const visibility = PersistentEntitySectionManager.getEffectiveStatus(entity, currentSection.getStatus());
        if (isVisibilityTicking(visibility)) {
          this.stopTicking(entity);
        }

        if (isVisibilityAccessible(visibility)) {
          this.stopTracking(entity);
        }

        if (entityRemovalReasonShouldDestroy(reason)) {
          this.callbacks.onDestroyed(entity);
        }

        this.knownUuids.delete(entity.uuid);
        entity.setLevelCallback(NULL_ENTITY_IN_LEVEL_CALLBACK);
        this.removeSectionIfEmpty(currentSectionKey, currentSection);
      },
    };
  }

  private updateEntityStatus(entity: T, previous: VisibilityValue, next: VisibilityValue): void {
    const previousEffective = PersistentEntitySectionManager.getEffectiveStatus(entity, previous);
    const nextEffective = PersistentEntitySectionManager.getEffectiveStatus(entity, next);
    if (previousEffective === nextEffective) {
      return;
    }

    const wasAccessible = isVisibilityAccessible(previousEffective);
    const isAccessible = isVisibilityAccessible(nextEffective);
    if (wasAccessible && !isAccessible) {
      this.stopTracking(entity);
    } else if (!wasAccessible && isAccessible) {
      this.startTracking(entity);
    }

    const wasTicking = isVisibilityTicking(previousEffective);
    const isTicking = isVisibilityTicking(nextEffective);
    if (wasTicking && !isTicking) {
      this.stopTicking(entity);
    } else if (!wasTicking && isTicking) {
      this.startTicking(entity);
    }
  }

  private startTicking(entity: T): void {
    this.callbacks.onTickingStart(entity);
  }

  private stopTicking(entity: T): void {
    this.callbacks.onTickingEnd(entity);
  }

  private startTracking(entity: T): void {
    this.visibleEntityStorage.add(entity);
    this.callbacks.onTrackingStart(entity);
  }

  private stopTracking(entity: T): void {
    this.callbacks.onTrackingEnd(entity);
    this.visibleEntityStorage.remove(entity);
  }

  private ensureChunkQueuedForLoad(key: string): void {
    if (this.getChunkLoadStatus(key) === ChunkLoadStatus.FRESH) {
      this.requestChunkLoad(key);
    }
  }

  private requestChunkLoad(key: string): void {
    this.chunkLoadStatuses.set(key, ChunkLoadStatus.PENDING);
    const chunk = parseEntityChunkKey(key);
    void this.permanentStorage.loadEntities(chunk.chunkX, chunk.chunkZ)
      .then((entities) => {
        this.loadingInbox.push(entities);
      });
  }

  private storeChunkSections(key: string, afterStored: (entity: T) => void): boolean {
    const status = this.getChunkLoadStatus(key);
    if (status === ChunkLoadStatus.PENDING) {
      return false;
    }

    const chunk = parseEntityChunkKey(key);
    const saveableEntities: T[] = [];
    for (const section of this.sectionStorage.getExistingSectionsInChunk(chunk.chunkX, chunk.chunkZ)) {
      for (const entity of section.getEntities()) {
        if (entity.shouldBeSaved()) {
          saveableEntities.push(entity);
        }
      }
    }

    if (saveableEntities.length === 0) {
      if (status === ChunkLoadStatus.LOADED) {
        void this.permanentStorage.storeEntities(createChunkEntities(chunk.chunkX, chunk.chunkZ));
      }
      return true;
    }

    if (status === ChunkLoadStatus.FRESH) {
      this.requestChunkLoad(key);
      return false;
    }

    void this.permanentStorage.storeEntities(createChunkEntities(chunk.chunkX, chunk.chunkZ, saveableEntities));
    for (const entity of saveableEntities) {
      afterStored(entity);
    }
    return true;
  }

  private processChunkUnload(key: string): boolean {
    const stored = this.storeChunkSections(key, (entity) => {
      for (const selfOrPassenger of entity.getPassengersAndSelf()) {
        this.unloadEntity(selfOrPassenger);
      }
    });
    if (!stored) {
      return false;
    }

    this.chunkLoadStatuses.delete(key);
    return true;
  }

  private unloadEntity(entity: RuntimeEntityAccess): void {
    entity.setRemoved(EntityRemovalReason.UNLOADED_TO_CHUNK);
    entity.setLevelCallback(NULL_ENTITY_IN_LEVEL_CALLBACK);
  }

  private processUnloads(): void {
    for (const key of [...this.chunksToUnload]) {
      if (this.getChunkVisibilityByKey(key) !== Visibility.HIDDEN || this.processChunkUnload(key)) {
        this.chunksToUnload.delete(key);
      }
    }
  }

  private processPendingLoads(): void {
    let chunk: ChunkEntities<T> | undefined;
    while ((chunk = this.loadingInbox.shift()) !== undefined) {
      this.addLegacyChunkEntities(chunk.entities);
      this.chunkLoadStatuses.set(entityChunkKey(chunk.chunkX, chunk.chunkZ), ChunkLoadStatus.LOADED);
    }
  }

  private getAllChunksToSave(): Set<string> {
    const chunks = new Set<string>();
    for (const chunk of this.sectionStorage.getAllChunksWithExistingSections()) {
      chunks.add(entityChunkKey(chunk.chunkX, chunk.chunkZ));
    }

    for (const [key, status] of this.chunkLoadStatuses) {
      if (status === ChunkLoadStatus.LOADED) {
        chunks.add(key);
      }
    }

    return chunks;
  }

  private getChunkVisibility(chunkX: number, chunkZ: number): VisibilityValue {
    return this.getChunkVisibilityByKey(entityChunkKey(chunkX, chunkZ));
  }

  private getChunkVisibilityByKey(key: string): VisibilityValue {
    return this.chunkVisibility.get(key) ?? Visibility.HIDDEN;
  }

  private getChunkLoadStatus(key: string): ChunkLoadStatus {
    return this.chunkLoadStatuses.get(key) ?? ChunkLoadStatus.FRESH;
  }

  private removeSectionIfEmpty(sectionKey: bigint, section: EntitySection<T>): void {
    if (section.isEmpty()) {
      this.sectionStorage.remove(sectionKey);
    }
  }
}
