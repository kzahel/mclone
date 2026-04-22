import { Matrix3f } from "../math/matrix3f";
import { Matrix4f } from "../math/matrix4f";
import { Quaternion } from "../math/quaternion";

function fastInvCubeRoot(value: number): number {
  const buffer = new ArrayBuffer(4);
  const dataView = new DataView(buffer);
  dataView.setFloat32(0, value, true);
  let bits = dataView.getInt32(0, true);
  bits = 1_419_967_116 - Math.trunc(bits / 3);
  dataView.setInt32(0, bits, true);
  let approximation = dataView.getFloat32(0, true);
  approximation = (0.6666667 * approximation) + (1 / (((3 * approximation) * approximation) * value));
  return (0.6666667 * approximation) + (1 / (((3 * approximation) * approximation) * value));
}

export class PoseStack {
  private readonly poseStack: PoseStackPose[] = [];

  public constructor() {
    const pose = new Matrix4f();
    pose.setIdentity();
    const normal = new Matrix3f();
    normal.setIdentity();
    this.poseStack.push(new PoseStackPose(pose, normal));
  }

  public translate(x: number, y: number, z: number): void {
    this.poseStack.at(-1)!.pose().multiplyWithTranslation(x, y, z);
  }

  public scale(x: number, y: number, z: number): void {
    const pose = this.poseStack.at(-1)!;
    pose.pose().multiply(Matrix4f.createScaleMatrix(x, y, z));
    if (x === y && y === z) {
      if (x > 0) {
        return;
      }

      pose.normal().mul(-1);
    }

    const inverseX = 1 / x;
    const inverseY = 1 / y;
    const inverseZ = 1 / z;
    const scale = fastInvCubeRoot((inverseX * inverseY) * inverseZ);
    pose.normal().mul(Matrix3f.createScaleMatrix(scale * inverseX, scale * inverseY, scale * inverseZ));
  }

  public mulPose(quaternion: Quaternion): void {
    const pose = this.poseStack.at(-1)!;
    pose.pose().multiply(quaternion);
    pose.normal().mul(quaternion);
  }

  public pushPose(): void {
    const pose = this.poseStack.at(-1)!;
    this.poseStack.push(new PoseStackPose(pose.pose().copy(), pose.normal().copy()));
  }

  public popPose(): void {
    this.poseStack.pop();
  }

  public last(): PoseStackPose {
    return this.poseStack.at(-1)!;
  }

  public clear(): boolean {
    return this.poseStack.length === 1;
  }

  public setIdentity(): void {
    const pose = this.poseStack.at(-1)!;
    pose.pose().setIdentity();
    pose.normal().setIdentity();
  }

  public mulPoseMatrix(matrix: Matrix4f): void {
    this.poseStack.at(-1)!.pose().multiply(matrix);
  }
}

export class PoseStackPose {
  public constructor(
    private readonly poseMatrix: Matrix4f,
    private readonly normalMatrix: Matrix3f,
  ) {}

  public pose(): Matrix4f {
    return this.poseMatrix;
  }

  public normal(): Matrix3f {
    return this.normalMatrix;
  }
}
