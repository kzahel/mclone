import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { SectionPos } from "../../../core/section-pos";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import { DataLayerStorageMap } from "./data-layer-storage-map";
import { LayerLightSectionStorage } from "./layer-light-section-storage";
import type { LightEngineStorageAccess } from "./layer-light-section-storage";

const HORIZONTALS = [Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST] as const;

export class SkyDataLayerStorageMap extends DataLayerStorageMap<SkyDataLayerStorageMap> {
  public constructor(
    map = new Map<bigint, DataLayer>(),
    private readonly topSections = new Map<bigint, number>(),
    public currentLowestY = Number.MAX_SAFE_INTEGER,
  ) {
    super(map);
  }

  public getTopSection(column: bigint): number {
    return this.topSections.get(column) ?? this.currentLowestY;
  }

  public setTopSection(column: bigint, sectionY: number): void {
    this.topSections.set(column, sectionY);
  }

  public removeTopSection(column: bigint): void {
    this.topSections.delete(column);
  }

  public copy(): SkyDataLayerStorageMap {
    return new SkyDataLayerStorageMap(this.cloneLayerMap(), new Map(this.topSections), this.currentLowestY);
  }
}

export class SkyLightSectionStorage extends LayerLightSectionStorage<SkyDataLayerStorageMap> {
  private readonly sectionsWithSources = new Set<bigint>();
  private readonly sectionsToAddSourcesTo = new Set<bigint>();
  private readonly sectionsToRemoveSourcesFrom = new Set<bigint>();
  private readonly columnsWithSkySources = new Set<bigint>();
  private hasSourceInconsistencies = false;

  public constructor(chunkSource: LightChunkGetter) {
    super(LightLayer.SKY, chunkSource, new SkyDataLayerStorageMap());
  }

  public override getLightValue(pos: bigint): number {
    return this.getLightValueFrom(pos, false);
  }

  public getLightValueFrom(pos: bigint, updating: boolean): number {
    let section = SectionPos.blockToSection(pos);
    let sectionY = SectionPos.y(section);
    const storageMap = updating ? this.updatingSectionData : this.visibleSectionData;
    const topSection = storageMap.getTopSection(SectionPos.getZeroNode(section));
    if (topSection !== storageMap.currentLowestY && sectionY < topSection) {
      let dataLayer = this.getDataLayerFromMap(storageMap, section);
      if (dataLayer === undefined) {
        pos = BlockPos.getFlatIndex(pos);
        while (dataLayer === undefined) {
          sectionY++;
          if (sectionY >= topSection) {
            return 15;
          }

          pos = BlockPos.offset(pos, 0, 16, 0);
          section = SectionPos.offset(section, Direction.UP);
          dataLayer = this.getDataLayerFromMap(storageMap, section);
        }
      }

      return dataLayer.get(
        SectionPos.sectionRelative(BlockPos.getX(pos)),
        SectionPos.sectionRelative(BlockPos.getY(pos)),
        SectionPos.sectionRelative(BlockPos.getZ(pos)),
      );
    }

    return updating && !this.lightOnInSection(section) ? 0 : 15;
  }

  protected override onNodeAdded(section: bigint): void {
    const sectionY = SectionPos.y(section);
    if (this.updatingSectionData.currentLowestY > sectionY) {
      this.updatingSectionData.currentLowestY = sectionY;
    }

    const column = SectionPos.getZeroNode(section);
    const topSection = this.updatingSectionData.getTopSection(column);
    if (topSection < sectionY + 1) {
      this.updatingSectionData.setTopSection(column, sectionY + 1);
      if (this.columnsWithSkySources.has(column)) {
        this.queueAddSource(section);
        if (topSection > this.updatingSectionData.currentLowestY) {
          this.queueRemoveSource(SectionPos.asLong(SectionPos.x(section), topSection - 1, SectionPos.z(section)));
        }

        this.recheckInconsistencyFlag();
      }
    }
  }

  private queueRemoveSource(section: bigint): void {
    this.sectionsToRemoveSourcesFrom.add(section);
    this.sectionsToAddSourcesTo.delete(section);
  }

