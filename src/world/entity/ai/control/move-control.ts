import { MobAttribute } from "../../attribute";
import { clamp } from "../../../../util/mth";
import type { PathfinderMob } from "../pathfinder-mob";

const MIN_SPEED_SQR = 2.5000003e-7;
const MAX_TURN = 90.0;

export class MoveControl {
  private wantedX = 0.0;
  private wantedY = 0.0;
  private wantedZ = 0.0;
  private speedModifier = 0.0;
  private operation: "wait" | "move_to" = "wait";

  public constructor(private readonly mob: PathfinderMob) {}

  public hasWanted(): boolean {
    return this.operation === "move_to";
  }

  public getSpeedModifier(): number {
    return this.speedModifier;
  }

  public setWantedPosition(x: number, y: number, z: number, speed: number): void {
    this.wantedX = x;
    this.wantedY = y;
    this.wantedZ = z;
    this.speedModifier = speed;
    this.operation = "move_to";
  }

  public tick(): void {
    if (this.operation !== "move_to") {
      this.mob.setZza(0.0);
      return;
    }

    this.operation = "wait";
    const dx = this.wantedX - this.mob.getX();
    const dz = this.wantedZ - this.mob.getZ();
    const dy = this.wantedY - this.mob.getY();
    const distanceSqr = (dx * dx) + (dy * dy) + (dz * dz);
    if (distanceSqr < MIN_SPEED_SQR) {
      this.mob.setZza(0.0);
      return;
    }

    const yaw = (Math.atan2(dz, dx) * 180.0 / Math.PI) - 90.0;
    this.mob.setYRot(this.rotlerp(this.mob.getYRot(), yaw, MAX_TURN));
    this.mob.setSpeed(this.speedModifier * this.mob.getAttributeValue(MobAttribute.MOVEMENT_SPEED));
    this.mob.setZza(1.0);
  }

  public getWantedX(): number {
    return this.wantedX;
  }

  public getWantedY(): number {
    return this.wantedY;
  }

  public getWantedZ(): number {
    return this.wantedZ;
  }

  private rotlerp(sourceAngle: number, targetAngle: number, maximumChange: number): number {
    let delta = wrapDegrees(targetAngle - sourceAngle);
    delta = clamp(delta, -maximumChange, maximumChange);
    let result = sourceAngle + delta;
    if (result < 0.0) {
      result += 360.0;
    } else if (result > 360.0) {
      result -= 360.0;
    }
    return result;
  }
}

function wrapDegrees(value: number): number {
  let wrapped = value % 360.0;
  if (wrapped >= 180.0) {
    wrapped -= 360.0;
  }
  if (wrapped < -180.0) {
    wrapped += 360.0;
  }
  return wrapped;
}
