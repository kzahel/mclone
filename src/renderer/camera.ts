import { BlockPos } from "../core/block-pos";
import { Matrix3f } from "./math/matrix3f";
import { Quaternion } from "./math/quaternion";
import { Vector3f } from "./math/vector3f";
import { FogType } from "../world/level/material/fog-type";
import { Vec3 } from "../world/phys/vec3";

export class Camera {
  private initialized = false;
  private positionValue = Vec3.ZERO;
  private readonly blockPositionValue = new BlockPos.MutableBlockPos();
  private readonly forwards = new Vector3f(0.0, 0.0, 1.0);
  private readonly up = new Vector3f(0.0, 1.0, 0.0);
  private readonly left = new Vector3f(1.0, 0.0, 0.0);
  private xRotValue = 0;
  private yRotValue = 0;
  private rotationValue = Quaternion.ONE.copy();
  private detached = false;

  // WebGPU: the smoke harness drives camera transforms directly instead of binding the camera to a client Entity.
  public setup(position: Vec3, xRot: number, yRot: number): void {
    this.initialized = true;
    this.detached = false;
    this.setRotation(yRot, xRot);
    this.setPosition(position);
  }

  protected setRotation(yRot: number, xRot: number): void {
    this.xRotValue = xRot;
    this.yRotValue = yRot;
    this.rotationValue = new Quaternion(0.0, 0.0, 0.0, 1.0);
    this.rotationValue.mul(Vector3f.YP.rotationDegrees(-yRot));
    this.rotationValue.mul(Vector3f.XP.rotationDegrees(xRot));
    const rotationMatrix = new Matrix3f(this.rotationValue);
    this.forwards.set(0.0, 0.0, 1.0);
    this.forwards.transform(rotationMatrix);
    this.up.set(0.0, 1.0, 0.0);
    this.up.transform(rotationMatrix);
    this.left.set(1.0, 0.0, 0.0);
    this.left.transform(rotationMatrix);
  }

  protected setPosition(position: Vec3): void {
    this.positionValue = position;
    this.blockPositionValue.set(position.x, position.y, position.z);
  }

  public getPosition(): Vec3 {
    return this.positionValue;
  }

  public getBlockPosition(): BlockPos {
    return this.blockPositionValue;
  }

  public getXRot(): number {
    return this.xRotValue;
  }

  public getYRot(): number {
    return this.yRotValue;
  }

  public rotation(): Quaternion {
    return this.rotationValue;
  }

  public isInitialized(): boolean {
    return this.initialized;
  }

  public isDetached(): boolean {
    return this.detached;
  }

  public getFluidInCamera(): FogType {
    return FogType.NONE;
  }

  public getLookVector(): Vector3f {
    return this.forwards;
  }

  public reset(): void {
    this.initialized = false;
    this.positionValue = Vec3.ZERO;
  }
}
