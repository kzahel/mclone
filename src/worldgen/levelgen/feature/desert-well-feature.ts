import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const SANDSTONE_LOCATION = new ResourceLocation("minecraft:sandstone");
const SANDSTONE_SLAB_LOCATION = new ResourceLocation("minecraft:sandstone_slab");
const WATER_LOCATION = new ResourceLocation("minecraft:water");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class DesertWellFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    const level = context.level();
    const sandSlab = getRequiredState(SANDSTONE_SLAB_LOCATION);
    const sandstone = getRequiredState(SANDSTONE_LOCATION);
    const water = getRequiredState(WATER_LOCATION);
    const sand = getRequiredState(SAND_LOCATION).getBlock();
    let origin = context.origin().above();

    while (level.isEmptyBlock(origin) && origin.getY() > level.getMinBuildHeight() + 2) {
      origin = origin.below();
    }

    if (!level.getBlockState(origin).is(sand)) {
      return false;
    }

    for (let offsetX = -2; offsetX <= 2; offsetX++) {
      for (let offsetZ = -2; offsetZ <= 2; offsetZ++) {
        if (level.isEmptyBlock(origin.offset(offsetX, -1, offsetZ)) && level.isEmptyBlock(origin.offset(offsetX, -2, offsetZ))) {
          return false;
        }
      }
    }

    for (let offsetY = -1; offsetY <= 0; offsetY++) {
      for (let offsetX = -2; offsetX <= 2; offsetX++) {
        for (let offsetZ = -2; offsetZ <= 2; offsetZ++) {
          level.setBlock(origin.offset(offsetX, offsetY, offsetZ), sandstone, 2);
        }
      }
    }

    level.setBlock(origin, water, 2);
    for (const direction of Direction.Plane.HORIZONTAL) {
      level.setBlock(origin.relative(direction), water, 2);
    }

    for (let offsetX = -2; offsetX <= 2; offsetX++) {
      for (let offsetZ = -2; offsetZ <= 2; offsetZ++) {
        if (offsetX === -2 || offsetX === 2 || offsetZ === -2 || offsetZ === 2) {
          level.setBlock(origin.offset(offsetX, 1, offsetZ), sandstone, 2);
        }
      }
    }

    level.setBlock(origin.offset(2, 1, 0), sandSlab, 2);
    level.setBlock(origin.offset(-2, 1, 0), sandSlab, 2);
    level.setBlock(origin.offset(0, 1, 2), sandSlab, 2);
    level.setBlock(origin.offset(0, 1, -2), sandSlab, 2);

    for (let offsetX = -1; offsetX <= 1; offsetX++) {
      for (let offsetZ = -1; offsetZ <= 1; offsetZ++) {
        level.setBlock(origin.offset(offsetX, 4, offsetZ), offsetX === 0 && offsetZ === 0 ? sandstone : sandSlab, 2);
      }
    }

    for (let offsetY = 1; offsetY <= 3; offsetY++) {
      level.setBlock(origin.offset(-1, offsetY, -1), sandstone, 2);
      level.setBlock(origin.offset(-1, offsetY, 1), sandstone, 2);
      level.setBlock(origin.offset(1, offsetY, -1), sandstone, 2);
      level.setBlock(origin.offset(1, offsetY, 1), sandstone, 2);
    }

    return true;
  }
}
