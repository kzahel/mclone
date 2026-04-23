import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import { BaseDiskFeature } from "./base-disk-feature";
import { DiskConfiguration } from "./configurations/disk-configuration";
import { FeaturePlaceContext } from "./feature-place-context";

const SNOW_BLOCK_LOCATION = new ResourceLocation("minecraft:snow_block");

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

export class IcePatchFeature extends BaseDiskFeature {
  public override place(context: FeaturePlaceContext<DiskConfiguration>): boolean {
    const level = context.level();
    const chunkGenerator = context.chunkGenerator();
    const random = context.random();
    const config = context.config();
    let origin = context.origin();
    const snowBlock = getRequiredBlock(SNOW_BLOCK_LOCATION);

    while (level.isEmptyBlock(origin) && origin.getY() > level.getMinBuildHeight() + 2) {
      origin = origin.below();
    }

    if (!level.getBlockState(origin).is(snowBlock)) {
      return false;
    }

    return super.place(new FeaturePlaceContext(level, chunkGenerator, random, origin, config));
  }
}
