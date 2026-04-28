import { getLandRandomPos } from "../util/land-random-pos";
import { RandomStrollGoal } from "./random-stroll-goal";
import type { Vec3 } from "../../../phys/vec3";
import type { PathfinderMob } from "../pathfinder-mob";

const PROBABILITY = 0.001;

export class WaterAvoidingRandomStrollGoal extends RandomStrollGoal {
  public constructor(
    mob: PathfinderMob,
    speedModifier: number,
    private readonly probability = PROBABILITY,
  ) {
    super(mob, speedModifier);
  }

  protected override getPosition(): Vec3 | undefined {
    if (this.mob.isInWaterOrBubble()) {
      return getLandRandomPos(this.mob, 15, 7) ?? super.getPosition();
    }

    return this.mob.getRandom().nextFloat() >= this.probability
      ? getLandRandomPos(this.mob, 10, 7)
      : super.getPosition();
  }
}
