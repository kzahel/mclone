export class Vec3i {
  protected xValue: number;
  protected yValue: number;
  protected zValue: number;

  public constructor(
    x: number,
    y: number,
    z: number,
  ) {
    this.xValue = Math.trunc(x);
    this.yValue = Math.trunc(y);
    this.zValue = Math.trunc(z);
  }

  public getX(): number {
    return this.xValue;
  }

  public getY(): number {
    return this.yValue;
  }

  public getZ(): number {
    return this.zValue;
  }

  public equals(other: unknown): boolean {
    return other instanceof Vec3i && this.xValue === other.xValue && this.yValue === other.yValue && this.zValue === other.zValue;
  }

  public toString(): string {
    return `${this.xValue}, ${this.yValue}, ${this.zValue}`;
  }
}
