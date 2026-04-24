import { Vec3i } from "./vec3i";
import { Direction } from "./direction";

const PACKED_X_LENGTH = 1 + 25;
const PACKED_Z_LENGTH = PACKED_X_LENGTH;
const PACKED_Y_LENGTH = 64 - PACKED_X_LENGTH - PACKED_Z_LENGTH;
const PACKED_X_MASK = (1n << BigInt(PACKED_X_LENGTH)) - 1n;
const PACKED_Y_MASK = (1n << BigInt(PACKED_Y_LENGTH)) - 1n;
const PACKED_Z_MASK = (1n << BigInt(PACKED_Z_LENGTH)) - 1n;
const Z_OFFSET = PACKED_Y_LENGTH;
const X_OFFSET = PACKED_Y_LENGTH + PACKED_Z_LENGTH;

function unpackSigned(value: bigint, leftShift: number, rightShift: number): number {
  return Number(BigInt.asIntN(64, value << BigInt(leftShift)) >> BigInt(rightShift));
}

export class BlockPos extends Vec3i {
  public static readonly ZERO = new BlockPos(0, 0, 0);

  public constructor(x: number, y: number, z: number) {
    super(x, y, z);
  }

  public asLong(): bigint {
    return BlockPos.asLong(this.getX(), this.getY(), this.getZ());
  }

  public static asLong(x: number, y: number, z: number): bigint {
    let packed = 0n;
    packed |= (BigInt(x) & PACKED_X_MASK) << BigInt(X_OFFSET);
    packed |= BigInt(y) & PACKED_Y_MASK;
    packed |= (BigInt(z) & PACKED_Z_MASK) << BigInt(Z_OFFSET);
    return BigInt.asIntN(64, packed);
  }

  public static getX(value: bigint): number {
    return unpackSigned(value, 64 - X_OFFSET - PACKED_X_LENGTH, 64 - PACKED_X_LENGTH);
  }

  public static getY(value: bigint): number {
    return unpackSigned(value, 64 - PACKED_Y_LENGTH, 64 - PACKED_Y_LENGTH);
  }

  public static getZ(value: bigint): number {
    return unpackSigned(value, 64 - Z_OFFSET - PACKED_Z_LENGTH, 64 - PACKED_Z_LENGTH);
  }

  public static of(value: bigint): BlockPos {
    return new BlockPos(BlockPos.getX(value), BlockPos.getY(value), BlockPos.getZ(value));
  }

  public static offset(value: bigint, direction: Direction): bigint;
  public static offset(value: bigint, x: number, y: number, z: number): bigint;
  public static offset(value: bigint, second: Direction | number, third?: number, fourth?: number): bigint {
    if (second instanceof Direction) {
      return BlockPos.offset(value, second.getStepX(), second.getStepY(), second.getStepZ());
    }

    return BlockPos.asLong(
      BlockPos.getX(value) + second,
      BlockPos.getY(value) + (third ?? 0),
      BlockPos.getZ(value) + (fourth ?? 0),
    );
  }

  public static getFlatIndex(value: bigint): bigint {
    return value & -16n;
  }

  public offset(x: number, y: number, z: number): BlockPos {
    return x === 0 && y === 0 && z === 0 ? this : new BlockPos(this.getX() + x, this.getY() + y, this.getZ() + z);
  }

  public relative(direction: Direction, amount = 1): BlockPos {
    return amount === 0
      ? this
      : new BlockPos(
          this.getX() + (direction.getStepX() * amount),
          this.getY() + (direction.getStepY() * amount),
          this.getZ() + (direction.getStepZ() * amount),
        );
  }

  public below(amount = 1): BlockPos {
    return this.relative(Direction.DOWN, amount);
  }

  public above(amount = 1): BlockPos {
    return this.relative(Direction.UP, amount);
  }

  public north(): BlockPos {
    return this.relative(Direction.NORTH);
  }

  public south(): BlockPos {
    return this.relative(Direction.SOUTH);
  }

  public west(): BlockPos {
    return this.relative(Direction.WEST);
  }

  public east(): BlockPos {
    return this.relative(Direction.EAST);
  }

  public mutable(): BlockPos.MutableBlockPos {
    return new BlockPos.MutableBlockPos(this.getX(), this.getY(), this.getZ());
  }
}

export namespace BlockPos {
  export class MutableBlockPos extends BlockPos {
    public constructor(x = 0, y = 0, z = 0) {
      super(x, y, z);
    }

    public set(x: number, y: number, z: number): MutableBlockPos {
      this.xValue = Math.trunc(x);
      this.yValue = Math.trunc(y);
      this.zValue = Math.trunc(z);
      return this;
    }

    public setWithOffset(pos: BlockPos, direction: Direction): MutableBlockPos;
    public setWithOffset(pos: BlockPos, dx: number, dy: number, dz: number): MutableBlockPos;
    public setWithOffset(pos: BlockPos, second: Direction | number, third?: number, fourth?: number): MutableBlockPos {
      if (second instanceof Direction) {
        return this.set(
          pos.getX() + second.getStepX(),
          pos.getY() + second.getStepY(),
          pos.getZ() + second.getStepZ(),
        );
      }

      return this.set(
        pos.getX() + second,
        pos.getY() + (third ?? 0),
        pos.getZ() + (fourth ?? 0),
      );
    }

    public move(direction: Direction): MutableBlockPos;
    public move(dx: number, dy: number, dz: number): MutableBlockPos;
    public move(first: Direction | number, second?: number, third?: number): MutableBlockPos {
      if (first instanceof Direction) {
        return this.set(this.getX() + first.getStepX(), this.getY() + first.getStepY(), this.getZ() + first.getStepZ());
      }

      return this.set(this.getX() + first, this.getY() + (second ?? 0), this.getZ() + (third ?? 0));
    }
  }
}
