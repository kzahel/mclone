import { Quaternion } from "./quaternion";

export class Matrix4f {
  public m00 = 0;
  public m01 = 0;
  public m02 = 0;
  public m03 = 0;
  public m10 = 0;
  public m11 = 0;
  public m12 = 0;
  public m13 = 0;
  public m20 = 0;
  public m21 = 0;
  public m22 = 0;
  public m23 = 0;
  public m30 = 0;
  public m31 = 0;
  public m32 = 0;
  public m33 = 0;

  public constructor(value?: Matrix4f | Quaternion) {
    if (value instanceof Matrix4f) {
      this.load(value);
    } else if (value instanceof Quaternion) {
      const i = value.i();
      const j = value.j();
      const k = value.k();
      const r = value.r();
      const ii = 2 * i * i;
      const jj = 2 * j * j;
      const kk = 2 * k * k;
      this.m00 = 1 - jj - kk;
      this.m11 = 1 - kk - ii;
      this.m22 = 1 - ii - jj;
      this.m33 = 1;
      const ij = i * j;
      const jk = j * k;
      const ki = k * i;
      const ir = i * r;
      const jr = j * r;
      const kr = k * r;
      this.m10 = 2 * (ij + kr);
      this.m01 = 2 * (ij - kr);
      this.m20 = 2 * (ki - jr);
      this.m02 = 2 * (ki + jr);
      this.m21 = 2 * (jk + ir);
      this.m12 = 2 * (jk - ir);
    }
  }

  public load(other: Matrix4f): void {
    this.m00 = other.m00;
    this.m01 = other.m01;
    this.m02 = other.m02;
    this.m03 = other.m03;
    this.m10 = other.m10;
    this.m11 = other.m11;
    this.m12 = other.m12;
    this.m13 = other.m13;
    this.m20 = other.m20;
    this.m21 = other.m21;
    this.m22 = other.m22;
    this.m23 = other.m23;
    this.m30 = other.m30;
    this.m31 = other.m31;
    this.m32 = other.m32;
    this.m33 = other.m33;
  }

  public setIdentity(): void {
    this.m00 = 1;
    this.m01 = 0;
    this.m02 = 0;
    this.m03 = 0;
    this.m10 = 0;
    this.m11 = 1;
    this.m12 = 0;
    this.m13 = 0;
    this.m20 = 0;
    this.m21 = 0;
    this.m22 = 1;
    this.m23 = 0;
    this.m30 = 0;
    this.m31 = 0;
    this.m32 = 0;
    this.m33 = 1;
  }

