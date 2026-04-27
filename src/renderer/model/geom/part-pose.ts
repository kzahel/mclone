export class PartPose {
  public static readonly ZERO = PartPose.offsetAndRotation(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

  private constructor(
    public readonly x: number,
    public readonly y: number,
    public readonly z: number,
    public readonly xRot: number,
    public readonly yRot: number,
    public readonly zRot: number,
  ) {}

  public static offset(x: number, y: number, z: number): PartPose {
    return PartPose.offsetAndRotation(x, y, z, 0.0, 0.0, 0.0);
  }

  public static rotation(xRot: number, yRot: number, zRot: number): PartPose {
    return PartPose.offsetAndRotation(0.0, 0.0, 0.0, xRot, yRot, zRot);
  }

  public static offsetAndRotation(x: number, y: number, z: number, xRot: number, yRot: number, zRot: number): PartPose {
    return new PartPose(x, y, z, xRot, yRot, zRot);
  }
}
