import { BlockPos } from "./block-pos";
import { SectionPos } from "./section-pos";

const COORD_MASK = 0xffff_ffffn;

function unpackSigned32(value: bigint): number {
  return Number(BigInt.asIntN(32, value));
}

export class ChunkPos {
  public constructor(
    public readonly x: number,
    public readonly z: number,
  ) {}

  public static ofBlockPos(pos: BlockPos): ChunkPos {
    return new ChunkPos(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getZ()),
    );
  }

  public static ofLong(value: bigint): ChunkPos {
    return new ChunkPos(ChunkPos.getX(value), ChunkPos.getZ(value));
  }

  public static asLong(x: number, z: number): bigint {
    return BigInt.asIntN(64, (BigInt(x) & COORD_MASK) | ((BigInt(z) & COORD_MASK) << 32n));
  }

  public static asLongFromBlockPos(pos: BlockPos): bigint {
    return ChunkPos.asLong(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()));
  }

  public static getX(chunkAsLong: bigint): number {
    return unpackSigned32(chunkAsLong & COORD_MASK);
  }

  public static getZ(chunkAsLong: bigint): number {
    return unpackSigned32((chunkAsLong >> 32n) & COORD_MASK);
  }

  public toLong(): bigint {
    return ChunkPos.asLong(this.x, this.z);
  }

  public getMinBlockX(): number {
    return SectionPos.sectionToBlockCoord(this.x);
  }

  public getMinBlockZ(): number {
    return SectionPos.sectionToBlockCoord(this.z);
  }

  public getMaxBlockX(): number {
    return this.getBlockX(15);
  }

  public getMaxBlockZ(): number {
    return this.getBlockZ(15);
  }

  public getMiddleBlockX(): number {
    return this.getBlockX(8);
  }

  public getMiddleBlockZ(): number {
    return this.getBlockZ(8);
  }

  public getBlockX(offset: number): number {
    return SectionPos.sectionToBlockCoordWithOffset(this.x, offset);
  }

  public getBlockZ(offset: number): number {
    return SectionPos.sectionToBlockCoordWithOffset(this.z, offset);
  }

  public getWorldPosition(): BlockPos {
    return new BlockPos(this.getMinBlockX(), 0, this.getMinBlockZ());
  }

  public getChessboardDistance(other: ChunkPos): number {
    return Math.max(Math.abs(this.x - other.x), Math.abs(this.z - other.z));
  }
}
