import { Direction } from "../../../core/direction";
import { DiscreteVoxelShape } from "./discrete-voxel-shape";

export class BitSetDiscreteVoxelShape extends DiscreteVoxelShape {
  private readonly storage = new Set<number>();
  private xMin = this.xSize;
  private yMin = this.ySize;
  private zMin = this.zSize;
  private xMax = 0;
  private yMax = 0;
  private zMax = 0;

  public constructor(xSize: number, ySize: number, zSize: number) {
    super(xSize, ySize, zSize);
  }

  private getIndex(x: number, y: number, z: number): number {
    return ((x * this.ySize) + y) * this.zSize + z;
  }

  public override isFull(x: number, y: number, z: number): boolean {
    return this.storage.has(this.getIndex(x, y, z));
  }

  public override fill(x: number, y: number, z: number): void {
    this.storage.add(this.getIndex(x, y, z));
    this.xMin = Math.min(this.xMin, x);
    this.yMin = Math.min(this.yMin, y);
    this.zMin = Math.min(this.zMin, z);
    this.xMax = Math.max(this.xMax, x + 1);
    this.yMax = Math.max(this.yMax, y + 1);
    this.zMax = Math.max(this.zMax, z + 1);
  }

  public override firstFull(axis: (typeof Direction.Axis)["X"]): number {
    return axis.choose(this.xMin, this.yMin, this.zMin);
  }

  public override lastFull(axis: (typeof Direction.Axis)["X"]): number {
    return axis.choose(this.xMax, this.yMax, this.zMax);
  }
}
