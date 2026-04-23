import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const SNOW_BLOCK_LOCATION = new ResourceLocation("minecraft:snow_block");
const ICE_LOCATION = new ResourceLocation("minecraft:ice");
const PACKED_ICE_LOCATION = new ResourceLocation("minecraft:packed_ice");

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

export class IceSpikeFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    let origin = context.origin();
    const random = context.random();
    const level = context.level();
    const snowBlock = getRequiredBlock(SNOW_BLOCK_LOCATION);
    const iceBlock = getRequiredBlock(ICE_LOCATION);
    const packedIceBlock = getRequiredBlock(PACKED_ICE_LOCATION);

    while (level.isEmptyBlock(origin) && origin.getY() > level.getMinBuildHeight() + 2) {
      origin = origin.below();
    }

    if (!level.getBlockState(origin).is(snowBlock)) {
      return false;
    }

    origin = origin.above(random.nextInt(4));
    const height = random.nextInt(4) + 7;
    let radius = Math.trunc(height / 4) + random.nextInt(2);
    if (radius > 1 && random.nextInt(60) === 0) {
      origin = origin.above(10 + random.nextInt(30));
    }

    for (let y = 0; y < height; y++) {
      const layerRadius = (1.0 - (y / height)) * radius;
      const layerRadiusInt = Math.ceil(layerRadius);
      for (let offsetX = -layerRadiusInt; offsetX <= layerRadiusInt; offsetX++) {
        const distanceX = Math.abs(offsetX) - 0.25;
        for (let offsetZ = -layerRadiusInt; offsetZ <= layerRadiusInt; offsetZ++) {
          const distanceZ = Math.abs(offsetZ) - 0.25;
          if (
            ((offsetX !== 0 || offsetZ !== 0) && ((distanceX * distanceX) + (distanceZ * distanceZ) > layerRadius * layerRadius))
            || ((offsetX === -layerRadiusInt || offsetX === layerRadiusInt || offsetZ === -layerRadiusInt || offsetZ === layerRadiusInt)
              && random.nextFloat() > 0.75)
          ) {
            continue;
          }

          const upperPos = origin.offset(offsetX, y, offsetZ);
          const upperState = level.getBlockState(upperPos);
          if (upperState.isAir() || Feature.isDirt(upperState) || upperState.is(snowBlock) || upperState.is(iceBlock)) {
            this.setBlock(level, upperPos, packedIceBlock.defaultBlockState());
          }

          if (y !== 0 && layerRadiusInt > 1) {
            const lowerPos = origin.offset(offsetX, -y, offsetZ);
            const lowerState = level.getBlockState(lowerPos);
            if (lowerState.isAir() || Feature.isDirt(lowerState) || lowerState.is(snowBlock) || lowerState.is(iceBlock)) {
              this.setBlock(level, lowerPos, packedIceBlock.defaultBlockState());
            }
          }
        }
      }
    }

    radius = Math.max(0, Math.min(radius - 1, 1));
    for (let offsetX = -radius; offsetX <= radius; offsetX++) {
      for (let offsetZ = -radius; offsetZ <= radius; offsetZ++) {
        let pos = origin.offset(offsetX, -1, offsetZ);
        let remainingDepth = 50;
        if (Math.abs(offsetX) === 1 && Math.abs(offsetZ) === 1) {
          remainingDepth = random.nextInt(5);
        }

        while (pos.getY() > 50) {
          const state = level.getBlockState(pos);
          if (!state.isAir() && !Feature.isDirt(state) && !state.is(snowBlock) && !state.is(iceBlock) && !state.is(packedIceBlock)) {
            break;
          }

          this.setBlock(level, pos, packedIceBlock.defaultBlockState());
          pos = pos.below();
          remainingDepth--;
          if (remainingDepth <= 0) {
            pos = pos.below(random.nextInt(5) + 1);
            remainingDepth = random.nextInt(5);
          }
        }
      }
    }

    return true;
  }
}
