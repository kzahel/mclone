import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { CoralFeature } from "./coral-feature";

function shuffleDirections(random: SimpleRandomSource): Direction[] {
  const directions = [...Direction.Plane.HORIZONTAL.stream()];
  for (let index = directions.length - 1; index > 0; index--) {
    const swapIndex = random.nextInt(index + 1);
    [directions[index], directions[swapIndex]] = [directions[swapIndex]!, directions[index]!];
  }

  return directions;
}

export class CoralTreeFeature extends CoralFeature {
  protected override placeFeature(level: WorldGenLevel, random: SimpleRandomSource, pos: BlockPos, state: BlockState): boolean {
    const mutablePos = pos.mutable();
    const trunkHeight = random.nextInt(3) + 1;

    for (let index = 0; index < trunkHeight; index++) {
      if (!this.placeCoralBlock(level, random, mutablePos, state)) {
        return true;
      }

      mutablePos.move(Direction.UP);
    }

    const branchStart = new BlockPos(mutablePos.getX(), mutablePos.getY(), mutablePos.getZ());
    const branchCount = random.nextInt(3) + 2;
    const directions = shuffleDirections(random);

    for (const direction of directions.slice(0, branchCount)) {
      mutablePos.set(branchStart.getX(), branchStart.getY(), branchStart.getZ());
      mutablePos.move(direction);
      const branchLength = random.nextInt(5) + 2;
      let branchDepth = 0;

      for (let index = 0; index < branchLength && this.placeCoralBlock(level, random, mutablePos, state); index++) {
        branchDepth++;
        mutablePos.move(Direction.UP);
        if (index === 0 || (branchDepth >= 2 && random.nextFloat() < 0.25)) {
          mutablePos.move(direction);
          branchDepth = 0;
        }
      }
    }

    return true;
  }
}
