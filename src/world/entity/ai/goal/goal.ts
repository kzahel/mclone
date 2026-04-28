export const GoalFlag = {
  MOVE: "move",
  LOOK: "look",
  JUMP: "jump",
  TARGET: "target",
} as const;

export type GoalFlag = typeof GoalFlag[keyof typeof GoalFlag];

export abstract class Goal {
  private flags = new Set<GoalFlag>();

  public abstract canUse(): boolean;

  public canContinueToUse(): boolean {
    return this.canUse();
  }

  public isInterruptable(): boolean {
    return true;
  }

  public start(): void {}

  public stop(): void {}

  public tick(): void {}

  public setFlags(flags: Iterable<GoalFlag>): void {
    this.flags = new Set(flags);
  }

  public getFlags(): ReadonlySet<GoalFlag> {
    return this.flags;
  }
}
