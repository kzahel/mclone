import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { CarvingMaskDecoratorConfiguration } from "../feature/configurations/carving-mask-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class CarvingMaskDecorator extends FeatureDecorator<CarvingMaskDecoratorConfiguration> {
  public override getPositions(
    context: DecorationContext,
    _random: SimpleRandomSource,
    config: CarvingMaskDecoratorConfiguration,
    pos: BlockPos,
  ): Iterable<BlockPos> {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    const carvingMask = context.getCarvingMask(config.step, chunkX, chunkZ);
    if (carvingMask === undefined) {
      return [];
    }

    return this.generatePositions(chunkX, chunkZ, context.getMinBuildHeight(), carvingMask);
  }

  private *generatePositions(
    chunkX: number,
    chunkZ: number,
    minBuildHeight: number,
    carvingMask: Uint8Array,
  ): IterableIterator<BlockPos> {
    const minBlockX = SectionPos.sectionToBlockCoord(chunkX);
    const minBlockZ = SectionPos.sectionToBlockCoord(chunkZ);
    for (let index = 0; index < carvingMask.length; index++) {
      if (carvingMask[index] === 0) {
        continue;
      }

      const localX = index & 15;
      const localZ = (index >> 4) & 15;
      const y = minBuildHeight + (index >> 8);
      yield new BlockPos(minBlockX + localX, y, minBlockZ + localZ);
    }
  }
}
