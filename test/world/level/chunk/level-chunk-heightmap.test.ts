import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ChunkBlockId } from "../../../../src/worldgen/chunk/chunk-block-buffer";
import { Heightmap } from "../../../../src/worldgen/levelgen/heightmap";
import { LevelChunk } from "../../../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";

describe("LevelChunk heightmaps", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("lazily primes and updates highest taken block columns", () => {
    const blocks = registerGeneratedRenderBlocks();
    const chunk = new LevelChunk(0, 0, blocks.airState, 0, 32);
    const stone = blocks.blockStateById[ChunkBlockId.STONE]!;
    const lowPos = new BlockPos(1, 10, 2);
    const highPos = new BlockPos(1, 15, 2);

    expect(chunk.getHeight(Heightmap.Types.WORLD_SURFACE_WG, 1, 2)).toBe(-1);

    chunk.setBlockState(lowPos, stone);
    expect(chunk.getHeight(Heightmap.Types.WORLD_SURFACE_WG, 1, 2)).toBe(10);

    chunk.setBlockState(highPos, stone);
    expect(chunk.getHeight(Heightmap.Types.WORLD_SURFACE_WG, 1, 2)).toBe(15);

    chunk.setBlockState(highPos, blocks.airState);
    expect(chunk.getHeight(Heightmap.Types.WORLD_SURFACE_WG, 1, 2)).toBe(10);
  });
});
