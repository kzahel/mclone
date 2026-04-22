import {
  BLOCKS_PER_SECTION,
  CHUNK_BLOCK_NAMES,
  ChunkBlockId,
  type ChunkBlockName,
  chunkBlockNameForId,
  MutableChunkBlockBuffer,
  SECTION_HEIGHT,
} from "./chunk-block-buffer.ts";

export interface ChunkSection {
  readonly y: number;
  readonly palette: readonly ChunkBlockName[];
  readonly blockOrder: "y-major,z-major,x-minor";
  readonly blocks: readonly number[];
}

export function buildChunkSections(chunk: MutableChunkBlockBuffer): ChunkSection[] {
  const sectionCount = chunk.height / SECTION_HEIGHT;
  const minSectionY = Math.floor(chunk.minY / SECTION_HEIGHT);
  const sections: ChunkSection[] = [];

  for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
    const start = sectionOffset * BLOCKS_PER_SECTION;
    const usedBlockIds = new Uint8Array(CHUNK_BLOCK_NAMES.length);
    let hasNonAir = false;

    for (let index = 0; index < BLOCKS_PER_SECTION; index++) {
      const blockId = chunk.blocks[start + index]!;
      usedBlockIds[blockId] = 1;
      if (blockId !== ChunkBlockId.AIR) {
        hasNonAir = true;
      }
    }

    if (!hasNonAir) {
      continue;
    }

    const paletteIds: ChunkBlockId[] = [];
    const paletteIndexByBlockId = new Int16Array(CHUNK_BLOCK_NAMES.length).fill(-1);
    for (let blockId = 0; blockId < CHUNK_BLOCK_NAMES.length; blockId++) {
      if (usedBlockIds[blockId] !== 0) {
        paletteIndexByBlockId[blockId] = paletteIds.length;
        paletteIds.push(blockId as ChunkBlockId);
      }
    }

    const paletteIndices = new Array<number>(BLOCKS_PER_SECTION);
    for (let index = 0; index < BLOCKS_PER_SECTION; index++) {
      paletteIndices[index] = paletteIndexByBlockId[chunk.blocks[start + index]!]!;
    }

    sections.push({
      y: minSectionY + sectionOffset,
      palette: paletteIds.map((blockId) => chunkBlockNameForId(blockId)),
      blockOrder: "y-major,z-major,x-minor",
      blocks: paletteIndices,
    });
  }

  return sections;
}
