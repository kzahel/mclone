import { Direction } from "../../../core/direction";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { SmallDripstoneConfiguration } from "./configurations/small-dripstone-configuration";
import { DripstoneUtils } from "./dripstone-utils";

function randomBetweenInclusive(
  random: import("../../prng/simple-random-source").SimpleRandomSource,
  minInclusive: number,
  maxInclusive: number,
): number {
  return minInclusive + random.nextInt((maxInclusive - minInclusive) + 1);
}

function randomDirection(random: import("../../prng/simple-random-source").SimpleRandomSource): Direction {
  const directions = Direction.values();
  return directions[random.nextInt(directions.length)]!;
}

export class SmallDripstoneFeature extends Feature<SmallDripstoneConfiguration> {
  public override place(context: FeaturePlaceContext<SmallDripstoneConfiguration>): boolean {
    const level = context.level();
    const origin = context.origin();
    const random = context.random();
    const config = context.config();
    if (!DripstoneUtils.isEmptyOrWater(level, origin)) {
      return false;
    }

    const placements = randomBetweenInclusive(random, 1, config.maxPlacements);
    let placed = false;
    for (let index = 0; index < placements; index++) {
      const pos = SmallDripstoneFeature.randomOffset(random, origin, config);
      if (SmallDripstoneFeature.searchAndTryToPlaceDripstone(level, random, pos, config)) {
        placed = true;
      }
    }

    return placed;
  }

  private static searchAndTryToPlaceDripstone(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    pos: import("../../../core/block-pos").BlockPos,
    config: SmallDripstoneConfiguration,
  ): boolean {
    const direction = randomDirection(random);
    const tipDirection = random.nextBoolean() ? Direction.UP : Direction.DOWN;
    const mutable = pos.mutable();

    for (let index = 0; index < config.emptySpaceSearchRadius; index++) {
      if (!DripstoneUtils.isEmptyOrWater(level, mutable)) {
        return false;
      }

      if (SmallDripstoneFeature.tryToPlaceDripstone(level, random, mutable, tipDirection, config)) {
        return true;
      }

      if (SmallDripstoneFeature.tryToPlaceDripstone(level, random, mutable, tipDirection.getOpposite(), config)) {
        return true;
      }

      mutable.move(direction);
    }

    return false;
  }

  private static tryToPlaceDripstone(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    pos: import("../../../core/block-pos").BlockPos,
    direction: Direction,
    config: SmallDripstoneConfiguration,
  ): boolean {
    if (!DripstoneUtils.isEmptyOrWater(level, pos)) {
      return false;
    }

    const basePos = pos.relative(direction.getOpposite());
    const baseState = level.getBlockState(basePos);
    if (!DripstoneUtils.isDripstoneBase(baseState)) {
      return false;
    }

    SmallDripstoneFeature.createPatchOfDripstoneBlocks(level, random, basePos);
    const dripstoneHeight =
      random.nextFloat() < config.chanceOfTallerDripstone && DripstoneUtils.isEmptyOrWater(level, pos.relative(direction)) ? 2 : 1;
    DripstoneUtils.growPointedDripstone(level, pos, direction, dripstoneHeight, false);
    return true;
  }

  private static createPatchOfDripstoneBlocks(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    pos: import("../../../core/block-pos").BlockPos,
  ): void {
    DripstoneUtils.placeDripstoneBlockIfPossible(level, pos);

    for (const direction of Direction.Plane.HORIZONTAL) {
      if (random.nextFloat() < 0.3) {
        continue;
      }

      const offsetPos = pos.relative(direction);
      DripstoneUtils.placeDripstoneBlockIfPossible(level, offsetPos);
      if (!random.nextBoolean()) {
        const secondPos = offsetPos.relative(randomDirection(random));
        DripstoneUtils.placeDripstoneBlockIfPossible(level, secondPos);
        if (!random.nextBoolean()) {
          const thirdPos = secondPos.relative(randomDirection(random));
          DripstoneUtils.placeDripstoneBlockIfPossible(level, thirdPos);
        }
      }
    }
  }

  private static randomOffset(
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    pos: import("../../../core/block-pos").BlockPos,
    config: SmallDripstoneConfiguration,
  ): import("../../../core/block-pos").BlockPos {
    return pos.offset(
      randomBetweenInclusive(random, -config.maxOffsetFromOrigin, config.maxOffsetFromOrigin),
      randomBetweenInclusive(random, -config.maxOffsetFromOrigin, config.maxOffsetFromOrigin),
      randomBetweenInclusive(random, -config.maxOffsetFromOrigin, config.maxOffsetFromOrigin),
    );
  }
}
