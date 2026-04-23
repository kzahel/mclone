import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import { Direction } from "../../../core/direction";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { VineBlock } from "../../../world/level/block/vine-block";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const VINE_LOCATION = new ResourceLocation("minecraft:vine");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class VinesFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    const level = context.level();
    const pos = context.origin();
    if (!level.isEmptyBlock(pos)) {
      return false;
    }

    for (const direction of Direction.values()) {
      if (direction !== Direction.DOWN && VineBlock.isAcceptableNeighbour(level, pos.relative(direction), direction)) {
        level.setBlock(pos, getRequiredState(VINE_LOCATION).setValue(VineBlock.getPropertyForFace(direction), true), 2);
        return true;
      }
    }

    return false;
  }
}
