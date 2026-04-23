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
  GRANITE: 10,
  DIORITE: 11,
  ANDESITE: 12,
  COARSE_DIRT: 13,
  PODZOL: 14,
  MYCELIUM: 15,
  TERRACOTTA: 16,
  WHITE_TERRACOTTA: 17,
  ORANGE_TERRACOTTA: 18,
  MAGENTA_TERRACOTTA: 19,
  LIGHT_BLUE_TERRACOTTA: 20,
  YELLOW_TERRACOTTA: 21,
  LIME_TERRACOTTA: 22,
  PINK_TERRACOTTA: 23,
  GRAY_TERRACOTTA: 24,
  LIGHT_GRAY_TERRACOTTA: 25,
  CYAN_TERRACOTTA: 26,
  PURPLE_TERRACOTTA: 27,
  BLUE_TERRACOTTA: 28,
  BROWN_TERRACOTTA: 29,
  GREEN_TERRACOTTA: 30,
  RED_TERRACOTTA: 31,
  BLACK_TERRACOTTA: 32,
  SANDSTONE: 33,
  RED_SANDSTONE: 34,
  PACKED_ICE: 35,
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
  "minecraft:granite",
  "minecraft:diorite",
  "minecraft:andesite",
  "minecraft:coarse_dirt",
  "minecraft:podzol",
  "minecraft:mycelium",
  "minecraft:terracotta",
  "minecraft:white_terracotta",
  "minecraft:orange_terracotta",
  "minecraft:magenta_terracotta",
  "minecraft:light_blue_terracotta",
  "minecraft:yellow_terracotta",
  "minecraft:lime_terracotta",
  "minecraft:pink_terracotta",
  "minecraft:gray_terracotta",
  "minecraft:light_gray_terracotta",
  "minecraft:cyan_terracotta",
  "minecraft:purple_terracotta",
  "minecraft:blue_terracotta",
  "minecraft:brown_terracotta",
  "minecraft:green_terracotta",
  "minecraft:red_terracotta",
  "minecraft:black_terracotta",
  "minecraft:sandstone",
  "minecraft:red_sandstone",
  "minecraft:packed_ice",
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
    case ChunkBlockId.GRANITE:
    case ChunkBlockId.DIORITE:
    case ChunkBlockId.ANDESITE:
    case ChunkBlockId.COARSE_DIRT:
    case ChunkBlockId.PODZOL:
    case ChunkBlockId.MYCELIUM:
    case ChunkBlockId.TERRACOTTA:
    case ChunkBlockId.WHITE_TERRACOTTA:
    case ChunkBlockId.ORANGE_TERRACOTTA:
    case ChunkBlockId.MAGENTA_TERRACOTTA:
    case ChunkBlockId.LIGHT_BLUE_TERRACOTTA:
    case ChunkBlockId.YELLOW_TERRACOTTA:
    case ChunkBlockId.LIME_TERRACOTTA:
    case ChunkBlockId.PINK_TERRACOTTA:
    case ChunkBlockId.GRAY_TERRACOTTA:
    case ChunkBlockId.LIGHT_GRAY_TERRACOTTA:
    case ChunkBlockId.CYAN_TERRACOTTA:
    case ChunkBlockId.PURPLE_TERRACOTTA:
    case ChunkBlockId.BLUE_TERRACOTTA:
    case ChunkBlockId.BROWN_TERRACOTTA:
    case ChunkBlockId.GREEN_TERRACOTTA:
    case ChunkBlockId.RED_TERRACOTTA:
    case ChunkBlockId.BLACK_TERRACOTTA:
    case ChunkBlockId.SANDSTONE:
    case ChunkBlockId.RED_SANDSTONE:
    case ChunkBlockId.PACKED_ICE:
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
