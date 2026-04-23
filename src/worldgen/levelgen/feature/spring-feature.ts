import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { SpringConfiguration } from "./configurations/spring-configuration";

export class SpringFeature extends Feature<SpringConfiguration> {
  public override place(context: FeaturePlaceContext<SpringConfiguration>): boolean {
    const config = context.config();
    const level = context.level();
    const pos = context.origin();
    if (!config.validBlocks.has(level.getBlockState(pos.above()).getBlock())) {
      return false;
    }

    if (config.requiresBlockBelow && !config.validBlocks.has(level.getBlockState(pos.below()).getBlock())) {
      return false;
    }

    const state = level.getBlockState(pos);
    if (!state.isAir() && !config.validBlocks.has(state.getBlock())) {
      return false;
    }

    let rockCount = 0;
    if (config.validBlocks.has(level.getBlockState(pos.west()).getBlock())) {
      rockCount++;
    }
    if (config.validBlocks.has(level.getBlockState(pos.east()).getBlock())) {
      rockCount++;
    }
    if (config.validBlocks.has(level.getBlockState(pos.north()).getBlock())) {
      rockCount++;
    }
    if (config.validBlocks.has(level.getBlockState(pos.south()).getBlock())) {
      rockCount++;
    }
    if (config.validBlocks.has(level.getBlockState(pos.below()).getBlock())) {
      rockCount++;
    }

    let holeCount = 0;
    if (level.isEmptyBlock(pos.west())) {
      holeCount++;
    }
    if (level.isEmptyBlock(pos.east())) {
      holeCount++;
    }
    if (level.isEmptyBlock(pos.north())) {
      holeCount++;
    }
    if (level.isEmptyBlock(pos.south())) {
      holeCount++;
    }
    if (level.isEmptyBlock(pos.below())) {
      holeCount++;
    }

    if (rockCount !== config.rockCount || holeCount !== config.holeCount) {
      return false;
    }

    level.setBlock(pos, config.state.createLegacyBlock(), 2);
    level.getLiquidTicks().scheduleTick(pos, config.state.getType(), 0);
    return true;
  }
}
