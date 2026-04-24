import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import {
  getLightSectionCount,
  getMaxLightSection,
  getMinLightSection,
  type LevelHeightAccessorLike,
} from "../light-section";
import { BlockLightEngine } from "./block-light-engine";
import { SkyLightEngine } from "./sky-light-engine";
import {
  DummyLightLayerEventListener,
  type ChunkPosLike,
  type LayerLightEventListener,
  type LightEventListener,
} from "./layer-light-event-listener";

export class LevelLightEngine implements LightEventListener {
  public static readonly MAX_SOURCE_LEVEL = 15;
  public static readonly LIGHT_SECTION_PADDING = 1;
  protected readonly levelHeightAccessor: LevelHeightAccessorLike;
  private readonly blockEngine: BlockLightEngine | undefined;
  private readonly skyEngine: SkyLightEngine | undefined;

  public constructor(chunkSource: LightChunkGetter, blockLight: boolean, skyLight: boolean) {
    this.levelHeightAccessor = chunkSource.getLevel();
    this.blockEngine = blockLight ? new BlockLightEngine(chunkSource) : undefined;
    this.skyEngine = skyLight ? new SkyLightEngine(chunkSource) : undefined;
  }

  public checkBlock(pos: BlockPos): void {
    this.blockEngine?.checkBlock(pos);
    this.skyEngine?.checkBlock(pos);
  }

  public onBlockEmissionIncrease(pos: BlockPos, lightEmission: number): void {
    this.blockEngine?.onBlockEmissionIncrease(pos, lightEmission);
  }

  public hasLightWork(): boolean {
    return this.skyEngine?.hasLightWork() === true ? true : this.blockEngine?.hasLightWork() === true;
  }

  public runUpdates(budget: number, updateSkyLight: boolean, skipEdgeLightPropagation: boolean): number {
    if (this.blockEngine !== undefined && this.skyEngine !== undefined) {
      const blockBudget = Math.trunc(budget / 2);
      const blockRemaining = this.blockEngine.runUpdates(blockBudget, updateSkyLight, skipEdgeLightPropagation);
      const skyBudget = budget - blockBudget + blockRemaining;
      const skyRemaining = this.skyEngine.runUpdates(skyBudget, updateSkyLight, skipEdgeLightPropagation);
      return blockRemaining === 0 && skyRemaining > 0
        ? this.blockEngine.runUpdates(skyRemaining, updateSkyLight, skipEdgeLightPropagation)
        : skyRemaining;
    }

    if (this.blockEngine !== undefined) {
      return this.blockEngine.runUpdates(budget, updateSkyLight, skipEdgeLightPropagation);
    }

    return this.skyEngine?.runUpdates(budget, updateSkyLight, skipEdgeLightPropagation) ?? budget;
  }

  public updateSectionStatus(section: SectionPos, empty: boolean): void {
    this.blockEngine?.updateSectionStatus(section, empty);
    this.skyEngine?.updateSectionStatus(section, empty);
  }

  public enableLightSources(chunk: ChunkPosLike, enabled: boolean): void {
    this.blockEngine?.enableLightSources(chunk, enabled);
    this.skyEngine?.enableLightSources(chunk, enabled);
  }

  public getLayerListener(layer: LightLayer): LayerLightEventListener {
    if (layer === LightLayer.BLOCK) {
      return this.blockEngine ?? DummyLightLayerEventListener.INSTANCE;
    }

    return this.skyEngine ?? DummyLightLayerEventListener.INSTANCE;
  }

  public getDebugData(layer: LightLayer, section: SectionPos): string {
    if (layer === LightLayer.BLOCK) {
      return this.blockEngine?.getDebugData(section.asLong()) ?? "n/a";
    }

    return this.skyEngine?.getDebugData(section.asLong()) ?? "n/a";
  }

  public queueSectionData(layer: LightLayer, section: SectionPos, data: DataLayer | undefined, trusted: boolean): void {
    if (layer === LightLayer.BLOCK) {
      this.blockEngine?.queueSectionData(section.asLong(), data, trusted);
    } else {
      this.skyEngine?.queueSectionData(section.asLong(), data, trusted);
    }
  }

  public retainData(chunk: ChunkPosLike, retain: boolean): void {
    this.blockEngine?.retainData(chunk, retain);
    this.skyEngine?.retainData(chunk, retain);
  }

  public getRawBrightness(pos: BlockPos, skyDarken: number): number {
    const sky = this.skyEngine === undefined ? 0 : this.skyEngine.getLightValue(pos) - skyDarken;
    const block = this.blockEngine === undefined ? 0 : this.blockEngine.getLightValue(pos);
    return Math.max(block, sky);
  }

  public getLightSectionCount(): number {
    return getLightSectionCount(this.levelHeightAccessor);
  }

  public getMinLightSection(): number {
    return getMinLightSection(this.levelHeightAccessor);
  }

  public getMaxLightSection(): number {
    return getMaxLightSection(this.levelHeightAccessor);
  }
}
