import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { SectionPos } from "../../../core/section-pos";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import { DataLayerStorageMap } from "./data-layer-storage-map";
import { LIGHT_SELF_SOURCE, SectionTracker } from "./section-tracker";

const MAX_UPDATE_BUDGET = 2147483647;
const DIRECTIONS = Direction.values();

export interface LightEngineStorageAccess {
  readonly selfSource: bigint;
  getQueueSize(): number;
  removeIf(predicate: (pos: bigint) => boolean): void;
  removeFromQueue(pos: bigint): void;
  checkEdgeForStorage(source: bigint, target: bigint, candidateLevel: number, decrease: boolean): void;
  computeLevelFromNeighborForStorage(source: bigint, target: bigint, sourceLevel: number): number;
  getLevelForStorage(pos: bigint): number;
}

export abstract class LayerLightSectionStorage<M extends DataLayerStorageMap<M>> extends SectionTracker {
  protected static readonly LIGHT_AND_DATA = 0;
  protected static readonly LIGHT_ONLY = 1;
  protected static readonly EMPTY = 2;
  protected static readonly EMPTY_DATA = new DataLayer();

  protected readonly dataSectionSet = new Set<bigint>();
  protected readonly toMarkNoData = new Set<bigint>();
  protected readonly toMarkData = new Set<bigint>();
  protected visibleSectionData: M;
  protected readonly updatingSectionData: M;
  protected readonly changedSections = new Set<bigint>();
  protected readonly sectionsAffectedByLightUpdates = new Set<bigint>();
  protected readonly queuedSections = new Map<bigint, DataLayer>();
  private readonly untrustedSections = new Set<bigint>();
  private readonly columnsToRetainQueuedDataFor = new Set<bigint>();
  private readonly toRemove = new Set<bigint>();
  protected hasToRemove = false;

  protected constructor(
    private readonly layer: LightLayer,
    private readonly chunkSource: LightChunkGetter,
    storageMap: M,
  ) {
    super();
    this.updatingSectionData = storageMap;
    this.visibleSectionData = storageMap.copy();
    this.visibleSectionData.disableCache();
  }

  public storingLightForSection(section: bigint): boolean {
    return this.getDataLayer(section, true) !== undefined;
  }

  public getDataLayer(section: bigint, updating: boolean): DataLayer | undefined {
    return this.getDataLayerFromMap(updating ? this.updatingSectionData : this.visibleSectionData, section);
  }

  protected getDataLayerFromMap(storageMap: M, section: bigint): DataLayer | undefined {
    return storageMap.getLayer(section);
  }

  public getDataLayerData(section: bigint): DataLayer | undefined {
    return this.queuedSections.get(section) ?? this.getDataLayer(section, false);
  }

  public getLevelForDebug(section: bigint): number {
    return this.getLevel(section);
  }

  public abstract getLightValue(pos: bigint): number;

  public getStoredLevel(pos: bigint): number {
    const section = SectionPos.blockToSection(pos);
    const dataLayer = this.getDataLayer(section, true);
    if (dataLayer === undefined) {
      return 0;
    }

    return dataLayer.get(
      SectionPos.sectionRelative(BlockPos.getX(pos)),
      SectionPos.sectionRelative(BlockPos.getY(pos)),
      SectionPos.sectionRelative(BlockPos.getZ(pos)),
    );
  }

  public setStoredLevel(pos: bigint, level: number): void {
    const x = BlockPos.getX(pos);
    const y = BlockPos.getY(pos);
    const z = BlockPos.getZ(pos);
    const section = SectionPos.blockToSection(pos);
    if (!this.changedSections.has(section)) {
      this.changedSections.add(section);
      this.updatingSectionData.copyDataLayer(section);
    }

    const dataLayer = this.getDataLayer(section, true);
    if (dataLayer === undefined) {
      throw new Error(`Cannot set missing light data layer ${section.toString()}`);
    }

    dataLayer.set(
      SectionPos.sectionRelative(x),
      SectionPos.sectionRelative(y),
      SectionPos.sectionRelative(z),
      level,
    );

    const sectionX = SectionPos.blockToSectionCoord(x);
    const sectionY = SectionPos.blockToSectionCoord(y);
    const sectionZ = SectionPos.blockToSectionCoord(z);
    const localX = SectionPos.sectionRelative(x);
    const localY = SectionPos.sectionRelative(y);
    const localZ = SectionPos.sectionRelative(z);
    const minSectionX = localX === 0 ? sectionX - 1 : sectionX;
    const maxSectionX = localX === 15 ? sectionX + 1 : sectionX;
    const minSectionY = localY === 0 ? sectionY - 1 : sectionY;
    const maxSectionY = localY === 15 ? sectionY + 1 : sectionY;
    const minSectionZ = localZ === 0 ? sectionZ - 1 : sectionZ;
    const maxSectionZ = localZ === 15 ? sectionZ + 1 : sectionZ;
    for (let affectedX = minSectionX; affectedX <= maxSectionX; affectedX++) {
      for (let affectedY = minSectionY; affectedY <= maxSectionY; affectedY++) {
        for (let affectedZ = minSectionZ; affectedZ <= maxSectionZ; affectedZ++) {
          this.sectionsAffectedByLightUpdates.add(SectionPos.asLong(affectedX, affectedY, affectedZ));
        }
      }
    }
  }

