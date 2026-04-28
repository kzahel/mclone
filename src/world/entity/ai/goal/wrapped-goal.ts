import { Goal } from "./goal";

export class WrappedGoal {
  private running = false;

  public constructor(
    public readonly priority: number,
    public readonly goal: Goal,
  ) {}

  public canBeReplacedBy(other: WrappedGoal): boolean {
    return this.isInterruptable() && other.priority < this.priority;
  }

  public isInterruptable(): boolean {
    return this.goal.isInterruptable();
  }

  public canUse(): boolean {
    return this.goal.canUse();
  }

  public canContinueToUse(): boolean {
    return this.goal.canContinueToUse();
  }

  public isRunning(): boolean {
    return this.running;
  }

  public start(): void {
    if (!this.running) {
      this.running = true;
      this.goal.start();
    }
  }

  public stop(): void {
    if (this.running) {
      this.running = false;
      this.goal.stop();
    }
  }

  public tick(): void {
    if (this.running) {
      this.goal.tick();
    }
  }
}
