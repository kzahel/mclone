import { Matrix4f } from "../math/matrix4f";
import { Quaternion } from "../math/quaternion";

export class Transformation {
  private static readonly IDENTITY = new Transformation();
  private readonly matrix: Matrix4f;

  public constructor(rotation?: Quaternion | Matrix4f | null) {
    if (rotation instanceof Matrix4f) {
      this.matrix = rotation.copy();
      return;
    }

    if (rotation instanceof Quaternion) {
      this.matrix = new Matrix4f(rotation);
      return;
    }

    this.matrix = new Matrix4f();
    this.matrix.setIdentity();
  }

  public static identity(): Transformation {
    return Transformation.IDENTITY;
  }

  public getMatrix(): Matrix4f {
    return this.matrix;
  }
}
