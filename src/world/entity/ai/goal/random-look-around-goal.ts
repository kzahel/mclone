import { Goal, GoalFlag } from "./goal";
import type { PathfinderMob } from "../pathfinder-mob";

export class RandomLookAroundGoal extends Goal {
  private relX = 0.0;
  private relZ = 0.0;
  private lookTime = 0;

  public constructor(private readonly mob: PathfinderMob) {
    super();
    this.setFlags([GoalFlag.MOVE, GoalFlag.LOOK]);
  }

  public canUse(): boolean {
    return this.mob.getRandom().nextFloat() < 0.02;
  }

  public override canContinueToUse(): boolean {
    return this.lookTime >= 0;
  }

  public override start(): void {
    const angle = (Math.PI * 2.0) * this.mob.getRandom().nextDouble();
    this.relX = Math.cos(angle);
    this.relZ = Math.sin(angle);
    this.lookTime = 20 + this.mob.getRandom().nextInt(20);
  }

  public override tick(): void {
    this.lookTime--;
    this.mob.getLookControl().setLookAt(this.mob.getX() + this.relX, this.mob.getEyeY(), this.mob.getZ() + this.relZ);
  }
}
