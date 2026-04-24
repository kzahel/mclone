import { BlockPos } from "../../../core/block-pos";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { Direction } from "../../../core/direction";
import type { Block } from "../../../world/level/block/block";
import { GlowLichenBlock } from "../../../world/level/block/glow-lichen-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { GlowLichenConfiguration } from "./configurations/glow-lichen-configuration";

const GLOW_LICHEN_LOCATION = new ResourceLocation("minecraft:glow_lichen");
const WATER_LOCATION = new ResourceLocation("minecraft:water");

function getRequiredGlowLichenBlock(): GlowLichenBlock {
  const block = Registry.BLOCK.get(GLOW_LICHEN_LOCATION) as Block | undefined;
  if (!(block instanceof GlowLichenBlock)) {
    throw new Error("Missing registered block minecraft:glow_lichen");
  }

  return block;
}

function getWaterBlock(): Block {
  const block = Registry.BLOCK.get(WATER_LOCATION) as Block | undefined;
  if (block === undefined) {
    throw new Error("Missing registered block minecraft:water");
  }

  return block;
}

function shuffleDirections(directions: readonly Direction[], random: import("../../prng/simple-random-source").SimpleRandomSource): Direction[] {
  const shuffled = [...directions];
  for (let index = shuffled.length - 1; index > 0; index--) {
    const swapIndex = random.nextInt(index + 1);
    const value = shuffled[index]!;
    shuffled[index] = shuffled[swapIndex]!;
    shuffled[swapIndex] = value;
  }

  return shuffled;
}

export class GlowLichenFeature extends Feature<GlowLichenConfiguration> {
  public override place(context: FeaturePlaceContext<GlowLichenConfiguration>): boolean {
    const level = context.level();
    const origin = context.origin();
    const random = context.random();
    const config = context.config();
    if (!GlowLichenFeature.isAirOrWater(level.getBlockState(origin))) {
      return false;
    }

    const directions = GlowLichenFeature.getShuffledDirections(config, random);
    if (GlowLichenFeature.placeGlowLichenIfPossible(level, origin, level.getBlockState(origin), config, random, directions)) {
      return true;
    }

    const mutable = origin.mutable();
    for (const direction of directions) {
      mutable.set(origin.getX(), origin.getY(), origin.getZ());
      const spreadDirections = GlowLichenFeature.getShuffledDirectionsExcept(config, random, direction.getOpposite());

      for (let index = 0; index < config.searchRange; index++) {
        mutable.setWithOffset(origin, direction);
        const state = level.getBlockState(mutable);
        if (!GlowLichenFeature.isAirOrWater(state) && !state.is(getRequiredGlowLichenBlock())) {
          break;
        }

        if (GlowLichenFeature.placeGlowLichenIfPossible(level, mutable, state, config, random, spreadDirections)) {
          return true;
        }
      }
    }

    return false;
  }

  public static placeGlowLichenIfPossible(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    pos: BlockPos,
    state: BlockState,
    config: GlowLichenConfiguration,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    directions: readonly Direction[],
  ): boolean {
    const mutable = pos.mutable();
    const glowLichen = getRequiredGlowLichenBlock();

    for (const direction of directions) {
      const neighborState = level.getBlockState(mutable.setWithOffset(pos, direction));
      if (!config.canBePlacedOn(neighborState.getBlock())) {
        continue;
      }

      const placedState = glowLichen.getStateForPlacement(state, level, pos, direction);
      if (placedState === undefined) {
        return false;
      }

      level.setBlock(pos, placedState, 3);
      if (random.nextFloat() < config.chanceOfSpreading) {
        glowLichen.spreadFromFaceTowardRandomDirection(placedState, level, pos, direction, random, true);
      }

      return true;
    }

    return false;
  }

  public static getShuffledDirections(
    config: GlowLichenConfiguration,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
  ): readonly Direction[] {
    return shuffleDirections(config.validDirections, random);
  }

  public static getShuffledDirectionsExcept(
    config: GlowLichenConfiguration,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    excludedDirection: Direction,
  ): readonly Direction[] {
    return shuffleDirections(
      config.validDirections.filter((direction) => direction !== excludedDirection),
      random,
    );
  }

  private static isAirOrWater(state: BlockState): boolean {
    return state.isAir() || state.is(getWaterBlock());
  }
}
