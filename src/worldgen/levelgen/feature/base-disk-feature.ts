import { BlockPos } from "../../../core/block-pos";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { DiskConfiguration } from "./configurations/disk-configuration";

const RED_SAND_LOCATION = new ResourceLocation("minecraft:red_sand");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const GRAVEL_LOCATION = new ResourceLocation("minecraft:gravel");
const RED_SANDSTONE_LOCATION = new ResourceLocation("minecraft:red_sandstone");
const SANDSTONE_LOCATION = new ResourceLocation("minecraft:sandstone");
const RED_SAND_LOCATION_STRING = RED_SAND_LOCATION.toString();

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function isFallingDiskState(state: BlockState): boolean {
  const location = state.getBlock().getLocation()?.toString();
  return location === RED_SAND_LOCATION.toString() || location === SAND_LOCATION.toString() || location === GRAVEL_LOCATION.toString();
}

export class BaseDiskFeature extends Feature<DiskConfiguration> {
  public override place(context: FeaturePlaceContext<DiskConfiguration>): boolean {
    const config = context.config();
    const origin = context.origin();
    const level = context.level();
    let placed = false;
    const topY = origin.getY() + config.halfHeight;
    const bottomY = origin.getY() - config.halfHeight - 1;
    const fallingState = isFallingDiskState(config.state);
    const radius = config.radius.sample(context.random());

    for (let x = origin.getX() - radius; x <= origin.getX() + radius; x++) {
      for (let z = origin.getZ() - radius; z <= origin.getZ() + radius; z++) {
        const offsetX = x - origin.getX();
        const offsetZ = z - origin.getZ();
        if ((offsetX * offsetX) + (offsetZ * offsetZ) > radius * radius) {
          continue;
        }

        let replacedPreviousLayer = false;
        for (let y = topY; y >= bottomY; y--) {
          const pos = new BlockPos(x, y, z);
          const state = level.getBlockState(pos);
          let replaced = false;
          if (y > bottomY) {
            for (const target of config.targets) {
              if (target.is(state.getBlock())) {
                level.setBlock(pos, config.state, 2);
                this.markAboveForPostProcessing(level, pos);
                placed = true;
                replaced = true;
                break;
              }
            }
          }

          if (fallingState && replacedPreviousLayer && state.isAir()) {
            const foundationState =
              config.state.getBlock().getLocation()?.toString() === RED_SAND_LOCATION_STRING
                ? getRequiredState(RED_SANDSTONE_LOCATION)
                : getRequiredState(SANDSTONE_LOCATION);
            level.setBlock(pos.above(), foundationState, 2);
          }

          replacedPreviousLayer = replaced;
        }
      }
    }

    return placed;
  }
}