  protected override getLevel(section: bigint): number {
    if (section === LIGHT_SELF_SOURCE) {
      return LayerLightSectionStorage.EMPTY;
    }
    if (this.dataSectionSet.has(section)) {
      return LayerLightSectionStorage.LIGHT_AND_DATA;
    }
    return !this.toRemove.has(section) && this.updatingSectionData.hasLayer(section)
      ? LayerLightSectionStorage.LIGHT_ONLY
      : LayerLightSectionStorage.EMPTY;
  }

  protected override getLevelFromSource(section: bigint): number {
    if (this.toMarkNoData.has(section)) {
      return LayerLightSectionStorage.EMPTY;
    }
    return !this.dataSectionSet.has(section) && !this.toMarkData.has(section)
      ? LayerLightSectionStorage.EMPTY
      : LayerLightSectionStorage.LIGHT_AND_DATA;
  }

  protected override setLevel(section: bigint, level: number): void {
    const oldLevel = this.getLevel(section);
    if (oldLevel !== LayerLightSectionStorage.LIGHT_AND_DATA && level === LayerLightSectionStorage.LIGHT_AND_DATA) {
      this.dataSectionSet.add(section);
      this.toMarkData.delete(section);
    }

    if (oldLevel === LayerLightSectionStorage.LIGHT_AND_DATA && level !== LayerLightSectionStorage.LIGHT_AND_DATA) {
      this.dataSectionSet.delete(section);
      this.toMarkNoData.delete(section);
    }

    if (oldLevel >= LayerLightSectionStorage.EMPTY && level !== LayerLightSectionStorage.EMPTY) {
      if (this.toRemove.has(section)) {
        this.toRemove.delete(section);
      } else {
        this.updatingSectionData.setLayer(section, this.createDataLayer(section));
        this.changedSections.add(section);
        this.onNodeAdded(section);

        for (let dx = -1; dx <= 1; dx++) {
          for (let dy = -1; dy <= 1; dy++) {
            for (let dz = -1; dz <= 1; dz++) {
              this.sectionsAffectedByLightUpdates.add(SectionPos.blockToSection(BlockPos.offset(section, dx, dy, dz)));
            }
          }
        }
      }
    }

    if (oldLevel !== LayerLightSectionStorage.EMPTY && level >= LayerLightSectionStorage.EMPTY) {
      this.toRemove.add(section);
    }

    this.hasToRemove = this.toRemove.size > 0;
  }

  protected createDataLayer(section: bigint): DataLayer {
    return this.queuedSections.get(section) ?? new DataLayer();
  }

  protected clearQueuedSectionBlocks(lightEngine: LightEngineStorageAccess, section: bigint): void {
    if (lightEngine.getQueueSize() < 8192) {
      lightEngine.removeIf((pos) => SectionPos.blockToSection(pos) === section);
      return;
    }

    const minX = SectionPos.sectionToBlockCoord(SectionPos.x(section));
    const minY = SectionPos.sectionToBlockCoord(SectionPos.y(section));
    const minZ = SectionPos.sectionToBlockCoord(SectionPos.z(section));
    for (let x = 0; x < 16; x++) {
      for (let y = 0; y < 16; y++) {
        for (let z = 0; z < 16; z++) {
          lightEngine.removeFromQueue(BlockPos.asLong(minX + x, minY + y, minZ + z));
        }
      }
    }
  }

  public hasInconsistencies(): boolean {
    return this.hasToRemove;
  }

  public markNewInconsistencies(
    lightEngine: LightEngineStorageAccess,
    _updateSkyLight: boolean,
    skipEdgeLightPropagation: boolean,
  ): void {
    if (!this.hasInconsistencies() && this.queuedSections.size === 0) {
      return;
    }

    for (const section of this.toRemove) {
      this.clearQueuedSectionBlocks(lightEngine, section);
      const queuedLayer = this.queuedSections.get(section);
      this.queuedSections.delete(section);
      const removedLayer = this.updatingSectionData.removeLayer(section);
      if (this.columnsToRetainQueuedDataFor.has(SectionPos.getZeroNode(section))) {
        if (queuedLayer !== undefined) {
          this.queuedSections.set(section, queuedLayer);
        } else if (removedLayer !== undefined) {
          this.queuedSections.set(section, removedLayer);
        }
      }
    }

    this.updatingSectionData.clearCache();
    for (const section of this.toRemove) {
      this.onNodeRemoved(section);
    }

    this.toRemove.clear();
    this.hasToRemove = false;

    for (const [section, layer] of this.queuedSections) {
      if (this.storingLightForSection(section) && this.updatingSectionData.getLayer(section) !== layer) {
        this.clearQueuedSectionBlocks(lightEngine, section);
        this.updatingSectionData.setLayer(section, layer);
        this.changedSections.add(section);
      }
    }

    this.updatingSectionData.clearCache();
    const sectionsToCheck = skipEdgeLightPropagation ? this.untrustedSections : new Set(this.queuedSections.keys());
    for (const section of sectionsToCheck) {
      this.checkEdgesForSection(lightEngine, section);
    }

    this.untrustedSections.clear();
    for (const section of [...this.queuedSections.keys()]) {
      if (this.storingLightForSection(section)) {
        this.queuedSections.delete(section);
      }
    }
  }

