export class UVPair {
  public constructor(
    private readonly uValue: number,
    private readonly vValue: number,
  ) {}

  public u(): number {
    return this.uValue;
  }

  public v(): number {
    return this.vValue;
  }

  public toString(): string {
    return `(${this.uValue},${this.vValue})`;
  }
}
