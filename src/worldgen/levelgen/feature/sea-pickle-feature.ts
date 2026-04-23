import { BlockPos } from "../../../core/block-pos";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { SeaPickleBlock } from "../../../world/level/block/sea-pickle-block";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { CountConfiguration } from "./configurations/count-configuration";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const SEA_PICKLE_LOCATION = new ResourceLocation("minecraft:sea_pickle");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class SeaPickleFeature extends Feature<CountConfiguration> {
  public override place(context: FeaturePlaceContext<CountConfiguration>): boolean {
    let placed = 0;
    const random = context.random();
    const level = context.level();
    const origin = context.origin();
    const count = context.config().count().sample(random);

    for (let index = 0; index < count; index++) {
      const offsetX = random.nextInt(8) - random.nextInt(8);
      const offsetZ = random.nextInt(8) - random.nextInt(8);
      const y = level.getHeight(Heightmap.Types.OCEAN_FLOOR, origin.getX() + offsetX, origin.getZ() + offsetZ);
      const pos = new BlockPos(origin.getX() + offsetX, y, origin.getZ() + offsetZ);
      const state = getRequiredState(SEA_PICKLE_LOCATION).setValue(SeaPickleBlock.PICKLES, random.nextInt(4) + 1);
      if (level.getBlockState(pos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString() && state.canSurvive(level, pos)) {
        level.setBlock(pos, state, 2);
        placed++;
      }
    }

    return placed > 0;
  }
}
