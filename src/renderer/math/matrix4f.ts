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
}
