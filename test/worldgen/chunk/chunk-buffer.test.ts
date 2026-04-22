import { describe, expect, test } from "vitest";
import {
  ChunkBlockId,
  MutableChunkBlockBuffer,
  blockBufferIndex,
} from "../../../src/worldgen/chunk/chunk-block-buffer.ts";
import { buildChunkHeightmaps } from "../../../src/worldgen/chunk/chunk-heightmaps.ts";
import { buildChunkSections } from "../../../src/worldgen/chunk/chunk-section-serialization.ts";

function columnIndex(localX: number, localZ: number): number {
  return (localZ << 4) | localX;
}

describe("MutableChunkBlockBuffer helpers", () => {
  test("stores widened numeric block ids and supports local or absolute-Y writes", () => {
    const chunk = new MutableChunkBlockBuffer(0, 0, 0, 32);

    chunk.setBlock(1, 2, 3, ChunkBlockId.GRASS_BLOCK);
    chunk.setBlockAtY(4, 17, 5, ChunkBlockId.SAND);

    expect(chunk.getBlock(1, 2, 3)).toBe(ChunkBlockId.GRASS_BLOCK);
    expect(chunk.getBlockAtY(4, 17, 5)).toBe(ChunkBlockId.SAND);
    expect(chunk.blocks[blockBufferIndex(4, 17, 5)]).toBe(ChunkBlockId.SAND);
  });

  test("serializes only non-empty sections and compacts each section palette independently", () => {
    const chunk = new MutableChunkBlockBuffer(0, 0, 0, 48);

    chunk.setBlock(2, 1, 3, ChunkBlockId.STONE);
    chunk.setBlock(4, 34, 5, ChunkBlockId.GRASS_BLOCK);
    chunk.setBlock(6, 35, 7, ChunkBlockId.SNOW);

    const sections = buildChunkSections(chunk);

    expect(sections).toHaveLength(2);

    expect(sections[0]).toEqual({
      y: 0,
      palette: ["minecraft:air", "minecraft:stone"],
      blockOrder: "y-major,z-major,x-minor",
      blocks: expect.any(Array),
    });
    expect(sections[1]).toEqual({
      y: 2,
      palette: ["minecraft:air", "minecraft:grass_block", "minecraft:snow"],
      blockOrder: "y-major,z-major,x-minor",
      blocks: expect.any(Array),
    });

    expect(sections[0]!.blocks[blockBufferIndex(2, 1, 3)]).toBe(1);
    expect(sections[1]!.blocks[blockBufferIndex(4, 2, 5)]).toBe(1);
    expect(sections[1]!.blocks[blockBufferIndex(6, 3, 7)]).toBe(2);
  });

  test("builds world-surface and ocean-floor heightmaps with motion-blocking semantics", () => {
    const chunk = new MutableChunkBlockBuffer(0, 0, 0, 32);

    chunk.setBlockAtY(0, 5, 0, ChunkBlockId.STONE);
    chunk.setBlockAtY(1, 7, 0, ChunkBlockId.SNOW);
    chunk.setBlockAtY(2, 2, 0, ChunkBlockId.SAND);
    chunk.setBlockAtY(2, 4, 0, ChunkBlockId.WATER);
    chunk.setBlockAtY(3, 10, 0, ChunkBlockId.GRASS_BLOCK);
    chunk.setBlockAtY(3, 11, 0, ChunkBlockId.SNOW);

    const heightmaps = buildChunkHeightmaps(chunk);

    expect(heightmaps.WORLD_SURFACE[columnIndex(0, 0)]).toBe(6);
    expect(heightmaps.OCEAN_FLOOR[columnIndex(0, 0)]).toBe(6);

    expect(heightmaps.WORLD_SURFACE[columnIndex(1, 0)]).toBe(8);
    expect(heightmaps.OCEAN_FLOOR[columnIndex(1, 0)]).toBe(0);

    expect(heightmaps.WORLD_SURFACE[columnIndex(2, 0)]).toBe(5);
    expect(heightmaps.OCEAN_FLOOR[columnIndex(2, 0)]).toBe(3);

    expect(heightmaps.WORLD_SURFACE[columnIndex(3, 0)]).toBe(12);
    expect(heightmaps.OCEAN_FLOOR[columnIndex(3, 0)]).toBe(11);
  });
});
