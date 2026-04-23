import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { SimpleBlockConfiguration } from "./configurations/simple-block-configuration";

export class SimpleBlockFeature extends Feature<SimpleBlockConfiguration> {
  public override place(context: FeaturePlaceContext<SimpleBlockConfiguration>): boolean {
    const config = context.config();
    const level = context.level();
    const pos = context.origin();
    if (
      (config.placeOn.length === 0 || config.placeOn.includes(level.getBlockState(pos.below()))) &&
      (config.placeIn.length === 0 || config.placeIn.includes(level.getBlockState(pos))) &&
      (config.placeUnder.length === 0 || config.placeUnder.includes(level.getBlockState(pos.above())))
    ) {
      const state = config.toPlace.getState(context.random(), pos);
      if (state.canSurvive(level, pos)) {
        level.setBlock(pos, state, 2);
        return true;
      }
    }

    return false;
  }
}
