import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { HeightmapConfiguration } from "../feature/configurations/heightmap-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class HeightmapSpreadDoubleDecorator extends FeatureDecorator<HeightmapConfiguration> {
  public override getPositions(
    context: DecorationContext,
    random: SimpleRandomSource,
    config: HeightmapConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const y = context.getHeight(config.heightmap, pos.getX(), pos.getZ());
    if (y === context.getMinBuildHeight()) {
      return [];
    }

    return [new BlockPos(pos.getX(), context.getMinBuildHeight() + random.nextInt((y - context.getMinBuildHeight()) * 2), pos.getZ())];
  }
}
