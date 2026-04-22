export class Vector3f {
  public constructor(
    private readonly xValue: number,
    private readonly yValue: number,
    private readonly zValue: number,
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
}
