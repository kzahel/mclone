import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { Heightmap } from "../heightmap";
import { WaterDepthThresholdConfiguration } from "../feature/configurations/water-depth-threshold-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class WaterDepthThresholdDecorator extends FeatureDecorator<WaterDepthThresholdConfiguration> {
  public override getPositions(
    context: DecorationContext,
    _random: SimpleRandomSource,
    config: WaterDepthThresholdConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const oceanFloorY = context.getHeight(Heightmap.Types.OCEAN_FLOOR, pos.getX(), pos.getZ());
    const worldSurfaceY = context.getHeight(Heightmap.Types.WORLD_SURFACE, pos.getX(), pos.getZ());
    return worldSurfaceY - oceanFloorY > config.maxWaterDepth ? [] : [pos];
  }
}
