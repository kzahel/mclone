import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { BlockTags } from "../../../tags/block-tags";
import { BaseCoralWallFanBlock } from "../../../world/level/block/base-coral-wall-fan-block";
import { SeaPickleBlock } from "../../../world/level/block/sea-pickle-block";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const SEA_PICKLE_LOCATION = new ResourceLocation("minecraft:sea_pickle");

const CORAL_BLOCK_LOCATIONS = [
  new ResourceLocation("minecraft:tube_coral_block"),
  new ResourceLocation("minecraft:brain_coral_block"),
  new ResourceLocation("minecraft:bubble_coral_block"),
  new ResourceLocation("minecraft:fire_coral_block"),
  new ResourceLocation("minecraft:horn_coral_block"),
] as const;

const CORAL_LOCATIONS = [
  new ResourceLocation("minecraft:tube_coral"),
  new ResourceLocation("minecraft:brain_coral"),
  new ResourceLocation("minecraft:bubble_coral"),
  new ResourceLocation("minecraft:fire_coral"),
  new ResourceLocation("minecraft:horn_coral"),
  new ResourceLocation("minecraft:tube_coral_fan"),
  new ResourceLocation("minecraft:brain_coral_fan"),
  new ResourceLocation("minecraft:bubble_coral_fan"),
  new ResourceLocation("minecraft:fire_coral_fan"),
  new ResourceLocation("minecraft:horn_coral_fan"),
] as const;

const WALL_CORAL_LOCATIONS = [
  new ResourceLocation("minecraft:tube_coral_wall_fan"),
  new ResourceLocation("minecraft:brain_coral_wall_fan"),
  new ResourceLocation("minecraft:bubble_coral_wall_fan"),
  new ResourceLocation("minecraft:fire_coral_wall_fan"),
  new ResourceLocation("minecraft:horn_coral_wall_fan"),
] as const;

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function getRandomState(random: SimpleRandomSource, locations: readonly ResourceLocation[]): BlockState {
  return getRequiredState(locations[random.nextInt(locations.length)]!);
}

export abstract class CoralFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    return this.placeFeature(context.level(), context.random(), context.origin(), getRandomState(context.random(), CORAL_BLOCK_LOCATIONS));
  }

  protected abstract placeFeature(level: WorldGenLevel, random: SimpleRandomSource, pos: BlockPos, state: BlockState): boolean;

  protected placeCoralBlock(level: WorldGenLevel, random: SimpleRandomSource, pos: BlockPos, state: BlockState): boolean {
    const abovePos = pos.above();
    const existingState = level.getBlockState(pos);
    if (
      (existingState.getBlock().getLocation()?.toString() === WATER_LOCATION.toString() || existingState.is(BlockTags.CORALS)) &&
      level.getBlockState(abovePos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString()
    ) {
      level.setBlock(pos, state, 3);
      if (random.nextFloat() < 0.25) {
        level.setBlock(abovePos, getRandomState(random, CORAL_LOCATIONS), 2);
      } else if (random.nextFloat() < 0.05) {
        level.setBlock(
          abovePos,
          getRequiredState(SEA_PICKLE_LOCATION).setValue(SeaPickleBlock.PICKLES, random.nextInt(4) + 1),
          2,
        );
      }

      for (const direction of Direction.Plane.HORIZONTAL) {
        if (random.nextFloat() < 0.2) {
          const relativePos = pos.relative(direction);
          if (level.getBlockState(relativePos).getBlock().getLocation()?.toString() === WATER_LOCATION.toString()) {
            let wallState = getRandomState(random, WALL_CORAL_LOCATIONS);
            if (wallState.hasProperty(BaseCoralWallFanBlock.FACING)) {
              wallState = wallState.setValue(BaseCoralWallFanBlock.FACING, direction);
            }

            level.setBlock(relativePos, wallState, 2);
          }
        }
      }

      return true;
    }

    return false;
  }
}
