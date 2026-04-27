export class CubeDeformation {
  public static readonly NONE = new CubeDeformation(0.0);

  public readonly growX: number;
  public readonly growY: number;
  public readonly growZ: number;

  public constructor(grow: number);
  public constructor(growX: number, growY: number, growZ: number);
  public constructor(growX: number, growY?: number, growZ?: number) {
    this.growX = growX;
    this.growY = growY ?? growX;
    this.growZ = growZ ?? growX;
  }

  public extend(grow: number): CubeDeformation;
  public extend(growX: number, growY: number, growZ: number): CubeDeformation;
  public extend(growX: number, growY?: number, growZ?: number): CubeDeformation {
    return new CubeDeformation(this.growX + growX, this.growY + (growY ?? growX), this.growZ + (growZ ?? growX));
  }
}
