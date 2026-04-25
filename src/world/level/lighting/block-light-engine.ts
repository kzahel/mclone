import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { SectionPos } from "../../../core/section-pos";
import type { BlockGetter } from "../block-getter";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import { BlockDataLayerStorageMap, BlockLightSectionStorage } from "./block-light-section-storage";
import { LayerLightEngine } from "./layer-light-engine";
import { LIGHT_SELF_SOURCE } from "./section-tracker";

const DIRECTIONS = Direction.values();

function signum(value: number): number {
  return value === 0 ? 0 : (value > 0 ? 1 : -1);
}

export class BlockLightEngine extends LayerLightEngine<BlockDataLayerStorageMap, BlockLightSectionStorage> {
  private readonly emissionPosScratch = new BlockPos.MutableBlockPos();

  public constructor(chunkSource: LightChunkGetter) {
    super(chunkSource, LightLayer.BLOCK, new BlockLightSectionStorage(chunkSource));
  }

  private getLightEmission(pos: bigint): number {
    const x = BlockPos.getX(pos);
    const y = BlockPos.getY(pos);
    const z = BlockPos.getZ(pos);
    const chunk: BlockGetter | null = this.chunkSource.getChunkForLighting(SectionPos.blockToSectionCoord(x), SectionPos.blockToSectionCoord(z));
    return chunk?.getBlockState(this.emissionPosScratch.set(x, y, z)).getLightEmission() ?? 0;
  }

  protected override computeLevelFromNeighbor(source: bigint, target: bigint, sourceLevel: number): number {
    if (target === LIGHT_SELF_SOURCE) {
      return 15;
    }
    if (source === LIGHT_SELF_SOURCE) {
      return sourceLevel + 15 - this.getLightEmission(target);
    }
    if (sourceLevel >= 15) {
      return sourceLevel;
    }

    const dx = signum(BlockPos.getX(target) - BlockPos.getX(source));
    const dy = signum(BlockPos.getY(target) - BlockPos.getY(source));
    const dz = signum(BlockPos.getZ(target) - BlockPos.getZ(source));
    const direction = Direction.fromNormal(dx, dy, dz);
    if (direction === undefined) {
      return 15;
    }

    const opacity = { value: 0 };
    const targetState = this.getStateAndOpacity(target, opacity);
    if (opacity.value >= 15) {
      return 15;
    }

    const sourceState = this.getStateAndOpacity(source, undefined);
    return this.shapesFaceOcclude(sourceState, targetState, direction)
      ? 15
      : sourceLevel + Math.max(1, opacity.value);
  }

  protected override checkNeighborsAfterUpdate(pos: bigint, level: number, decrease: boolean): void {
    const section = SectionPos.blockToSection(pos);
    for (const direction of DIRECTIONS) {
      const neighbor = BlockPos.offset(pos, direction);
      const neighborSection = SectionPos.blockToSection(neighbor);
      if (section === neighborSection || this.storage.storingLightForSection(neighborSection)) {
        this.checkNeighbor(pos, neighbor, level, decrease);
      }
    }
  }

  protected override getComputedLevel(pos: bigint, source: bigint, candidateLevel: number): number {
    let level = candidateLevel;
    if (source !== LIGHT_SELF_SOURCE) {
      const sourceLevel = this.computeLevelFromNeighbor(LIGHT_SELF_SOURCE, pos, 0);
      if (candidateLevel > sourceLevel) {
        level = sourceLevel;
      }

      if (level === 0) {
        return level;
      }
    }

    const section = SectionPos.blockToSection(pos);
    const dataLayer = this.storage.getDataLayer(section, true);

    for (const direction of DIRECTIONS) {
      const neighbor = BlockPos.offset(pos, direction);
      if (neighbor !== source) {
        const neighborSection = SectionPos.blockToSection(neighbor);
        const neighborDataLayer: DataLayer | undefined = section === neighborSection
          ? dataLayer
          : this.storage.getDataLayer(neighborSection, true);

        if (neighborDataLayer !== undefined) {
          const computedLevel = this.computeLevelFromNeighbor(neighbor, pos, this.getLevelFromDataLayer(neighborDataLayer, neighbor));
          if (level > computedLevel) {
            level = computedLevel;
          }

          if (level === 0) {
            return level;
          }
        }
      }
    }

    return level;
  }

  public override onBlockEmissionIncrease(pos: BlockPos, lightEmission: number): void {
    this.storage.runAllUpdates();
    this.checkEdge(LIGHT_SELF_SOURCE, pos.asLong(), 15 - lightEmission, true);
  }
}
