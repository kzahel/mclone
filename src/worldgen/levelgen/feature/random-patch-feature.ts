import { BlockPos } from "../../../core/block-pos";
import { Fluids } from "../../../world/level/material/fluids";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";

export class RandomPatchFeature extends Feature<RandomPatchConfiguration> {
  public override place(context: FeaturePlaceContext<RandomPatchConfiguration>): boolean {
    const config = context.config();
    const random = context.random();
    const origin = context.origin();
    const level = context.level();
    const state = config.stateProvider.getState(random, origin);
    const basePos = config.project ? level.getHeightmapPos(Heightmap.Types.WORLD_SURFACE_WG, origin) : origin;
    let placedCount = 0;
    const mutablePos = new BlockPos.MutableBlockPos();

    for (let index = 0; index < config.tries; index++) {
      mutablePos.setWithOffset(
        basePos,
        random.nextInt(config.xspread + 1) - random.nextInt(config.xspread + 1),
        random.nextInt(config.yspread + 1) - random.nextInt(config.yspread + 1),
        random.nextInt(config.zspread + 1) - random.nextInt(config.zspread + 1),
      );
      const belowPos = mutablePos.below();
      const belowState = level.getBlockState(belowPos);
      if (
        (level.isEmptyBlock(mutablePos) || (config.canReplace && level.getBlockState(mutablePos).getMaterial().isReplaceable())) &&
        state.canSurvive(level, mutablePos) &&
        (config.whitelist.size === 0 || config.whitelist.has(belowState.getBlock())) &&
        !config.blacklist.has(belowState) &&
        (!config.needWater ||
          level.getFluidState(belowPos.west()).getType().isSame(Fluids.WATER) ||
          level.getFluidState(belowPos.east()).getType().isSame(Fluids.WATER) ||
          level.getFluidState(belowPos.north()).getType().isSame(Fluids.WATER) ||
          level.getFluidState(belowPos.south()).getType().isSame(Fluids.WATER))
      ) {
        config.blockPlacer.place(level, mutablePos, state, random);
        placedCount++;
      }
    }

    return placedCount > 0;
  }
}