  public multiply(value: Matrix4f | Quaternion | number): void {
    if (typeof value === "number") {
      this.m00 *= value;
      this.m01 *= value;
      this.m02 *= value;
      this.m03 *= value;
      this.m10 *= value;
      this.m11 *= value;
      this.m12 *= value;
      this.m13 *= value;
      this.m20 *= value;
      this.m21 *= value;
      this.m22 *= value;
      this.m23 *= value;
      this.m30 *= value;
      this.m31 *= value;
      this.m32 *= value;
      this.m33 *= value;
      return;
    }

    const other = value instanceof Quaternion ? new Matrix4f(value) : value;
    const m00 = (((this.m00 * other.m00) + (this.m01 * other.m10)) + (this.m02 * other.m20)) + (this.m03 * other.m30);
    const m01 = (((this.m00 * other.m01) + (this.m01 * other.m11)) + (this.m02 * other.m21)) + (this.m03 * other.m31);
    const m02 = (((this.m00 * other.m02) + (this.m01 * other.m12)) + (this.m02 * other.m22)) + (this.m03 * other.m32);
    const m03 = (((this.m00 * other.m03) + (this.m01 * other.m13)) + (this.m02 * other.m23)) + (this.m03 * other.m33);
    const m10 = (((this.m10 * other.m00) + (this.m11 * other.m10)) + (this.m12 * other.m20)) + (this.m13 * other.m30);
    const m11 = (((this.m10 * other.m01) + (this.m11 * other.m11)) + (this.m12 * other.m21)) + (this.m13 * other.m31);
    const m12 = (((this.m10 * other.m02) + (this.m11 * other.m12)) + (this.m12 * other.m22)) + (this.m13 * other.m32);
    const m13 = (((this.m10 * other.m03) + (this.m11 * other.m13)) + (this.m12 * other.m23)) + (this.m13 * other.m33);
    const m20 = (((this.m20 * other.m00) + (this.m21 * other.m10)) + (this.m22 * other.m20)) + (this.m23 * other.m30);
    const m21 = (((this.m20 * other.m01) + (this.m21 * other.m11)) + (this.m22 * other.m21)) + (this.m23 * other.m31);
    const m22 = (((this.m20 * other.m02) + (this.m21 * other.m12)) + (this.m22 * other.m22)) + (this.m23 * other.m32);
    const m23 = (((this.m20 * other.m03) + (this.m21 * other.m13)) + (this.m22 * other.m23)) + (this.m23 * other.m33);
    const m30 = (((this.m30 * other.m00) + (this.m31 * other.m10)) + (this.m32 * other.m20)) + (this.m33 * other.m30);
    const m31 = (((this.m30 * other.m01) + (this.m31 * other.m11)) + (this.m32 * other.m21)) + (this.m33 * other.m31);
    const m32 = (((this.m30 * other.m02) + (this.m31 * other.m12)) + (this.m32 * other.m22)) + (this.m33 * other.m32);
    const m33 = (((this.m30 * other.m03) + (this.m31 * other.m13)) + (this.m32 * other.m23)) + (this.m33 * other.m33);
    this.m00 = m00;
    this.m01 = m01;
    this.m02 = m02;
    this.m03 = m03;
    this.m10 = m10;
    this.m11 = m11;
    this.m12 = m12;
    this.m13 = m13;
    this.m20 = m20;
    this.m21 = m21;
    this.m22 = m22;
    this.m23 = m23;
    this.m30 = m30;
    this.m31 = m31;
    this.m32 = m32;
    this.m33 = m33;
  }

  public copy(): Matrix4f {
    return new Matrix4f(this);
  }

  public transpose(): void {
    let swap = this.m10;
    this.m10 = this.m01;
    this.m01 = swap;
    swap = this.m20;
    this.m20 = this.m02;
    this.m02 = swap;
    swap = this.m21;
    this.m21 = this.m12;
    this.m12 = swap;
    swap = this.m30;
    this.m30 = this.m03;
    this.m03 = swap;
    swap = this.m31;
    this.m31 = this.m13;
    this.m13 = swap;
    swap = this.m32;
    this.m32 = this.m23;
    this.m23 = swap;
  }

