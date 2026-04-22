import { positiveModulo } from "../../util/mth";
import { Vector3f } from "../math/vector3f";
import { type ModelState } from "./model-state";
import { Transformation } from "./transformation";

export class BlockModelRotation implements ModelState {
  private static readonly BY_INDEX = new Map<number, BlockModelRotation>();

  public static readonly X0_Y0 = new BlockModelRotation(0, 0);
  public static readonly X0_Y90 = new BlockModelRotation(0, 90);
  public static readonly X0_Y180 = new BlockModelRotation(0, 180);
  public static readonly X0_Y270 = new BlockModelRotation(0, 270);
  public static readonly X90_Y0 = new BlockModelRotation(90, 0);
  public static readonly X90_Y90 = new BlockModelRotation(90, 90);
  public static readonly X90_Y180 = new BlockModelRotation(90, 180);
  public static readonly X90_Y270 = new BlockModelRotation(90, 270);
  public static readonly X180_Y0 = new BlockModelRotation(180, 0);
  public static readonly X180_Y90 = new BlockModelRotation(180, 90);
  public static readonly X180_Y180 = new BlockModelRotation(180, 180);
  public static readonly X180_Y270 = new BlockModelRotation(180, 270);
  public static readonly X270_Y0 = new BlockModelRotation(270, 0);
  public static readonly X270_Y90 = new BlockModelRotation(270, 90);
  public static readonly X270_Y180 = new BlockModelRotation(270, 180);
  public static readonly X270_Y270 = new BlockModelRotation(270, 270);

  private readonly transformation: Transformation;
  private readonly index: number;

  private static getIndex(x: number, y: number): number {
    return (x * 360) + y;
  }

  private constructor(x: number, y: number) {
    this.index = BlockModelRotation.getIndex(x, y);
    const quaternion = Vector3f.YP.rotationDegrees(-y);
    quaternion.mul(Vector3f.XP.rotationDegrees(-x));
    this.transformation = new Transformation(quaternion);
    BlockModelRotation.BY_INDEX.set(this.index, this);
  }

  public getRotation(): Transformation {
    return this.transformation;
  }

  public isUvLocked(): boolean {
    return false;
  }

  public getIndexValue(): number {
    return this.index;
  }

  public static by(x: number, y: number): BlockModelRotation | undefined {
    return BlockModelRotation.BY_INDEX.get(BlockModelRotation.getIndex(positiveModulo(x, 360), positiveModulo(y, 360)));
  }
}
