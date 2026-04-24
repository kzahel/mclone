import { intFloorDiv } from "../util/mth";
import { BlockPos } from "./block-pos";
import { Direction } from "./direction";
import { Vec3i } from "./vec3i";

export interface SectionPosLike {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

const PACKED_X_LENGTH = 22;
const PACKED_Y_LENGTH = 20;
const PACKED_Z_LENGTH = 22;
const PACKED_X_MASK = (1n << BigInt(PACKED_X_LENGTH)) - 1n;
const PACKED_Y_MASK = (1n << BigInt(PACKED_Y_LENGTH)) - 1n;
const PACKED_Z_MASK = (1n << BigInt(PACKED_Z_LENGTH)) - 1n;
const Z_OFFSET = PACKED_Y_LENGTH;
const X_OFFSET = PACKED_Y_LENGTH + PACKED_Z_LENGTH;

function unpackSigned(value: bigint, leftShift: number, rightShift: number): number {
  return Number(BigInt.asIntN(64, value << BigInt(leftShift)) >> BigInt(rightShift));
}

export class SectionPos extends Vec3i {
  public constructor(x: number, y: number, z: number) {
    super(x, y, z);
  }

  public static of(x: number, y: number, z: number): SectionPos {
    return new SectionPos(x, y, z);
  }

  public static ofKey(value: bigint): SectionPos {
    return new SectionPos(SectionPos.x(value), SectionPos.y(value), SectionPos.z(value));
  }

  public static fromBlockPos(pos: BlockPos): SectionPos {
    return new SectionPos(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getY()),
      SectionPos.blockToSectionCoord(pos.getZ()),
    );
  }

  public static posToSectionCoord(value: number): number {
    return SectionPos.blockToSectionCoord(Math.floor(value));
  }

  public static blockToSectionCoord(value: number): number {
    return intFloorDiv(value, 16);
  }

  public static sectionToBlockCoord(value: number): number {
    return value << 4;
  }

  public static sectionToBlockCoordWithOffset(value: number, offset: number): number {
    return SectionPos.sectionToBlockCoord(value) + offset;
  }

  public static sectionRelative(value: number): number {
    return value & 15;
  }

  public static x(value: bigint): number {
    return unpackSigned(value, 64 - X_OFFSET - PACKED_X_LENGTH, 64 - PACKED_X_LENGTH);
  }

  public static y(value: bigint): number {
    return unpackSigned(value, 64 - PACKED_Y_LENGTH, 64 - PACKED_Y_LENGTH);
  }

  public static z(value: bigint): number {
    return unpackSigned(value, 64 - Z_OFFSET - PACKED_Z_LENGTH, 64 - PACKED_Z_LENGTH);
  }

  public static asLong(pos: SectionPosLike): bigint;
  public static asLong(x: number, y: number, z: number): bigint;
  public static asLong(first: SectionPosLike | number, second?: number, third?: number): bigint {
    const x = typeof first === "number" ? first : first.x;
    const y = typeof first === "number" ? second! : first.y;
    const z = typeof first === "number" ? third! : first.z;
    let packed = 0n;
    packed |= (BigInt(x) & PACKED_X_MASK) << BigInt(X_OFFSET);
    packed |= BigInt(y) & PACKED_Y_MASK;
    packed |= (BigInt(z) & PACKED_Z_MASK) << BigInt(Z_OFFSET);
    return BigInt.asIntN(64, packed);
  }

  public static asLongFromBlockPos(pos: BlockPos): bigint {
    return SectionPos.asLong(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getY()),
      SectionPos.blockToSectionCoord(pos.getZ()),
    );
  }

  public static blockToSection(value: bigint): bigint {
    return SectionPos.asLong(
      SectionPos.blockToSectionCoord(BlockPos.getX(value)),
      SectionPos.blockToSectionCoord(BlockPos.getY(value)),
      SectionPos.blockToSectionCoord(BlockPos.getZ(value)),
    );
  }

  public static getZeroNode(value: bigint): bigint {
    return value & -1048576n;
  }

  public static offset(value: bigint, direction: Direction): bigint;
  public static offset(value: bigint, x: number, y: number, z: number): bigint;
  public static offset(value: bigint, second: Direction | number, third?: number, fourth?: number): bigint {
    if (second instanceof Direction) {
      return SectionPos.offset(value, second.getStepX(), second.getStepY(), second.getStepZ());
    }

    return SectionPos.asLong(
      SectionPos.x(value) + second,
      SectionPos.y(value) + (third ?? 0),
      SectionPos.z(value) + (fourth ?? 0),
    );
  }

  public asLong(): bigint {
    return SectionPos.asLong(this.getX(), this.getY(), this.getZ());
  }

  public x(): number {
    return this.getX();
  }

  public y(): number {
    return this.getY();
  }

  public z(): number {
    return this.getZ();
  }
}
