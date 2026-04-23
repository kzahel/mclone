import { describe, expect, test } from "vitest";
import { Biome } from "../../../src/worldgen/biome/biome.ts";
import { ChunkBlockId, MutableChunkBlockBuffer } from "../../../src/worldgen/chunk/chunk-block-buffer.ts";
import type { CarverContext, CanyonCarverConfiguration, CaveCarverConfiguration } from "../../../src/worldgen/carver/carver-config.ts";
import { absolute, constantFloat } from "../../../src/worldgen/carver/carver-config.ts";
import { UnderwaterCanyonWorldCarver } from "../../../src/worldgen/carver/underwater-canyon-world-carver.ts";
import { UnderwaterCaveWorldCarver } from "../../../src/worldgen/carver/underwater-cave-world-carver.ts";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source.ts";

const TEST_CONTEXT: CarverContext = {
  minY: 0,
  genDepth: 256,
  seaLevel: 63,
};

const TEST_CAVE_CONFIG: CaveCarverConfiguration = {
  probability: 1,
  y: { sample: () => 32 },
  yScale: constantFloat(1),
  lavaLevel: absolute(10),
  aquifersEnabled: false,
  horizontalRadiusMultiplier: constantFloat(1),
  verticalRadiusMultiplier: constantFloat(1),
  floorLevel: constantFloat(-0.7),
};

const TEST_CANYON_CONFIG: CanyonCarverConfiguration = {
  probability: 1,
  y: { sample: () => 32 },
  yScale: constantFloat(1),
  lavaLevel: absolute(10),
  aquifersEnabled: false,
  verticalRotation: constantFloat(0),
  shape: {
    distanceFactor: constantFloat(1),
    thickness: constantFloat(1),
    widthSmoothness: 1,
    horizontalRadiusFactor: constantFloat(1),
    verticalRadiusDefaultFactor: 1,
    verticalRadiusCenterFactor: 0,
  },
};

const UNUSED_BIOME_ACCESSOR = (): Biome => new Biome(0, "minecraft:ocean", 0, 0, 0.5, 0.5, 0x3f76e4);

class FixedFloatRandom extends SimpleRandomSource {
  public constructor(private readonly value: number) {
    super(0n);
  }

  public override nextFloat(): number {
    return this.value;
  }
}

class TestUnderwaterCaveWorldCarver extends UnderwaterCaveWorldCarver {
  public carveAt(chunk: MutableChunkBlockBuffer, random: SimpleRandomSource, y: number, x = 1, z = 1): boolean {
    return this.carveBlock(
      TEST_CONTEXT,
      TEST_CAVE_CONFIG,
      chunk,
      UNUSED_BIOME_ACCESSOR,
      random,
      x,
      y,
      z,
      false,
    );
  }
}

class TestUnderwaterCanyonWorldCarver extends UnderwaterCanyonWorldCarver {
  public carveAt(chunk: MutableChunkBlockBuffer, random: SimpleRandomSource, y: number, x = 1, z = 1): boolean {
    return this.carveBlock(
      TEST_CONTEXT,
      TEST_CANYON_CONFIG,
      chunk,
      UNUSED_BIOME_ACCESSOR,
      random,
      x,
      y,
      z,
      false,
    );
  }
}

