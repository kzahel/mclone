import { Goal, GoalFlag } from "./goal";
import type { MobLookTarget, PathfinderMob } from "../pathfinder-mob";

const DEFAULT_PROBABILITY = 0.02;

export class LookAtPlayerGoal extends Goal {
  private lookAt: MobLookTarget | undefined;
  private lookTime = 0;

  public constructor(
    private readonly mob: PathfinderMob,
    private readonly lookDistance: number,
    private readonly probability = DEFAULT_PROBABILITY,
    private readonly onlyHorizontal = false,
  ) {
    super();
    this.setFlags([GoalFlag.LOOK]);
  }

  public canUse(): boolean {
    if (this.mob.getRandom().nextFloat() >= this.probability) {
      return false;
    }

    // Runtime: MobAiLevel nearest-player bridge replaces vanilla TargetingConditions until players are entities.
    this.lookAt = this.mob.getAiLevel()?.getNearestPlayer?.(
      this.mob.getX(),
      this.mob.getEyeY(),
      this.mob.getZ(),
      this.lookDistance,
    );
    return this.lookAt !== undefined;
  }

  public override canContinueToUse(): boolean {
    if (this.lookAt === undefined || !this.lookAt.isAlive()) {
      return false;
    }

    const dx = this.lookAt.getX() - this.mob.getX();
    const dy = this.lookAt.getY() - this.mob.getY();
    const dz = this.lookAt.getZ() - this.mob.getZ();
    return ((dx * dx) + (dy * dy) + (dz * dz)) <= (this.lookDistance * this.lookDistance) && this.lookTime > 0;
  }

  public override start(): void {
    this.lookTime = 40 + this.mob.getRandom().nextInt(40);
  }

  public override stop(): void {
    this.lookAt = undefined;
  }

  public override tick(): void {
    if (this.lookAt === undefined) {
      return;
    }

    const y = this.onlyHorizontal ? this.mob.getEyeY() : this.lookAt.getEyeY();
    this.mob.getLookControl().setLookAt(this.lookAt.getX(), y, this.lookAt.getZ());
    this.lookTime--;
  }
}