  private checkEdgesForSection(lightEngine: LightEngineStorageAccess, section: bigint): void {
    if (!this.storingLightForSection(section)) {
      return;
    }

    const minX = SectionPos.sectionToBlockCoord(SectionPos.x(section));
    const minY = SectionPos.sectionToBlockCoord(SectionPos.y(section));
    const minZ = SectionPos.sectionToBlockCoord(SectionPos.z(section));

    for (const direction of DIRECTIONS) {
      const neighborSection = SectionPos.offset(section, direction);
      if (this.queuedSections.has(neighborSection) || !this.storingLightForSection(neighborSection)) {
        continue;
      }

      for (let first = 0; first < 16; first++) {
        for (let second = 0; second < 16; second++) {
          let source: bigint;
          let target: bigint;
          switch (direction) {
            case Direction.DOWN:
              source = BlockPos.asLong(minX + second, minY, minZ + first);
              target = BlockPos.asLong(minX + second, minY - 1, minZ + first);
              break;
            case Direction.UP:
              source = BlockPos.asLong(minX + second, minY + 15, minZ + first);
              target = BlockPos.asLong(minX + second, minY + 16, minZ + first);
              break;
            case Direction.NORTH:
              source = BlockPos.asLong(minX + first, minY + second, minZ);
              target = BlockPos.asLong(minX + first, minY + second, minZ - 1);
              break;
            case Direction.SOUTH:
              source = BlockPos.asLong(minX + first, minY + second, minZ + 15);
              target = BlockPos.asLong(minX + first, minY + second, minZ + 16);
              break;
            case Direction.WEST:
              source = BlockPos.asLong(minX, minY + first, minZ + second);
              target = BlockPos.asLong(minX - 1, minY + first, minZ + second);
              break;
            default:
              source = BlockPos.asLong(minX + 15, minY + first, minZ + second);
              target = BlockPos.asLong(minX + 16, minY + first, minZ + second);
              break;
          }

          lightEngine.checkEdgeForStorage(source, target, lightEngine.computeLevelFromNeighborForStorage(source, target, lightEngine.getLevelForStorage(source)), false);
          lightEngine.checkEdgeForStorage(target, source, lightEngine.computeLevelFromNeighborForStorage(target, source, lightEngine.getLevelForStorage(target)), false);
        }
      }
    }
  }

  protected onNodeAdded(_section: bigint): void {}

  protected onNodeRemoved(_section: bigint): void {}

  public enableLightSources(_chunkColumn: bigint, _enabled: boolean): void {}

  public retainData(chunkColumn: bigint, retain: boolean): void {
    if (retain) {
      this.columnsToRetainQueuedDataFor.add(chunkColumn);
    } else {
      this.columnsToRetainQueuedDataFor.delete(chunkColumn);
    }
  }

  public queueSectionData(section: bigint, data: DataLayer | undefined, trusted: boolean): void {
    if (data !== undefined) {
      this.queuedSections.set(section, data);
      if (!trusted) {
        this.untrustedSections.add(section);
      }
    } else {
      this.queuedSections.delete(section);
    }
  }

  public updateSectionStatus(section: bigint, empty: boolean): void {
    const hasData = this.dataSectionSet.has(section);
    if (!hasData && !empty) {
      this.toMarkData.add(section);
      this.checkEdge(LIGHT_SELF_SOURCE, section, LayerLightSectionStorage.LIGHT_AND_DATA, true);
    }

    if (hasData && empty) {
      this.toMarkNoData.add(section);
      this.checkEdge(LIGHT_SELF_SOURCE, section, LayerLightSectionStorage.EMPTY, false);
    }
  }

  public runAllUpdates(): void {
    if (this.hasWork()) {
      this.runUpdates(MAX_UPDATE_BUDGET);
    }
  }

  public swapSectionMap(): void {
    if (this.changedSections.size > 0) {
      this.visibleSectionData = this.updatingSectionData.copy();
      this.visibleSectionData.disableCache();
      this.changedSections.clear();
    }

    if (this.sectionsAffectedByLightUpdates.size > 0) {
      for (const section of this.sectionsAffectedByLightUpdates) {
        this.chunkSource.onLightUpdate?.(this.layer, SectionPos.ofKey(section));
      }

      this.sectionsAffectedByLightUpdates.clear();
    }
  }

  public hasStorageWork(): boolean {
    return this.hasWork();
  }
}
