import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import { BambooBlock } from "../../../world/level/block/bamboo-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { BambooLeaves } from "../../../world/level/block/state/properties/bamboo-leaves";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { ProbabilityFeatureConfiguration } from "./configurations/probability-feature-configuration";

const BAMBOO_LOCATION = new ResourceLocation("minecraft:bamboo");
const PODZOL_LOCATION = new ResourceLocation("minecraft:podzol");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function getBambooTrunkState(): BlockState {
  return getRequiredState(BAMBOO_LOCATION)
    .setValue(BambooBlock.AGE, 1)
    .setValue(BambooBlock.LEAVES, BambooLeaves.NONE)
    .setValue(BambooBlock.STAGE, 0);
}

function getBambooFinalLargeState(): BlockState {
  return getBambooTrunkState().setValue(BambooBlock.LEAVES, BambooLeaves.LARGE).setValue(BambooBlock.STAGE, 1);
}

function getBambooTopLargeState(): BlockState {
  return getBambooTrunkState().setValue(BambooBlock.LEAVES, BambooLeaves.LARGE);
}

function getBambooTopSmallState(): BlockState {
  return getBambooTrunkState().setValue(BambooBlock.LEAVES, BambooLeaves.SMALL);
}

export class BambooFeature extends Feature<ProbabilityFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<ProbabilityFeatureConfiguration>): boolean {
    let placed = 0;
    const origin = context.origin();
    const level = context.level();
    const random = context.random();
    const config = context.config();
    const bambooPos = origin.mutable();
    const podzolPos = origin.mutable();

    if (level.isEmptyBlock(bambooPos)) {
      if (getRequiredState(BAMBOO_LOCATION).canSurvive(level, bambooPos)) {
        const height = random.nextInt(12) + 5;
        if (random.nextFloat() < config.probability) {
          const radius = random.nextInt(4) + 1;

          for (let x = origin.getX() - radius; x <= origin.getX() + radius; x++) {
            for (let z = origin.getZ() - radius; z <= origin.getZ() + radius; z++) {
              const dx = x - origin.getX();
              const dz = z - origin.getZ();
              if ((dx * dx) + (dz * dz) <= radius * radius) {
                podzolPos.set(x, level.getHeight(Heightmap.Types.WORLD_SURFACE, x, z) - 1, z);
                if (Feature.isDirt(level.getBlockState(podzolPos))) {
                  level.setBlock(podzolPos, getRequiredState(PODZOL_LOCATION), 2);
                }
              }
            }
          }
        }

        const bambooTrunk = getBambooTrunkState();
        for (let index = 0; index < height && level.isEmptyBlock(bambooPos); index++) {
          level.setBlock(bambooPos, bambooTrunk, 2);
          bambooPos.move(Direction.UP);
        }

        if (bambooPos.getY() - origin.getY() >= 3) {
          level.setBlock(bambooPos, getBambooFinalLargeState(), 2);
          bambooPos.move(Direction.DOWN);
          level.setBlock(bambooPos, getBambooTopLargeState(), 2);
          bambooPos.move(Direction.DOWN);
          level.setBlock(bambooPos, getBambooTopSmallState(), 2);
        }
      }

      placed++;
    }

    return placed > 0;
  }
}
