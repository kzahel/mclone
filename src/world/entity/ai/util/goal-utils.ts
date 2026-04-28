import type { BlockPos } from "../../../../core/block-pos";
import type { PathfinderMob } from "../pathfinder-mob";

export function mobRestricted(mob: PathfinderMob, radius: number): boolean {
  if (!mob.hasRestriction()) {
    return false;
  }

  const center = mob.getRestrictCenter();
  const dx = center.getX() + 0.5 - mob.getX();
  const dy = center.getY() + 0.5 - mob.getY();
  const dz = center.getZ() + 0.5 - mob.getZ();
  const distance = mob.getRestrictRadius() + radius + 1.0;
  return (dx * dx) + (dy * dy) + (dz * dz) < distance * distance;
}

export function isOutsideLimits(pos: BlockPos, mob: PathfinderMob): boolean {
  const level = mob.getAiLevel();
  if (level === undefined) {
    return false;
  }

  return pos.getY() < level.getMinBuildHeight() || pos.getY() > level.getMaxBuildHeight();
}

export function isRestricted(shortCircuit: boolean, mob: PathfinderMob, pos: BlockPos): boolean {
  return shortCircuit && !mob.isWithinRestriction(pos);
}

export function isNotStable(mob: PathfinderMob, pos: BlockPos): boolean {
  return !mob.getNavigation().isStableDestination(pos);
}

export function isWater(mob: PathfinderMob, pos: BlockPos): boolean {
  return mob.getAiLevel()?.isWater(pos) ?? false;
}

export function hasMalus(mob: PathfinderMob, pos: BlockPos): boolean {
  return mob.hasPathfindingMalus(pos);
}

export function isSolid(mob: PathfinderMob, pos: BlockPos): boolean {
  return mob.getAiLevel()?.isSolid(pos) ?? false;
}
