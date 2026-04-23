import { Matrix3f } from "./matrix3f";
import { Quaternion } from "./quaternion";

export class Vector3f {
  public static readonly XP = new Vector3f(1.0, 0.0, 0.0);
  public static readonly YP = new Vector3f(0.0, 1.0, 0.0);
  public static readonly ZP = new Vector3f(0.0, 0.0, 1.0);

  public constructor(
    private xValue = 0,
    private yValue = 0,
    private zValue = 0,
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

  public copy(): Vector3f {
    return new Vector3f(this.xValue, this.yValue, this.zValue);
  }

  public set(x: number, y: number, z: number): void {
    this.xValue = x;
    this.yValue = y;
    this.zValue = z;
  }

  public mul(value: number): void {
    this.xValue *= value;
    this.yValue *= value;
    this.zValue *= value;
  }

  public add(x: number, y: number, z: number): void {
    this.xValue += x;
    this.yValue += y;
    this.zValue += z;
  }

  public lerp(other: Vector3f, delta: number): void {
    this.xValue += (other.xValue - this.xValue) * delta;
    this.yValue += (other.yValue - this.yValue) * delta;
    this.zValue += (other.zValue - this.zValue) * delta;
  }

  public map(mapper: (value: number) => number): void {
    this.xValue = mapper(this.xValue);
    this.yValue = mapper(this.yValue);
    this.zValue = mapper(this.zValue);
  }

  public sub(other: Vector3f): void {
    this.xValue -= other.xValue;
    this.yValue -= other.yValue;
    this.zValue -= other.zValue;
  }

  public cross(other: Vector3f): void {
    const x = (this.yValue * other.zValue) - (this.zValue * other.yValue);
    const y = (this.zValue * other.xValue) - (this.xValue * other.zValue);
    const z = (this.xValue * other.yValue) - (this.yValue * other.xValue);
    this.xValue = x;
    this.yValue = y;
    this.zValue = z;
  }

  public dot(other: Vector3f): number {
    return (this.xValue * other.xValue) + (this.yValue * other.yValue) + (this.zValue * other.zValue);
  }

  public normalize(): void {
    const length = Math.sqrt(this.dot(this));
    if (length < 1.0e-6) {
      return;
    }

    this.xValue /= length;
    this.yValue /= length;
    this.zValue /= length;
  }

  public transform(matrix: Matrix3f): void {
    const x = this.xValue;
    const y = this.yValue;
    const z = this.zValue;
    this.xValue = ((matrix.m00 * x) + (matrix.m01 * y)) + (matrix.m02 * z);
    this.yValue = ((matrix.m10 * x) + (matrix.m11 * y)) + (matrix.m12 * z);
    this.zValue = ((matrix.m20 * x) + (matrix.m21 * y)) + (matrix.m22 * z);
  }

  public rotationDegrees(degrees: number): Quaternion {
    const radians = degrees * (Math.PI / 180.0);
    const sine = Math.sin(radians / 2.0);
    const cosine = Math.cos(radians / 2.0);
    return new Quaternion(this.xValue * sine, this.yValue * sine, this.zValue * sine, cosine);
  }

  public clamp(min: number, max: number): void {
    this.xValue = Math.max(min, Math.min(max, this.xValue));
    this.yValue = Math.max(min, Math.min(max, this.yValue));
    this.zValue = Math.max(min, Math.min(max, this.zValue));
  }

  public equals(other: unknown): boolean {
    return other instanceof Vector3f && this.xValue === other.xValue && this.yValue === other.yValue && this.zValue === other.zValue;
  }

  public toString(): string {
    return `[${this.xValue}, ${this.yValue}, ${this.zValue}]`;
  }
}
