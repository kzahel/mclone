import { generateRandomDirection, generateRandomPos, generateRandomPosTowardDirection, moveUpOutOfSolid } from "./random-pos";
import {
  hasMalus,
  isNotStable,
  isOutsideLimits,
  isRestricted,
  isSolid,
  isWater,
  mobRestricted,
} from "./goal-utils";
import type { PathfinderMob } from "../pathfinder-mob";
import type { Vec3 } from "../../../phys/vec3";

export function getLandRandomPos(
  mob: PathfinderMob,
  radius: number,
  verticalRange: number,
): Vec3 | undefined {
  const shortCircuit = mobRestricted(mob, radius);
  const level = mob.getAiLevel();

  return generateRandomPos(
    () => {
      const direction = generateRandomDirection(mob.getRandom(), radius, verticalRange);
      let pos = generateRandomPosTowardDirection(mob, radius, direction);
      if (level !== undefined) {
        pos = moveUpOutOfSolid(pos, level.getMaxBuildHeight(), (candidate) => isSolid(mob, candidate));
      }
      if (
        isOutsideLimits(pos, mob)
        || isRestricted(shortCircuit, mob, pos)
        || isNotStable(mob, pos)
        || isWater(mob, pos)
        || hasMalus(mob, pos)
      ) {
        return undefined;
      }

      return pos;
    },
    (pos) => mob.getWalkTargetValue(pos),
  );
}
