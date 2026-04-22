import {
  CHUNK_WIDTH,
  type ChunkBlockId,
  ChunkBlockId as BlockId,
  blockBufferIndex,
  isMotionBlockingBlock,
  MutableChunkBlockBuffer,
} from "./chunk-block-buffer.ts";

export interface ChunkHeightmaps {
  readonly OCEAN_FLOOR: readonly number[];
  readonly WORLD_SURFACE: readonly number[];
}

export function buildChunkHeightmaps(chunk: MutableChunkBlockBuffer): ChunkHeightmaps {
  const oceanFloor = new Array<number>(CHUNK_WIDTH * CHUNK_WIDTH).fill(chunk.minY);
  const worldSurface = new Array<number>(CHUNK_WIDTH * CHUNK_WIDTH).fill(chunk.minY);
  const maxY = chunk.minY + chunk.height - 1;

  for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
    for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
      const columnIndex = (localZ << 4) | localX;
      let foundOceanFloor = false;
      let foundWorldSurface = false;

      for (let y = maxY; y >= chunk.minY; y--) {
        const blockId = chunk.blocks[blockBufferIndex(localX, y - chunk.minY, localZ)]! as ChunkBlockId;
        if (!foundWorldSurface && blockId !== BlockId.AIR) {
          worldSurface[columnIndex] = y + 1;
          foundWorldSurface = true;
        }

        if (!foundOceanFloor && isMotionBlockingBlock(blockId)) {
          oceanFloor[columnIndex] = y + 1;
          foundOceanFloor = true;
        }

        if (foundWorldSurface && foundOceanFloor) {
          break;
        }
      }
    }
  }

  return {
    OCEAN_FLOOR: oceanFloor,
    WORLD_SURFACE: worldSurface,
  };
}
