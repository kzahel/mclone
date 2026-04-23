import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { SnowyDirtBlock } from "../../../world/level/block/snowy-dirt-block";
import type { Block } from "../../../world/level/block/block";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const ICE_LOCATION = new ResourceLocation("minecraft:ice");
const SNOW_LOCATION = new ResourceLocation("minecraft:snow");

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

export class SnowAndFreezeFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    const level = context.level();
    const origin = context.origin();
    const snowPos = new BlockPos.MutableBlockPos();
    const freezePos = new BlockPos.MutableBlockPos();
    const iceState = getRequiredBlock(ICE_LOCATION).defaultBlockState();
    const snowState = getRequiredBlock(SNOW_LOCATION).defaultBlockState();

    for (let offsetX = 0; offsetX < 16; offsetX++) {
      for (let offsetZ = 0; offsetZ < 16; offsetZ++) {
        const worldX = origin.getX() + offsetX;
        const worldZ = origin.getZ() + offsetZ;
        const topY = level.getHeight(Heightmap.Types.MOTION_BLOCKING, worldX, worldZ);
        snowPos.set(worldX, topY, worldZ);
        freezePos.set(worldX, topY, worldZ).move(Direction.DOWN);
        const biome = level.getBiome(snowPos);
        if (biome.shouldFreeze(level, freezePos, false)) {
          level.setBlock(freezePos, iceState, 2);
        }

        if (biome.shouldSnow(level, snowPos)) {
          level.setBlock(snowPos, snowState, 2);
          const belowState = level.getBlockState(freezePos);
          if (belowState.hasProperty(SnowyDirtBlock.SNOWY)) {
            level.setBlock(freezePos, belowState.setValue(SnowyDirtBlock.SNOWY, true), 2);
          }
        }
      }
    }

    return true;
  }
}