  private queueAddSource(section: bigint): void {
    this.sectionsToAddSourcesTo.add(section);
    this.sectionsToRemoveSourcesFrom.delete(section);
  }

  private recheckInconsistencyFlag(): void {
    this.hasSourceInconsistencies = this.sectionsToAddSourcesTo.size > 0 || this.sectionsToRemoveSourcesFrom.size > 0;
  }

  protected override onNodeRemoved(section: bigint): void {
    const column = SectionPos.getZeroNode(section);
    const hadSkySources = this.columnsWithSkySources.has(column);
    if (hadSkySources) {
      this.queueRemoveSource(section);
    }

    let sectionY = SectionPos.y(section);
    if (this.updatingSectionData.getTopSection(column) === sectionY + 1) {
      let cursor = section;
      while (!this.storingLightForSection(cursor) && this.hasSectionsBelow(sectionY)) {
        cursor = SectionPos.offset(cursor, Direction.DOWN);
        sectionY--;
      }

      if (this.storingLightForSection(cursor)) {
        this.updatingSectionData.setTopSection(column, sectionY + 1);
        if (hadSkySources) {
          this.queueAddSource(cursor);
        }
      } else {
        this.updatingSectionData.removeTopSection(column);
      }
    }

    if (hadSkySources) {
      this.recheckInconsistencyFlag();
    }
  }

  public override enableLightSources(column: bigint, enabled: boolean): void {
    this.runAllUpdates();
    if (enabled && !this.columnsWithSkySources.has(column)) {
      this.columnsWithSkySources.add(column);
      const topSection = this.updatingSectionData.getTopSection(column);
      if (topSection !== this.updatingSectionData.currentLowestY) {
        this.queueAddSource(SectionPos.asLong(SectionPos.x(column), topSection - 1, SectionPos.z(column)));
        this.recheckInconsistencyFlag();
      }
    } else if (!enabled) {
      this.columnsWithSkySources.delete(column);
    }
  }

  public override hasInconsistencies(): boolean {
    return super.hasInconsistencies() || this.hasSourceInconsistencies;
  }

  protected override createDataLayer(section: bigint): DataLayer {
    const queuedLayer = this.queuedSections.get(section);
    if (queuedLayer !== undefined) {
      return queuedLayer;
    }

    let sectionAbove = SectionPos.offset(section, Direction.UP);
    const topSection = this.updatingSectionData.getTopSection(SectionPos.getZeroNode(section));
    if (topSection !== this.updatingSectionData.currentLowestY && SectionPos.y(sectionAbove) < topSection) {
      let layerAbove = this.getDataLayer(sectionAbove, true);
      while (layerAbove === undefined) {
        sectionAbove = SectionPos.offset(sectionAbove, Direction.UP);
        layerAbove = this.getDataLayer(sectionAbove, true);
      }

      return SkyLightSectionStorage.repeatFirstLayer(layerAbove);
    }

    return new DataLayer();
  }

  private static repeatFirstLayer(layer: DataLayer): DataLayer {
    if (layer.isEmpty()) {
      return new DataLayer();
    }

    const source = layer.getData();
    const repeated = new Uint8Array(DataLayer.SIZE);
    for (let y = 0; y < 16; y++) {
      repeated.set(source.subarray(0, DataLayer.LAYER_SIZE), y * DataLayer.LAYER_SIZE);
    }
    return new DataLayer(repeated);
  }