describe("Underwater carvers", () => {
  test("skip carving at or above sea level even when the current block is replaceable", () => {
    const chunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    chunk.setBlockAtY(1, 63, 1, ChunkBlockId.STONE);

    const carved = new TestUnderwaterCaveWorldCarver().carveAt(chunk, new FixedFloatRandom(0.5), 63);

    expect(carved).toBe(false);
    expect(chunk.getBlockAtY(1, 63, 1)).toBe(ChunkBlockId.STONE);
  });

  test("fill water below sea level, magma-or-obsidian at y=10, and lava below y=10", () => {
    const caveCarver = new TestUnderwaterCaveWorldCarver();

    const floodedChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    floodedChunk.setBlockAtY(1, 62, 1, ChunkBlockId.STONE);
    expect(caveCarver.carveAt(floodedChunk, new FixedFloatRandom(0.5), 62)).toBe(true);
    expect(floodedChunk.getBlockAtY(1, 62, 1)).toBe(ChunkBlockId.WATER);
    expect(floodedChunk.getScheduledLiquidTicks()).toEqual([
      { x: 1, y: 62, z: 1, target: "minecraft:water", delay: 0 },
    ]);

    const magmaChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    magmaChunk.setBlockAtY(1, 10, 1, ChunkBlockId.STONE);
    expect(caveCarver.carveAt(magmaChunk, new FixedFloatRandom(0.1), 10)).toBe(true);
    expect(magmaChunk.getBlockAtY(1, 10, 1)).toBe(ChunkBlockId.MAGMA_BLOCK);
    expect(magmaChunk.getScheduledBlockTicks()).toEqual([
      { x: 1, y: 10, z: 1, target: "minecraft:magma_block", delay: 0 },
    ]);

    const obsidianChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    obsidianChunk.setBlockAtY(1, 10, 1, ChunkBlockId.STONE);
    expect(caveCarver.carveAt(obsidianChunk, new FixedFloatRandom(0.5), 10)).toBe(true);
    expect(obsidianChunk.getBlockAtY(1, 10, 1)).toBe(ChunkBlockId.OBSIDIAN);
    expect(obsidianChunk.getScheduledBlockTicks()).toEqual([]);

    const lavaChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    lavaChunk.setBlockAtY(1, 9, 1, ChunkBlockId.STONE);
    expect(caveCarver.carveAt(lavaChunk, new FixedFloatRandom(0.5), 9)).toBe(false);
    expect(lavaChunk.getBlockAtY(1, 9, 1)).toBe(ChunkBlockId.LAVA);
  });

  test("only schedule a water liquid tick when a flow direction crosses the chunk boundary or opens into air", () => {
    const caveCarver = new TestUnderwaterCaveWorldCarver();
    const enclosedChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    for (const [x, y, z] of [
      [1, 20, 1],
      [1, 19, 1],
      [1, 20, 0],
      [1, 20, 2],
      [0, 20, 1],
      [2, 20, 1],
    ] as const) {
      enclosedChunk.setBlockAtY(x, y, z, ChunkBlockId.STONE);
    }

    expect(caveCarver.carveAt(enclosedChunk, new FixedFloatRandom(0.5), 20)).toBe(true);
    expect(enclosedChunk.getScheduledLiquidTicks()).toEqual([]);

    const edgeChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    edgeChunk.setBlockAtY(0, 20, 0, ChunkBlockId.STONE);
    expect(caveCarver.carveAt(edgeChunk, new FixedFloatRandom(0.5), 20, 0, 0)).toBe(true);
    expect(edgeChunk.getScheduledLiquidTicks()).toEqual([
      { x: 0, y: 20, z: 0, target: "minecraft:water", delay: 0 },
    ]);
  });

  test("underwater canyons can replace air pockets while underwater caves cannot", () => {
    const caveChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);
    const canyonChunk = new MutableChunkBlockBuffer(0, 0, 0, 256);

    const caveCarver = new TestUnderwaterCaveWorldCarver();
    const canyonCarver = new TestUnderwaterCanyonWorldCarver();

    expect(caveCarver.carveAt(caveChunk, new FixedFloatRandom(0.5), 20)).toBe(false);
    expect(caveChunk.getBlockAtY(1, 20, 1)).toBe(ChunkBlockId.AIR);

    expect(canyonCarver.carveAt(canyonChunk, new FixedFloatRandom(0.5), 20)).toBe(true);
    expect(canyonChunk.getBlockAtY(1, 20, 1)).toBe(ChunkBlockId.WATER);
  });
});
