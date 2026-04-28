import { getDefaultRandomPos } from "../util/default-random-pos";
import type { PathfinderMob } from "../pathfinder-mob";
import { Goal, GoalFlag } from "./goal";
import type { Vec3 } from "../../../phys/vec3";

export const DEFAULT_RANDOM_STROLL_INTERVAL = 120;

export class RandomStrollGoal extends Goal {
  protected wantedX = 0.0;
  protected wantedY = 0.0;
  protected wantedZ = 0.0;
  protected interval: number;
  protected forceTrigger = false;

  public constructor(
    protected readonly mob: PathfinderMob,
    protected readonly speedModifier: number,
    interval = DEFAULT_RANDOM_STROLL_INTERVAL,
    private readonly checkNoActionTime = true,
  ) {
    super();
    this.interval = interval;
    this.setFlags([GoalFlag.MOVE]);
  }

  public canUse(): boolean {
    if (this.mob.isVehicle()) {
      return false;
    }

    if (!this.forceTrigger) {
      if (this.checkNoActionTime && this.mob.getNoActionTime() >= 100) {
        return false;
      }

      if (this.mob.getRandom().nextInt(this.interval) !== 0) {
        return false;
      }
    }

    const position = this.getPosition();
    if (position === undefined) {
      return false;
    }

    this.wantedX = position.x;
    this.wantedY = position.y;
    this.wantedZ = position.z;
    this.forceTrigger = false;
    return true;
  }

  protected getPosition(): Vec3 | undefined {
    return getDefaultRandomPos(this.mob, 10, 7);
  }

  public override canContinueToUse(): boolean {
    return !this.mob.getNavigation().isDone() && !this.mob.isVehicle();
  }

  public override start(): void {
    this.mob.getNavigation().moveTo(this.wantedX, this.wantedY, this.wantedZ, this.speedModifier);
  }

  public override stop(): void {
    this.mob.getNavigation().stop();
    super.stop();
  }

  public trigger(): void {
    this.forceTrigger = true;
  }

  public setInterval(newChance: number): void {
    this.interval = newChance;
  }
}
