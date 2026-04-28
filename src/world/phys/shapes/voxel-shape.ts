import { Direction } from "../../../core/direction";
import { AABB } from "../aabb";

const EPSILON = 1.0e-7;

function nearlyEqual(left: number, right: number): boolean {
  return Math.abs(left - right) <= EPSILON;
}

function axisMax(box: AABB, axis: Direction.Axis): number {
  switch (axis) {
    case Direction.Axis.X:
      return box.maxX;
    case Direction.Axis.Y:
      return box.maxY;
    case Direction.Axis.Z:
    default:
      return box.maxZ;
  }
}

export class VoxelShape {
  public constructor(private readonly boxes: readonly AABB[]) {}

  public isEmpty(): boolean {
    return this.boxes.length === 0;
  }

  public toAabbs(): readonly AABB[] {
    return this.boxes;
  }

  public max(axis: Direction.Axis): number {
    let max = 0.0;
    for (const box of this.boxes) {
      max = Math.max(max, axisMax(box, axis));
    }
    return max;
  }

  public isFullBlock(): boolean {
    return this.boxes.length === 1 && isUnitBox(this.boxes[0]!);
  }
}

function isUnitBox(box: AABB): boolean {
  return nearlyEqual(box.minX, 0.0)
    && nearlyEqual(box.minY, 0.0)
    && nearlyEqual(box.minZ, 0.0)
    && nearlyEqual(box.maxX, 1.0)
    && nearlyEqual(box.maxY, 1.0)
    && nearlyEqual(box.maxZ, 1.0);
}

const EMPTY_SHAPE = new VoxelShape([]);
const BLOCK_SHAPE = new VoxelShape([new AABB(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)]);

export const Shapes = {
  empty(): VoxelShape {
    return EMPTY_SHAPE;
  },

  block(): VoxelShape {
    return BLOCK_SHAPE;
  },

  box(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number): VoxelShape {
    if (maxX <= minX || maxY <= minY || maxZ <= minZ) {
      return EMPTY_SHAPE;
    }
    return new VoxelShape([new AABB(minX, minY, minZ, maxX, maxY, maxZ)]);
  },
};
