export class NoiseSlideSettings {
  public constructor(
    private readonly targetValue: number,
    private readonly sizeValue: number,
    private readonly offsetValue: number,
  ) {}

  public target(): number {
    return this.targetValue;
  }

  public size(): number {
    return this.sizeValue;
  }

  public offset(): number {
    return this.offsetValue;
  }
}
