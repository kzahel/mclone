export const CHUNK_WIDTH = 16;
export const SECTION_HEIGHT = 16;
export const BLOCKS_PER_SECTION = CHUNK_WIDTH * CHUNK_WIDTH * SECTION_HEIGHT;

export const ChunkBlockId = {
  AIR: 0,
  STONE: 1,
  WATER: 2,
  BEDROCK: 3,
  GRASS_BLOCK: 4,
  DIRT: 5,
  SAND: 6,
  GRAVEL: 7,
  SNOW: 8,
  LAVA: 9,
} as const;

export type ChunkBlockId = (typeof ChunkBlockId)[keyof typeof ChunkBlockId];

export const CHUNK_BLOCK_NAMES = [
  "minecraft:air",
  "minecraft:stone",
  "minecraft:water",
  "minecraft:bedrock",
  "minecraft:grass_block",
  "minecraft:dirt",
  "minecraft:sand",
  "minecraft:gravel",
  "minecraft:snow",
  "minecraft:lava",
] as const;

export const TERRAIN_STAGE_BLOCK_NAMES = [
  "minecraft:air",
  "minecraft:stone",
  "minecraft:water",
  "minecraft:bedrock",
] as const;

export type ChunkBlockName = (typeof CHUNK_BLOCK_NAMES)[number];

export function blockBufferIndex(localX: number, localY: number, localZ: number): number {
  return (localY << 8) | (localZ << 4) | localX;
}

export function chunkBlockNameForId(blockId: ChunkBlockId): ChunkBlockName {
  return CHUNK_BLOCK_NAMES[blockId]!;
}

export function isMotionBlockingBlock(blockId: ChunkBlockId): boolean {
  switch (blockId) {
    case ChunkBlockId.STONE:
    case ChunkBlockId.BEDROCK:
    case ChunkBlockId.GRASS_BLOCK:
    case ChunkBlockId.DIRT:
    case ChunkBlockId.SAND:
    case ChunkBlockId.GRAVEL:
      return true;
    default:
      return false;
  }
}

export class MutableChunkBlockBuffer {
  public readonly blocks: Uint8Array;

  public constructor(
    public readonly chunkX: number,
    public readonly chunkZ: number,
    public readonly minY: number,
    public readonly height: number,
    public readonly biomes: readonly number[] = [],
    blocks?: Uint8Array,
  ) {
    if (height <= 0 || height % SECTION_HEIGHT !== 0) {
      throw new RangeError(`chunk height ${height} must be a positive multiple of ${SECTION_HEIGHT}`);
    }

    const expectedLength = height * CHUNK_WIDTH * CHUNK_WIDTH;
    if (blocks !== undefined && blocks.length !== expectedLength) {
      throw new RangeError(`chunk block buffer length ${blocks.length} did not match expected ${expectedLength}`);
    }

    this.blocks = blocks === undefined ? new Uint8Array(expectedLength) : Uint8Array.from(blocks);
  }

  public getBlock(localX: number, localY: number, localZ: number): ChunkBlockId {
    return this.blocks[blockBufferIndex(localX, localY, localZ)]! as ChunkBlockId;
  }

  public getBlockAtY(localX: number, y: number, localZ: number): ChunkBlockId {
    return this.getBlock(localX, y - this.minY, localZ);
  }

  public setBlock(localX: number, localY: number, localZ: number, blockId: ChunkBlockId): void {
    this.blocks[blockBufferIndex(localX, localY, localZ)] = blockId;
  }

  public setBlockAtY(localX: number, y: number, localZ: number, blockId: ChunkBlockId): void {
    this.setBlock(localX, y - this.minY, localZ, blockId);
  }
}
