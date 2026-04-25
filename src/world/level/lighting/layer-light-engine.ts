import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { SectionPos } from "../../../core/section-pos";
import type { BlockGetter } from "../block-getter";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import type { BlockState } from "../block/state/block-state";
import { DynamicGraphMinFixedPoint } from "./dynamic-graph-min-fixed-point";
import { DataLayerStorageMap } from "./data-layer-storage-map";
import { LIGHT_SELF_SOURCE } from "./section-tracker";
import { LayerLightSectionStorage, type LightEngineStorageAccess } from "./layer-light-section-storage";
import type { LayerLightEventListener, ChunkPosLike } from "./layer-light-event-listener";

const DIRECTIONS = Direction.values();

interface MutableInt {
  value: number;
}

export abstract class LayerLightEngine<M extends DataLayerStorageMap<M>, S extends LayerLightSectionStorage<M>>
  extends DynamicGraphMinFixedPoint
  implements LayerLightEventListener, LightEngineStorageAccess {
  public readonly selfSource = LIGHT_SELF_SOURCE;
  private runningLightUpdates = false;
  private readonly lastChunkX: number[] = [Number.NaN, Number.NaN];
  private readonly lastChunkZ: number[] = [Number.NaN, Number.NaN];
  private readonly lastChunk: Array<BlockGetter | null> = [null, null];
  private readonly blockPosScratch = new BlockPos.MutableBlockPos();

  protected constructor(
    protected readonly chunkSource: LightChunkGetter,
    protected readonly layer: LightLayer,
    protected readonly storage: S,
  ) {
    super(16);
    this.clearCache();
  }

  protected override checkNode(pos: bigint): void {
    this.storage.runAllUpdates();
    if (this.storage.storingLightForSection(SectionPos.blockToSection(pos))) {
      super.checkNode(pos);
    }
  }

  private getChunk(chunkX: number, chunkZ: number): BlockGetter | null {
    for (let index = 0; index < 2; index++) {
      if (chunkX === this.lastChunkX[index] && chunkZ === this.lastChunkZ[index]) {
        return this.lastChunk[index] ?? null;
      }
    }

    const chunk = this.chunkSource.getChunkForLighting(chunkX, chunkZ);
    this.lastChunkX[1] = this.lastChunkX[0]!;
    this.lastChunkZ[1] = this.lastChunkZ[0]!;
    this.lastChunk[1] = this.lastChunk[0] ?? null;
    this.lastChunkX[0] = chunkX;
    this.lastChunkZ[0] = chunkZ;
    this.lastChunk[0] = chunk;
    return chunk;
  }

  private clearCache(): void {
    this.lastChunkX[0] = Number.NaN;
    this.lastChunkZ[0] = Number.NaN;
    this.lastChunkX[1] = Number.NaN;
    this.lastChunkZ[1] = Number.NaN;
    this.lastChunk[0] = null;
    this.lastChunk[1] = null;
  }

  protected getStateAndOpacity(pos: bigint, opacity: MutableInt | undefined): BlockState | undefined {
    if (pos === LIGHT_SELF_SOURCE) {
      if (opacity !== undefined) {
        opacity.value = 0;
      }

      return undefined;
    }

    const chunkX = SectionPos.blockToSectionCoord(BlockPos.getX(pos));
    const chunkZ = SectionPos.blockToSectionCoord(BlockPos.getZ(pos));
    const chunk = this.getChunk(chunkX, chunkZ);
    if (chunk === null) {
      if (opacity !== undefined) {
        opacity.value = 16;
      }

      return undefined;
    }

    const blockPos = this.blockPosScratch.set(
      BlockPos.getX(pos),
      BlockPos.getY(pos),
      BlockPos.getZ(pos),
    );
    const state = chunk.getBlockState(blockPos);
    if (opacity !== undefined) {
      opacity.value = state.getLightBlock(this.chunkSource.getLevel(), blockPos);
    }

    return state.canOcclude() && state.useShapeForLightOcclusion() ? state : undefined;
  }

  protected shapesFaceOcclude(fromState: BlockState | undefined, toState: BlockState | undefined, _direction: Direction): boolean {
    // Current block states do not expose voxel face shapes yet; full opacity is still enforced by getLightBlock.
    return fromState !== undefined && toState !== undefined && fromState.canOcclude() && toState.canOcclude();
  }

  public static getLightBlockInto(
    _level: BlockGetter,
    fromState: BlockState,
    _fromPos: BlockPos,
    toState: BlockState,
    _toPos: BlockPos,
    _direction: Direction,
    opacity: number,
  ): number {
    const fromUsesShape = fromState.canOcclude() && fromState.useShapeForLightOcclusion();
    const toUsesShape = toState.canOcclude() && toState.useShapeForLightOcclusion();
    return fromUsesShape && toUsesShape ? 16 : opacity;
  }

  protected override isSource(pos: bigint): boolean {
    return pos === LIGHT_SELF_SOURCE;
  }

  protected override getComputedLevel(_pos: bigint, _source: bigint, _candidateLevel: number): number {
    return 0;
  }

  protected override getLevel(pos: bigint): number {
    return pos === LIGHT_SELF_SOURCE ? 0 : 15 - this.storage.getStoredLevel(pos);
  }

  protected getLevelFromDataLayer(layer: DataLayer, pos: bigint): number {
    return 15 - layer.get(
      SectionPos.sectionRelative(BlockPos.getX(pos)),
      SectionPos.sectionRelative(BlockPos.getY(pos)),
      SectionPos.sectionRelative(BlockPos.getZ(pos)),
    );
  }

  protected override setLevel(pos: bigint, level: number): void {
    this.storage.setStoredLevel(pos, Math.min(15, 15 - level));
  }

  protected override computeLevelFromNeighbor(_source: bigint, _target: bigint, _sourceLevel: number): number {
    return 0;
  }

  public hasLightWork(): boolean {
    return this.hasWork() || this.storage.hasStorageWork() || this.storage.hasInconsistencies();
  }

  public runUpdates(budget: number, updateSkyLight: boolean, skipEdgeLightPropagation: boolean): number {
    if (!this.runningLightUpdates) {
      if (this.storage.hasStorageWork()) {
        budget = this.storage.runUpdates(budget);
        if (budget === 0) {
          return budget;
        }
      }

      this.storage.markNewInconsistencies(this, updateSkyLight, skipEdgeLightPropagation);
    }

    this.runningLightUpdates = true;
    if (this.hasWork()) {
      budget = this.runUpdatesForGraph(budget);
      this.clearCache();
      if (budget === 0) {
        return budget;
      }
    }

    this.runningLightUpdates = false;
    this.storage.swapSectionMap();
    return budget;
  }

  public queueSectionData(section: bigint, data: DataLayer | undefined, trusted: boolean): void {
    this.storage.queueSectionData(section, data, trusted);
  }

  public getDataLayerData(section: SectionPos): DataLayer | undefined {
    return this.storage.getDataLayerData(section.asLong());
  }

  public getLightValue(pos: BlockPos): number {
    return this.storage.getLightValue(pos.asLong());
  }

  public getDebugData(section: bigint): string {
    return `${this.storage.getLevelForDebug(section)}`;
  }

  public checkBlock(pos: BlockPos): void {
    const packed = pos.asLong();
    this.checkNode(packed);

    for (const direction of DIRECTIONS) {
      this.checkNode(BlockPos.offset(packed, direction));
    }
  }

  public onBlockEmissionIncrease(_pos: BlockPos, _lightEmission: number): void {}

  public updateSectionStatus(section: SectionPos, empty: boolean): void {
    this.storage.updateSectionStatus(section.asLong(), empty);
  }

  public enableLightSources(chunk: ChunkPosLike, enabled: boolean): void {
    const zeroNode = SectionPos.getZeroNode(SectionPos.asLong(chunk.x, 0, chunk.z));
    this.storage.enableLightSources(zeroNode, enabled);
  }

  public retainData(chunk: ChunkPosLike, retain: boolean): void {
    const zeroNode = SectionPos.getZeroNode(SectionPos.asLong(chunk.x, 0, chunk.z));
    this.storage.retainData(zeroNode, retain);
  }

  public checkEdgeForStorage(source: bigint, target: bigint, candidateLevel: number, decrease: boolean): void {
    this.checkEdge(source, target, candidateLevel, decrease);
  }

  public computeLevelFromNeighborForStorage(source: bigint, target: bigint, sourceLevel: number): number {
    return this.computeLevelFromNeighbor(source, target, sourceLevel);
  }

  public getLevelForStorage(pos: bigint): number {
    return this.getLevel(pos);
  }
}
