import { Matrix4f } from "./matrix4f";
import { Quaternion } from "./quaternion";

export class Matrix3f {
  public m00 = 0;
  public m01 = 0;
  public m02 = 0;
  public m10 = 0;
  public m11 = 0;
  public m12 = 0;
  public m20 = 0;
  public m21 = 0;
  public m22 = 0;

  public constructor(value?: Matrix3f | Matrix4f | Quaternion) {
    if (value instanceof Matrix3f) {
      this.load(value);
      return;
    }

    if (value instanceof Matrix4f) {
      this.m00 = value.m00;
      this.m01 = value.m01;
      this.m02 = value.m02;
      this.m10 = value.m10;
      this.m11 = value.m11;
      this.m12 = value.m12;
      this.m20 = value.m20;
      this.m21 = value.m21;
      this.m22 = value.m22;
      return;
    }

    if (value instanceof Quaternion) {
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

  public load(other: Matrix3f): void {
    this.m00 = other.m00;
    this.m01 = other.m01;
    this.m02 = other.m02;
    this.m10 = other.m10;
    this.m11 = other.m11;
    this.m12 = other.m12;
    this.m20 = other.m20;
    this.m21 = other.m21;
    this.m22 = other.m22;
  }

  public setIdentity(): void {
    this.m00 = 1;
    this.m01 = 0;
    this.m02 = 0;
    this.m10 = 0;
    this.m11 = 1;
    this.m12 = 0;
    this.m20 = 0;
    this.m21 = 0;
    this.m22 = 1;
  }

  public mul(value: Matrix3f | Quaternion | number): void {
    if (typeof value === "number") {
      this.m00 *= value;
      this.m01 *= value;
      this.m02 *= value;
      this.m10 *= value;
      this.m11 *= value;
      this.m12 *= value;
      this.m20 *= value;
      this.m21 *= value;
      this.m22 *= value;
      return;
    }

    const other = value instanceof Quaternion ? new Matrix3f(value) : value;
    const m00 = ((this.m00 * other.m00) + (this.m01 * other.m10)) + (this.m02 * other.m20);
    const m01 = ((this.m00 * other.m01) + (this.m01 * other.m11)) + (this.m02 * other.m21);
    const m02 = ((this.m00 * other.m02) + (this.m01 * other.m12)) + (this.m02 * other.m22);
    const m10 = ((this.m10 * other.m00) + (this.m11 * other.m10)) + (this.m12 * other.m20);
    const m11 = ((this.m10 * other.m01) + (this.m11 * other.m11)) + (this.m12 * other.m21);
    const m12 = ((this.m10 * other.m02) + (this.m11 * other.m12)) + (this.m12 * other.m22);
    const m20 = ((this.m20 * other.m00) + (this.m21 * other.m10)) + (this.m22 * other.m20);
    const m21 = ((this.m20 * other.m01) + (this.m21 * other.m11)) + (this.m22 * other.m21);
    const m22 = ((this.m20 * other.m02) + (this.m21 * other.m12)) + (this.m22 * other.m22);
    this.m00 = m00;
    this.m01 = m01;
    this.m02 = m02;
    this.m10 = m10;
    this.m11 = m11;
    this.m12 = m12;
    this.m20 = m20;
    this.m21 = m21;
    this.m22 = m22;
  }

  public copy(): Matrix3f {
    return new Matrix3f(this);
  }

  public toFloat32Array(): Float32Array {
    return new Float32Array([
      this.m00,
      this.m10,
      this.m20,
      this.m01,
      this.m11,
      this.m21,
      this.m02,
      this.m12,
      this.m22,
    ]);
  }

  public static createScaleMatrix(x: number, y: number, z: number): Matrix3f {
    const matrix = new Matrix3f();
    matrix.m00 = x;
    matrix.m11 = y;
    matrix.m22 = z;
    return matrix;
  }
}
