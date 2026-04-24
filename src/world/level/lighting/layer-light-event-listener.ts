import type { BlockPos } from "../../../core/block-pos";
import type { SectionPos } from "../../../core/section-pos";
import type { DataLayer } from "../chunk/data-layer";

export interface ChunkPosLike {
  readonly x: number;
  readonly z: number;
}

export interface LightEventListener {
  checkBlock(pos: BlockPos): void;

  onBlockEmissionIncrease(pos: BlockPos, lightEmission: number): void;

  hasLightWork(): boolean;

  runUpdates(budget: number, updateSkyLight: boolean, skipEdgeLightPropagation: boolean): number;

  updateSectionStatus(section: SectionPos, empty: boolean): void;

  enableLightSources(chunk: ChunkPosLike, enabled: boolean): void;
}

export interface LayerLightEventListener extends LightEventListener {
  getDataLayerData(section: SectionPos): DataLayer | undefined;

  getLightValue(pos: BlockPos): number;
}

export class DummyLightLayerEventListener implements LayerLightEventListener {
  public static readonly INSTANCE = new DummyLightLayerEventListener();

  private constructor() {}

  public getDataLayerData(_section: SectionPos): DataLayer | undefined {
    return undefined;
  }

  public getLightValue(_pos: BlockPos): number {
    return 0;
  }

  public checkBlock(_pos: BlockPos): void {}

  public onBlockEmissionIncrease(_pos: BlockPos, _lightEmission: number): void {}

  public hasLightWork(): boolean {
    return false;
  }

  public runUpdates(budget: number, _updateSkyLight: boolean, _skipEdgeLightPropagation: boolean): number {
    return budget;
  }

  public updateSectionStatus(_section: SectionPos, _empty: boolean): void {}

  public enableLightSources(_chunk: ChunkPosLike, _enabled: boolean): void {}
}
