import { BlockPos } from "../../../../core/block-pos";
import type { MobNavigation, PathfinderMob } from "../pathfinder-mob";

const WAYPOINT_REACHED_DISTANCE_SQR = 0.04;

export class SimpleGroundPathNavigation implements MobNavigation {
  private target: { readonly x: number; readonly y: number; readonly z: number } | undefined;
  private speedModifier = 0.0;

  public constructor(private readonly mob: PathfinderMob) {}

  public isDone(): boolean {
    return this.target === undefined;
  }

  public isStableDestination(pos: BlockPos): boolean {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return true;
    }

    return level.isStableDestination(pos);
  }

  public moveTo(x: number, y: number, z: number, speed: number): boolean {
    const targetPos = new BlockPos(x, y, z);
    if (!this.isStableDestination(targetPos)) {
      this.target = undefined;
      return false;
    }

    this.target = { x, y, z };
    this.speedModifier = speed;
    return true;
  }

  public stop(): void {
    this.target = undefined;
  }

  public tick(): void {
    if (this.target === undefined) {
      return;
    }

    const dx = this.target.x - this.mob.getX();
    const dz = this.target.z - this.mob.getZ();
    if ((dx * dx) + (dz * dz) <= WAYPOINT_REACHED_DISTANCE_SQR) {
      this.target = undefined;
      return;
    }

    // Runtime: direct waypoint steering until vanilla PathFinder/WalkNodeEvaluator are ported.
    this.mob.getMoveControl().setWantedPosition(this.target.x, this.target.y, this.target.z, this.speedModifier);
  }

  public getTarget(): { readonly x: number; readonly y: number; readonly z: number } | undefined {
    return this.target;
  }
}
