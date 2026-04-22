export class Vec3 {
  public static readonly ZERO = new Vec3(0, 0, 0);

  public constructor(
    public readonly x: number,
    public readonly y: number,
    public readonly z: number,
  ) {}
}
