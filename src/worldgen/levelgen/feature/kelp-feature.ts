import { BlockPos } from "../../../core/block-pos";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import { KelpBlock } from "../../../world/level/block/kelp-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const KELP_LOCATION = new ResourceLocation("minecraft:kelp");
const KELP_PLANT_LOCATION = new ResourceLocation("minecraft:kelp_plant");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class KelpFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    let placed = 0;
    const level = context.level();
    const origin = context.origin();
    const random = context.random();
    const y = level.getHeight(Heightmap.Types.OCEAN_FLOOR, origin.getX(), origin.getZ());
    let pos = new BlockPos(origin.getX(), y, origin.getZ());
    if (level.getBlockState(pos).getBlock().getLocation()?.toString() !== WATER_LOCATION.toString()) {
      return false;
    }

    const kelpState = getRequiredState(KELP_LOCATION);
    const kelpPlantState = getRequiredState(KELP_PLANT_LOCATION);
    const height = 1 + random.nextInt(10);

    for (let index = 0; index <= height; index++) {
      const abovePos = pos.above();
      const isWaterColumn =
        level.getBlockState(pos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString() &&
        level.getBlockState(abovePos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString();
      if (isWaterColumn && kelpPlantState.canSurvive(level, pos)) {
        if (index === height) {
          level.setBlock(pos, kelpState.setValue(KelpBlock.AGE, random.nextInt(4) + 20), 2);
          placed++;
        } else {
          level.setBlock(pos, kelpPlantState, 2);
        }
      } else if (index > 0) {
        const belowPos = pos.below();
        if (
          kelpState.canSurvive(level, belowPos) &&
          level.getBlockState(belowPos.below()).getBlock().getLocation()?.toString() !== KELP_LOCATION.toString()
        ) {
          level.setBlock(belowPos, kelpState.setValue(KelpBlock.AGE, random.nextInt(4) + 20), 2);
          placed++;
        }
        break;
      }

      pos = abovePos;
    }

    return placed > 0;
  }
}
