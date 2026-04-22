export class Vector3f {
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

  public mul(value: number): void {
    this.xValue *= value;
    this.yValue *= value;
    this.zValue *= value;
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
