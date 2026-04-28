import { BlockPos } from "../../../../core/block-pos";
import { Vec3 } from "../../../phys/vec3";
import type { PathfinderMob } from "../pathfinder-mob";

export const RANDOM_POS_ATTEMPTS = 10;

export function generateRandomDirection(
  random: { nextInt(bound: number): number },
  horizontalRange: number,
  verticalRange: number,
): BlockPos {
  return new BlockPos(
    random.nextInt(2 * horizontalRange + 1) - horizontalRange,
    random.nextInt(2 * verticalRange + 1) - verticalRange,
    random.nextInt(2 * horizontalRange + 1) - horizontalRange,
  );
}

export function generateRandomPosTowardDirection(
  mob: PathfinderMob,
  radius: number,
  pos: BlockPos,
): BlockPos {
  let x = pos.getX();
  let z = pos.getZ();
  if (mob.hasRestriction() && radius > 1) {
    const center = mob.getRestrictCenter();
    x += mob.getX() > center.getX() ? -mob.getRandom().nextInt(Math.floor(radius / 2)) : mob.getRandom().nextInt(Math.floor(radius / 2));
    z += mob.getZ() > center.getZ() ? -mob.getRandom().nextInt(Math.floor(radius / 2)) : mob.getRandom().nextInt(Math.floor(radius / 2));
  }

  return new BlockPos(x + mob.getX(), pos.getY() + mob.getY(), z + mob.getZ());
}

export function moveUpOutOfSolid(pos: BlockPos, maxY: number, isSolid: (candidate: BlockPos) => boolean): BlockPos {
  if (!isSolid(pos)) {
    return pos;
  }

  let candidate = pos.above();
  while (candidate.getY() < maxY && isSolid(candidate)) {
    candidate = candidate.above();
  }
  return candidate;
}

export function generateRandomPos(
  supplier: () => BlockPos | undefined,
  scorer: (pos: BlockPos) => number,
): Vec3 | undefined {
  let bestPos: BlockPos | undefined;
  let bestScore = Number.NEGATIVE_INFINITY;

  for (let attempt = 0; attempt < RANDOM_POS_ATTEMPTS; attempt++) {
    const candidate = supplier();
    if (candidate === undefined) {
      continue;
    }

    const score = scorer(candidate);
    if (score > bestScore) {
      bestScore = score;
      bestPos = candidate;
    }
  }

  return bestPos === undefined ? undefined : new Vec3(bestPos.getX() + 0.5, bestPos.getY(), bestPos.getZ() + 0.5);
}
