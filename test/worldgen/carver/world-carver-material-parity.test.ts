import { describe, expect, test } from "vitest";
import type { Biome } from "../../../src/worldgen/biome/biome.ts";
import type { CarverConfiguration, CarverContext } from "../../../src/worldgen/carver/carver-config.ts";
import type { ChunkPosLike } from "../../../src/worldgen/carver/world-carver.ts";
import { WorldCarver } from "../../../src/worldgen/carver/world-carver.ts";
import { ChunkBlockId, MutableChunkBlockBuffer } from "../../../src/worldgen/chunk/chunk-block-buffer.ts";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source.ts";

class TestWorldCarver extends WorldCarver<CarverConfiguration> {
  public override isStartChunk(_config: CarverConfiguration, _random: SimpleRandomSource): boolean {
    return false;
  }

  public override carve(
    _context: CarverContext,
    _config: CarverConfiguration,
    _chunk: MutableChunkBlockBuffer,
    _biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    _random: SimpleRandomSource,
    _chunkPos: ChunkPosLike,
    _carvingMask: Uint8Array,
  ): boolean {
    return false;
  }

  public canReplace(blockId: ChunkBlockId): boolean {
    return this.canReplaceBlock(blockId);
  }

  public canReplaceWithAbove(blockId: ChunkBlockId, aboveBlockId: ChunkBlockId): boolean {
    return this.canReplaceBlockWithAbove(blockId, aboveBlockId);
  }

  public isSurfaceTop(blockId: ChunkBlockId): boolean {
    return this.isSurfaceTopBlock(blockId);
  }
}

const carver = new TestWorldCarver();

describe("WorldCarver material parity", () => {
  test("matches the widened live overworld replaceable-material set", () => {
    const replaceable = [
      ChunkBlockId.STONE,
      ChunkBlockId.GRANITE,
      ChunkBlockId.DIORITE,
      ChunkBlockId.ANDESITE,
      ChunkBlockId.DIRT,
      ChunkBlockId.COARSE_DIRT,
      ChunkBlockId.PODZOL,
      ChunkBlockId.GRASS_BLOCK,
      ChunkBlockId.MYCELIUM,
      ChunkBlockId.TERRACOTTA,
      ChunkBlockId.WHITE_TERRACOTTA,
      ChunkBlockId.ORANGE_TERRACOTTA,
      ChunkBlockId.MAGENTA_TERRACOTTA,
      ChunkBlockId.LIGHT_BLUE_TERRACOTTA,
      ChunkBlockId.YELLOW_TERRACOTTA,
      ChunkBlockId.LIME_TERRACOTTA,
      ChunkBlockId.PINK_TERRACOTTA,
      ChunkBlockId.GRAY_TERRACOTTA,
      ChunkBlockId.LIGHT_GRAY_TERRACOTTA,
      ChunkBlockId.CYAN_TERRACOTTA,
      ChunkBlockId.PURPLE_TERRACOTTA,
      ChunkBlockId.BLUE_TERRACOTTA,
      ChunkBlockId.BROWN_TERRACOTTA,
      ChunkBlockId.GREEN_TERRACOTTA,
      ChunkBlockId.RED_TERRACOTTA,
      ChunkBlockId.BLACK_TERRACOTTA,
      ChunkBlockId.SANDSTONE,
      ChunkBlockId.RED_SANDSTONE,
      ChunkBlockId.SNOW,
      ChunkBlockId.PACKED_ICE,
    ] as const;

    for (const blockId of replaceable) {
      expect(carver.canReplace(blockId)).toBe(true);
    }

    for (const blockId of [ChunkBlockId.AIR, ChunkBlockId.WATER, ChunkBlockId.BEDROCK, ChunkBlockId.LAVA]) {
      expect(carver.canReplace(blockId)).toBe(false);
    }
  });

  test("sand and gravel only use the special replacement path when the ceiling is not water", () => {
    expect(carver.canReplace(ChunkBlockId.SAND)).toBe(false);
    expect(carver.canReplace(ChunkBlockId.GRAVEL)).toBe(false);

    expect(carver.canReplaceWithAbove(ChunkBlockId.SAND, ChunkBlockId.AIR)).toBe(true);
    expect(carver.canReplaceWithAbove(ChunkBlockId.GRAVEL, ChunkBlockId.STONE)).toBe(true);
    expect(carver.canReplaceWithAbove(ChunkBlockId.SAND, ChunkBlockId.WATER)).toBe(false);
    expect(carver.canReplaceWithAbove(ChunkBlockId.GRAVEL, ChunkBlockId.WATER)).toBe(false);
  });

  test("surface-top detection treats mycelium the same way as grass_block", () => {
    expect(carver.isSurfaceTop(ChunkBlockId.GRASS_BLOCK)).toBe(true);
    expect(carver.isSurfaceTop(ChunkBlockId.MYCELIUM)).toBe(true);
    expect(carver.isSurfaceTop(ChunkBlockId.TERRACOTTA)).toBe(false);
    expect(carver.isSurfaceTop(ChunkBlockId.DIRT)).toBe(false);
  });
});
