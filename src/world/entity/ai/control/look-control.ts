import { clamp } from "../../../../util/mth";
import type { PathfinderMob } from "../pathfinder-mob";

const EPSILON = 1.0e-5;
const RETURN_HEAD_TO_BODY_SPEED = 10.0;

export class LookControl {
  protected yMaxRotSpeed = 0.0;
  protected xMaxRotAngle = 0.0;
  protected hasWanted = false;
  protected wantedX = 0.0;
  protected wantedY = 0.0;
  protected wantedZ = 0.0;

  public constructor(protected readonly mob: PathfinderMob) {}

  public setLookAt(x: number, y: number, z: number, deltaYaw = this.mob.getHeadRotSpeed(), deltaPitch = this.mob.getMaxHeadXRot()): void {
    this.wantedX = x;
    this.wantedY = y;
    this.wantedZ = z;
    this.yMaxRotSpeed = deltaYaw;
    this.xMaxRotAngle = deltaPitch;
    this.hasWanted = true;
  }

  public tick(): void {
    if (this.resetXRotOnTick()) {
      this.mob.setXRot(0.0);
    }

    if (this.hasWanted) {
      this.hasWanted = false;
      const yRot = this.getYRotD();
      if (yRot !== undefined) {
        this.mob.setYHeadRot(this.rotateTowards(this.mob.getYHeadRot(), yRot, this.yMaxRotSpeed));
      }

      const xRot = this.getXRotD();
      if (xRot !== undefined) {
        this.mob.setXRot(this.rotateTowards(this.mob.getXRot(), xRot, this.xMaxRotAngle));
      }
    } else {
      this.mob.setYHeadRot(this.rotateTowards(this.mob.getYHeadRot(), this.mob.getYBodyRot(), RETURN_HEAD_TO_BODY_SPEED));
    }

    this.clampHeadRotationToBody();
  }

  protected clampHeadRotationToBody(): void {
    if (!this.mob.getNavigation().isDone()) {
      this.mob.setYHeadRot(rotateIfNecessary(this.mob.getYHeadRot(), this.mob.getYBodyRot(), this.mob.getMaxHeadYRot()));
    }
  }

  protected resetXRotOnTick(): boolean {
    return true;
  }

  public isHasWanted(): boolean {
    return this.hasWanted;
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

  protected getXRotD(): number | undefined {
    const dx = this.wantedX - this.mob.getX();
    const dy = this.wantedY - this.mob.getEyeY();
    const dz = this.wantedZ - this.mob.getZ();
    const horizontalDistance = Math.sqrt((dx * dx) + (dz * dz));
    if (!(Math.abs(dy) > EPSILON) && !(Math.abs(horizontalDistance) > EPSILON)) {
      return undefined;
    }

    return -(Math.atan2(dy, horizontalDistance) * 180.0 / Math.PI);
  }

  protected getYRotD(): number | undefined {
    const dx = this.wantedX - this.mob.getX();
    const dz = this.wantedZ - this.mob.getZ();
    if (!(Math.abs(dz) > EPSILON) && !(Math.abs(dx) > EPSILON)) {
      return undefined;
    }

    return (Math.atan2(dz, dx) * 180.0 / Math.PI) - 90.0;
  }

  protected rotateTowards(from: number, to: number, maxDelta: number): number {
    const degrees = degreesDifference(from, to);
    const delta = clamp(degrees, -maxDelta, maxDelta);
    return from + delta;
  }
}

function rotateIfNecessary(from: number, to: number, maxDelta: number): number {
  const degrees = degreesDifference(from, to);
  const delta = clamp(degrees, -maxDelta, maxDelta);
  return to - delta;
}

function degreesDifference(from: number, to: number): number {
  return wrapDegrees(to - from);
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
