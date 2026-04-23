import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { CoralFeature } from "./coral-feature";

function getRandomHorizontalDirection(random: SimpleRandomSource): Direction {
  return Direction.Plane.HORIZONTAL.stream()[random.nextInt(4)]!;
}

function shuffleDirections(random: SimpleRandomSource, directions: readonly Direction[]): Direction[] {
  const shuffled = [...directions];
  for (let index = shuffled.length - 1; index > 0; index--) {
    const swapIndex = random.nextInt(index + 1);
    [shuffled[index], shuffled[swapIndex]] = [shuffled[swapIndex]!, shuffled[index]!];
  }

  return shuffled;
}

export class CoralClawFeature extends CoralFeature {
  protected override placeFeature(level: WorldGenLevel, random: SimpleRandomSource, pos: BlockPos, state: BlockState): boolean {
    if (!this.placeCoralBlock(level, random, pos, state)) {
      return false;
    }

    const mainDirection = getRandomHorizontalDirection(random);
    const armCount = random.nextInt(2) + 2;
    const directions = shuffleDirections(random, [mainDirection, mainDirection.getClockWise(), mainDirection.getCounterClockWise()]).slice(0, armCount);

    for (const direction of directions) {
      const mutablePos = pos.mutable();
      const stemLength = random.nextInt(2) + 1;
      mutablePos.move(direction);
      let growthDirection: Direction;
      let growthLength: number;
      if (direction === mainDirection) {
        growthDirection = mainDirection;
        growthLength = random.nextInt(3) + 2;
      } else {
        mutablePos.move(Direction.UP);
        const candidateDirections = [direction, Direction.UP] as const;
        growthDirection = candidateDirections[random.nextInt(candidateDirections.length)]!;
        growthLength = random.nextInt(3) + 3;
      }

      for (let index = 0; index < stemLength && this.placeCoralBlock(level, random, mutablePos, state); index++) {
        mutablePos.move(growthDirection);
      }

      mutablePos.move(growthDirection.getOpposite());
      mutablePos.move(Direction.UP);

      for (let index = 0; index < growthLength; index++) {
        mutablePos.move(mainDirection);
        if (!this.placeCoralBlock(level, random, mutablePos, state)) {
          break;
        }

        if (random.nextFloat() < 0.25) {
          mutablePos.move(Direction.UP);
        }
      }
    }

    return true;
  }
}
