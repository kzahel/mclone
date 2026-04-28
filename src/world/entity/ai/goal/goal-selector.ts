import { GoalFlag } from "./goal";
import type { Goal } from "./goal";
import { WrappedGoal } from "./wrapped-goal";

function goalsUseAnyFlag(left: WrappedGoal, right: WrappedGoal): boolean {
  for (const flag of left.goal.getFlags()) {
    if (right.goal.getFlags().has(flag)) {
      return true;
    }
  }
  return false;
}

export class GoalSelector {
  private readonly availableGoals: WrappedGoal[] = [];
  private readonly lockedFlags = new Map<GoalFlag, WrappedGoal>();
  private readonly disabledFlags = new Set<GoalFlag>();

  public addGoal(priority: number, goal: Goal): void {
    this.availableGoals.push(new WrappedGoal(priority, goal));
  }

  public removeGoal(goal: Goal): void {
    for (let index = this.availableGoals.length - 1; index >= 0; index--) {
      const wrapped = this.availableGoals[index]!;
      if (wrapped.goal === goal) {
        wrapped.stop();
        this.availableGoals.splice(index, 1);
      }
    }
  }

  public tick(): void {
    for (const wrappedGoal of this.availableGoals) {
      if (
        wrappedGoal.isRunning()
        && (this.goalContainsAnyFlags(wrappedGoal, this.disabledFlags) || !wrappedGoal.canContinueToUse())
      ) {
        wrappedGoal.stop();
      }
    }

    for (const [flag, wrappedGoal] of [...this.lockedFlags]) {
      if (!wrappedGoal.isRunning()) {
        this.lockedFlags.delete(flag);
      }
    }

    for (const wrappedGoal of this.availableGoals) {
      if (
        !wrappedGoal.isRunning()
        && !this.goalContainsAnyFlags(wrappedGoal, this.disabledFlags)
        && this.goalCanBeReplacedForAllFlags(wrappedGoal)
        && wrappedGoal.canUse()
      ) {
        for (const flag of wrappedGoal.goal.getFlags()) {
          const lockedGoal = this.lockedFlags.get(flag);
          lockedGoal?.stop();
          this.lockedFlags.set(flag, wrappedGoal);
        }
        wrappedGoal.start();
      }
    }

    for (const wrappedGoal of this.availableGoals) {
      wrappedGoal.tick();
    }
  }

  private goalContainsAnyFlags(goal: WrappedGoal, flags: ReadonlySet<GoalFlag>): boolean {
    for (const flag of goal.goal.getFlags()) {
      if (flags.has(flag)) {
        return true;
      }
    }
    return false;
  }

  private goalCanBeReplacedForAllFlags(goal: WrappedGoal): boolean {
    for (const availableGoal of this.availableGoals) {
      if (
        availableGoal !== goal
        && availableGoal.isRunning()
        && goalsUseAnyFlag(goal, availableGoal)
        && !availableGoal.canBeReplacedBy(goal)
      ) {
        return false;
      }
    }
    return true;
  }
}
