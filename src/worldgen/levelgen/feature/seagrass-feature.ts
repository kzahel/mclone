import { BlockPos } from "../../../core/block-pos";
import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import { DoublePlantBlock } from "../../../world/level/block/double-plant-block";
import { DoubleBlockHalf } from "../../../world/level/block/state/properties/double-block-half";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { ProbabilityFeatureConfiguration } from "./configurations/probability-feature-configuration";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const SEAGRASS_LOCATION = new ResourceLocation("minecraft:seagrass");
const TALL_SEAGRASS_LOCATION = new ResourceLocation("minecraft:tall_seagrass");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class SeagrassFeature extends Feature<ProbabilityFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<ProbabilityFeatureConfiguration>): boolean {
    let placed = false;
    const random = context.random();
    const level = context.level();
    const origin = context.origin();
    const config = context.config();
    const offsetX = random.nextInt(8) - random.nextInt(8);
    const offsetZ = random.nextInt(8) - random.nextInt(8);
    const y = level.getHeight(Heightmap.Types.OCEAN_FLOOR, origin.getX() + offsetX, origin.getZ() + offsetZ);
    const pos = new BlockPos(origin.getX() + offsetX, y, origin.getZ() + offsetZ);
    if (level.getBlockState(pos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString()) {
      const tall = random.nextDouble() < config.probability;
      const state = tall ? getRequiredState(TALL_SEAGRASS_LOCATION) : getRequiredState(SEAGRASS_LOCATION);
      if (state.canSurvive(level, pos)) {
        if (tall) {
          const upperState = state.setValue(DoublePlantBlock.HALF, DoubleBlockHalf.UPPER);
          const abovePos = pos.above();
          if (level.getBlockState(abovePos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString()) {
            level.setBlock(pos, state, 2);
            level.setBlock(abovePos, upperState, 2);
          }
        } else {
          level.setBlock(pos, state, 2);
        }

        placed = true;
      }
    }

    return placed;
  }
}
