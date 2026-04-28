import { generateRandomDirection, generateRandomPos, generateRandomPosTowardDirection } from "./random-pos";
import {
  hasMalus,
  isNotStable,
  isOutsideLimits,
  isRestricted,
  mobRestricted,
} from "./goal-utils";
import type { PathfinderMob } from "../pathfinder-mob";
import type { Vec3 } from "../../../phys/vec3";

export function getDefaultRandomPos(
  mob: PathfinderMob,
  radius: number,
  verticalRange: number,
): Vec3 | undefined {
  const shortCircuit = mobRestricted(mob, radius);

  return generateRandomPos(
    () => {
      const direction = generateRandomDirection(mob.getRandom(), radius, verticalRange);
      const pos = generateRandomPosTowardDirection(mob, radius, direction);
      if (
        isOutsideLimits(pos, mob)
        || isRestricted(shortCircuit, mob, pos)
        || isNotStable(mob, pos)
        || hasMalus(mob, pos)
      ) {
        return undefined;
      }

      return pos;
    },
    (pos) => mob.getWalkTargetValue(pos),
  );
}
