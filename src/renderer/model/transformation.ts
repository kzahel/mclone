import { Matrix4f } from "../math/matrix4f";
import { Quaternion } from "../math/quaternion";
import { Vector3f } from "../math/vector3f";

export class Transformation {
  private static readonly IDENTITY = new Transformation();
  private readonly matrix: Matrix4f;

  public constructor(value?: Quaternion | Matrix4f | Vector3f | null, leftRotation?: Quaternion | null, scale?: Vector3f | null, rightRotation?: Quaternion | null) {
    if (value instanceof Matrix4f) {
      this.matrix = value.copy();
      return;
    }

    if (value instanceof Quaternion) {
      this.matrix = new Matrix4f(value);
      return;
    }

    if (value instanceof Vector3f || leftRotation !== undefined || scale !== undefined || rightRotation !== undefined || value === null) {
      this.matrix = Transformation.compose(value instanceof Vector3f ? value : null, leftRotation ?? null, scale ?? null, rightRotation ?? null);
      return;
    }

    this.matrix = new Matrix4f();
    this.matrix.setIdentity();
  }

  public static identity(): Transformation {
    return Transformation.IDENTITY;
  }

  private static compose(
    translation: Vector3f | null,
    leftRotation: Quaternion | null,
    scale: Vector3f | null,
    rightRotation: Quaternion | null,
  ): Matrix4f {
    const matrix = new Matrix4f();
    matrix.setIdentity();
    if (leftRotation !== null) {
      matrix.multiply(new Matrix4f(leftRotation));
    }

    if (scale !== null) {
      matrix.multiply(Matrix4f.createScaleMatrix(scale.x(), scale.y(), scale.z()));
    }

    if (rightRotation !== null) {
      matrix.multiply(new Matrix4f(rightRotation));
    }

    if (translation !== null) {
      matrix.m03 = translation.x();
      matrix.m13 = translation.y();
      matrix.m23 = translation.z();
    }

    return matrix;
  }

  public compose(other: Transformation): Transformation {
    const matrix = this.getMatrix();
    matrix.multiply(other.getMatrix());
    return new Transformation(matrix);
  }

  public inverse(): Transformation | undefined {
    if (this === Transformation.IDENTITY) {
      return this;
    }

    const matrix = this.getMatrix();
    return matrix.invert() ? new Transformation(matrix) : undefined;
  }

  public getMatrix(): Matrix4f {
    return this.matrix.copy();
  }

  public toString(): string {
    return `Transformation(${[
      this.matrix.m00,
      this.matrix.m01,
      this.matrix.m02,
      this.matrix.m03,
      this.matrix.m10,
      this.matrix.m11,
      this.matrix.m12,
      this.matrix.m13,
      this.matrix.m20,
      this.matrix.m21,
      this.matrix.m22,
      this.matrix.m23,
      this.matrix.m30,
      this.matrix.m31,
      this.matrix.m32,
      this.matrix.m33,
    ].join(",")})`;
  }
}