  public invert(): boolean {
    const augmented = [
      [this.m00, this.m01, this.m02, this.m03, 1, 0, 0, 0],
      [this.m10, this.m11, this.m12, this.m13, 0, 1, 0, 0],
      [this.m20, this.m21, this.m22, this.m23, 0, 0, 1, 0],
      [this.m30, this.m31, this.m32, this.m33, 0, 0, 0, 1],
    ];

    for (let column = 0; column < 4; column++) {
      let pivotRow = column;
      for (let row = column + 1; row < 4; row++) {
        if (Math.abs(augmented[row]![column]!) > Math.abs(augmented[pivotRow]![column]!)) {
          pivotRow = row;
        }
      }

      const pivot = augmented[pivotRow]![column]!;
      if (Math.abs(pivot) <= 1.0e-6) {
        return false;
      }

      if (pivotRow !== column) {
        const swap = augmented[column]!;
        augmented[column] = augmented[pivotRow]!;
        augmented[pivotRow] = swap;
      }

      for (let index = 0; index < 8; index++) {
        augmented[column]![index] = augmented[column]![index]! / pivot;
      }

      for (let row = 0; row < 4; row++) {
        if (row === column) {
          continue;
        }

        const factor = augmented[row]![column]!;
        if (factor === 0) {
          continue;
        }

        for (let index = 0; index < 8; index++) {
          augmented[row]![index] = augmented[row]![index]! - (factor * augmented[column]![index]!);
        }
      }
    }

    this.m00 = augmented[0]![4]!;
    this.m01 = augmented[0]![5]!;
    this.m02 = augmented[0]![6]!;
    this.m03 = augmented[0]![7]!;
    this.m10 = augmented[1]![4]!;
    this.m11 = augmented[1]![5]!;
    this.m12 = augmented[1]![6]!;
    this.m13 = augmented[1]![7]!;
    this.m20 = augmented[2]![4]!;
    this.m21 = augmented[2]![5]!;
    this.m22 = augmented[2]![6]!;
    this.m23 = augmented[2]![7]!;
    this.m30 = augmented[3]![4]!;
    this.m31 = augmented[3]![5]!;
    this.m32 = augmented[3]![6]!;
    this.m33 = augmented[3]![7]!;
    return true;
  }

  public multiplyWithTranslation(x: number, y: number, z: number): void {
    this.m03 = (((this.m00 * x) + (this.m01 * y)) + (this.m02 * z)) + this.m03;
    this.m13 = (((this.m10 * x) + (this.m11 * y)) + (this.m12 * z)) + this.m13;
    this.m23 = (((this.m20 * x) + (this.m21 * y)) + (this.m22 * z)) + this.m23;
    this.m33 = (((this.m30 * x) + (this.m31 * y)) + (this.m32 * z)) + this.m33;
  }

  public toFloat32Array(): Float32Array {
    return new Float32Array([
      this.m00,
      this.m10,
      this.m20,
      this.m30,
      this.m01,
      this.m11,
      this.m21,
      this.m31,
      this.m02,
      this.m12,
      this.m22,
      this.m32,
      this.m03,
      this.m13,
      this.m23,
      this.m33,
    ]);
  }

  public static createScaleMatrix(x: number, y: number, z: number): Matrix4f {
    const matrix = new Matrix4f();
    matrix.m00 = x;
    matrix.m11 = y;
    matrix.m22 = z;
    matrix.m33 = 1;
    return matrix;
  }

  public static perspective(fovDegrees: number, aspectRatio: number, nearPlane: number, farPlane: number): Matrix4f {
    const scale = 1.0 / Math.tan((fovDegrees * (Math.PI / 180.0)) / 2.0);
    const matrix = new Matrix4f();
    matrix.m00 = scale / aspectRatio;
    matrix.m11 = scale;
    matrix.m22 = (farPlane + nearPlane) / (nearPlane - farPlane);
    matrix.m32 = -1.0;
    matrix.m23 = ((2.0 * farPlane) * nearPlane) / (nearPlane - farPlane);
    return matrix;
  }

  public static orthographic(left: number, right: number, bottom: number, top: number, nearPlane: number, farPlane: number): Matrix4f {
    const matrix = new Matrix4f();
    const width = right - left;
    const height = bottom - top;
    const depth = farPlane - nearPlane;
    matrix.m00 = 2.0 / width;
    matrix.m11 = 2.0 / height;
    matrix.m22 = -2.0 / depth;
    matrix.m03 = -(right + left) / width;
    matrix.m13 = -(bottom + top) / height;
    matrix.m23 = -(farPlane + nearPlane) / depth;
    matrix.m33 = 1.0;
    return matrix;
  }

  public static createTranslateMatrix(x: number, y: number, z: number): Matrix4f {
    const matrix = new Matrix4f();
    matrix.setIdentity();
    matrix.m03 = x;
    matrix.m13 = y;
    matrix.m23 = z;
    return matrix;
  }
}
