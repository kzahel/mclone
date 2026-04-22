export class NoiseSamplingSettings {
  public constructor(
    private readonly xzScaleValue: number,
    private readonly yScaleValue: number,
    private readonly xzFactorValue: number,
    private readonly yFactorValue: number,
  ) {}

  public xzScale(): number {
    return this.xzScaleValue;
  }

  public yScale(): number {
    return this.yScaleValue;
  }

  public xzFactor(): number {
    return this.xzFactorValue;
  }

  public yFactor(): number {
    return this.yFactorValue;
  }
}
