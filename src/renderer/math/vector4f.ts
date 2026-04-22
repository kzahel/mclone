import { Matrix4f } from "./matrix4f";

export class Vector4f {
  public constructor(
    private xValue = 0,
    private yValue = 0,
    private zValue = 0,
    private wValue = 0,
  ) {}

  public x(): number {
    return this.xValue;
  }

  public y(): number {
    return this.yValue;
  }

  public z(): number {
    return this.zValue;
  }

  public w(): number {
    return this.wValue;
  }

  public mul(value: number): void {
    this.xValue *= value;
    this.yValue *= value;
    this.zValue *= value;
    this.wValue *= value;
  }

  public set(x: number, y: number, z: number, w: number): void {
    this.xValue = x;
    this.yValue = y;
    this.zValue = z;
    this.wValue = w;
  }

  public transform(matrix: Matrix4f): void {
    const x = this.xValue;
    const y = this.yValue;
    const z = this.zValue;
    const w = this.wValue;
    this.xValue = (((matrix.m00 * x) + (matrix.m01 * y)) + (matrix.m02 * z)) + (matrix.m03 * w);
    this.yValue = (((matrix.m10 * x) + (matrix.m11 * y)) + (matrix.m12 * z)) + (matrix.m13 * w);
    this.zValue = (((matrix.m20 * x) + (matrix.m21 * y)) + (matrix.m22 * z)) + (matrix.m23 * w);
    this.wValue = (((matrix.m30 * x) + (matrix.m31 * y)) + (matrix.m32 * z)) + (matrix.m33 * w);
  }
}