  public override markNewInconsistencies(
    lightEngine: LightEngineStorageAccess,
    updateSkyLight: boolean,
    skipEdgeLightPropagation: boolean,
  ): void {
    super.markNewInconsistencies(lightEngine, updateSkyLight, skipEdgeLightPropagation);
    if (!updateSkyLight) {
      return;
    }

    if (this.sectionsToAddSourcesTo.size > 0) {
      for (const section of this.sectionsToAddSourcesTo) {
        const level = this.getLevel(section);
        if (level !== LayerLightSectionStorage.EMPTY && !this.sectionsToRemoveSourcesFrom.has(section) && !this.sectionsWithSources.has(section)) {
          this.sectionsWithSources.add(section);
          if (level === LayerLightSectionStorage.LIGHT_ONLY) {
            this.clearQueuedSectionBlocks(lightEngine, section);
            if (!this.changedSections.has(section)) {
              this.changedSections.add(section);
              this.updatingSectionData.copyDataLayer(section);
            }

            this.getDataLayer(section, true)!.getData().fill(0xFF);
            const minX = SectionPos.sectionToBlockCoord(SectionPos.x(section));
            const minY = SectionPos.sectionToBlockCoord(SectionPos.y(section));
            const minZ = SectionPos.sectionToBlockCoord(SectionPos.z(section));

            for (const direction of HORIZONTALS) {
              const neighborSection = SectionPos.offset(section, direction);
              if (
                (this.sectionsToRemoveSourcesFrom.has(neighborSection) ||
                  (!this.sectionsWithSources.has(neighborSection) && !this.sectionsToAddSourcesTo.has(neighborSection))) &&
                this.storingLightForSection(neighborSection)
              ) {
                for (let first = 0; first < 16; first++) {
                  for (let second = 0; second < 16; second++) {
                    let source: bigint;
                    let target: bigint;
                    switch (direction) {
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

                    lightEngine.checkEdgeForStorage(source, target, lightEngine.computeLevelFromNeighborForStorage(source, target, 0), true);
                  }
                }
              }
            }

            for (let x = 0; x < 16; x++) {
              for (let z = 0; z < 16; z++) {
                const source = BlockPos.asLong(
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.x(section), x),
                  SectionPos.sectionToBlockCoord(SectionPos.y(section)),
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.z(section), z),
                );
                const target = BlockPos.asLong(
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.x(section), x),
                  SectionPos.sectionToBlockCoord(SectionPos.y(section)) - 1,
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.z(section), z),
                );
                lightEngine.checkEdgeForStorage(source, target, lightEngine.computeLevelFromNeighborForStorage(source, target, 0), true);
              }
            }
          } else {
            for (let x = 0; x < 16; x++) {
              for (let z = 0; z < 16; z++) {
                const pos = BlockPos.asLong(
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.x(section), x),
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.y(section), 15),
                  SectionPos.sectionToBlockCoordWithOffset(SectionPos.z(section), z),
                );
                lightEngine.checkEdgeForStorage(lightEngine.selfSource, pos, 0, true);
              }
            }
          }
        }
      }
    }

    this.sectionsToAddSourcesTo.clear();
    if (this.sectionsToRemoveSourcesFrom.size > 0) {
      for (const section of this.sectionsToRemoveSourcesFrom) {
        if (this.sectionsWithSources.delete(section) && this.storingLightForSection(section)) {
          for (let x = 0; x < 16; x++) {
            for (let z = 0; z < 16; z++) {
              const pos = BlockPos.asLong(
                SectionPos.sectionToBlockCoordWithOffset(SectionPos.x(section), x),
                SectionPos.sectionToBlockCoordWithOffset(SectionPos.y(section), 15),
                SectionPos.sectionToBlockCoordWithOffset(SectionPos.z(section), z),
              );
              lightEngine.checkEdgeForStorage(lightEngine.selfSource, pos, 15, false);
            }
          }
        }
      }
    }

    this.sectionsToRemoveSourcesFrom.clear();
    this.hasSourceInconsistencies = false;
  }

  public hasSectionsBelow(sectionY: number): boolean {
    return sectionY >= this.updatingSectionData.currentLowestY;
  }

  public isAboveData(section: bigint): boolean {
    const column = SectionPos.getZeroNode(section);
    const topSection = this.updatingSectionData.getTopSection(column);
    return topSection === this.updatingSectionData.currentLowestY || SectionPos.y(section) >= topSection;
  }

  protected lightOnInSection(section: bigint): boolean {
    return this.columnsWithSkySources.has(SectionPos.getZeroNode(section));
  }
}
