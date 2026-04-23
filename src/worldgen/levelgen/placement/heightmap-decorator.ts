import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { HeightmapConfiguration } from "../feature/configurations/heightmap-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class HeightmapDecorator extends FeatureDecorator<HeightmapConfiguration> {
  public override getPositions(
    context: DecorationContext,
    _random: SimpleRandomSource,
    config: HeightmapConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const y = context.getHeight(config.heightmap, pos.getX(), pos.getZ());
    return y > context.getMinBuildHeight() ? [new BlockPos(pos.getX(), y, pos.getZ())] : [];
  }
}
