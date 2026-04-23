import { AxisCycle } from "../../../core/axis-cycle";
import { Direction } from "../../../core/direction";

export type IntFaceConsumer = (direction: Direction, x: number, y: number, z: number) => void;

export abstract class DiscreteVoxelShape {
  protected constructor(
    protected readonly xSize: number,
    protected readonly ySize: number,
    protected readonly zSize: number,
  ) {
    if (xSize < 0 || ySize < 0 || zSize < 0) {
      throw new Error(`Need all positive sizes: x=${xSize}, y=${ySize}, z=${zSize}`);
    }
  }

  public isFullWide(x: number, y: number, z: number): boolean {
    if (x < 0 || y < 0 || z < 0) {
      return false;
    }

    return x < this.xSize && y < this.ySize && z < this.zSize ? this.isFull(x, y, z) : false;
  }

  public abstract isFull(x: number, y: number, z: number): boolean;

  public abstract fill(x: number, y: number, z: number): void;

  public abstract firstFull(axis: (typeof Direction.Axis)["X"]): number;

  public abstract lastFull(axis: (typeof Direction.Axis)["X"]): number;

  public getSize(axis: (typeof Direction.Axis)["X"]): number {
    return axis.choose(this.xSize, this.ySize, this.zSize);
  }

  public forAllFaces(consumer: IntFaceConsumer): void {
    this.forAllAxisFaces(consumer, AxisCycle.NONE);
    this.forAllAxisFaces(consumer, AxisCycle.FORWARD);
    this.forAllAxisFaces(consumer, AxisCycle.BACKWARD);
  }

  private forAllAxisFaces(consumer: IntFaceConsumer, cycle: ReturnType<typeof AxisCycle.between> | typeof AxisCycle.NONE): void {
    const inverse = cycle.inverse();
    const axis = inverse.cycleAxis(Direction.Axis.Z);
    const sizeX = this.getSize(inverse.cycleAxis(Direction.Axis.X));
    const sizeY = this.getSize(inverse.cycleAxis(Direction.Axis.Y));
    const sizeZ = this.getSize(axis);
    const negative = Direction.fromAxisAndDirection(axis, Direction.AxisDirection.NEGATIVE);
    const positive = Direction.fromAxisAndDirection(axis, Direction.AxisDirection.POSITIVE);

    for (let x = 0; x < sizeX; x++) {
      for (let y = 0; y < sizeY; y++) {
        let previousFull = false;

        for (let z = 0; z <= sizeZ; z++) {
          const currentFull =
            z !== sizeZ &&
            this.isFull(
              inverse.cycle(x, y, z, Direction.Axis.X),
              inverse.cycle(x, y, z, Direction.Axis.Y),
              inverse.cycle(x, y, z, Direction.Axis.Z),
            );
          if (!previousFull && currentFull) {
            consumer(
              negative,
              inverse.cycle(x, y, z, Direction.Axis.X),
              inverse.cycle(x, y, z, Direction.Axis.Y),
              inverse.cycle(x, y, z, Direction.Axis.Z),
            );
          }

          if (previousFull && !currentFull) {
            consumer(
              positive,
              inverse.cycle(x, y, z - 1, Direction.Axis.X),
              inverse.cycle(x, y, z - 1, Direction.Axis.Y),
              inverse.cycle(x, y, z - 1, Direction.Axis.Z),
            );
          }

          previousFull = currentFull;
        }
      }
    }
  }
}
