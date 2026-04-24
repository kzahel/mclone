import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import { GeneratedDecorationRegion } from "../../src/world/level/generated-decoration-region";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { GeneratedRenderLevel } from "../../src/world/level/generated-render-level";
import type { BlockState } from "../../src/world/level/block/state/block-state";
import type { Block } from "../../src/world/level/block/block";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { NoiseBasedChunkGenerator } from "../../src/worldgen/levelgen/noise-based-chunk-generator";

function createGeneratedLevel(): GeneratedRenderLevel {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  const generator = new NoiseBasedChunkGenerator(biomeSource, 12345n);
  return new GeneratedRenderLevel(blocks.airState, generator, biomeSource, 12345n, blocks.blockStateById);
}

function getRequiredBlockState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing block ${location}`);
  }

  return block.defaultBlockState();
}

describe("GeneratedDecorationRegion", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("keeps FEATURES reads at ±8 chunks and normal writes at ±1 chunk", () => {
    const level = createGeneratedLevel();
    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getAuthorityChunk(10, 0)).not.toBeNull();

    const region = new GeneratedDecorationRegion(level, 0, 0);
    const writablePos = new BlockPos(16, 90, 0);
    const blockedPos = new BlockPos(32, 90, 0);
    const farReadPos = new BlockPos(9 * 16, 90, 0);
    const writableState = getRequiredBlockState("minecraft:oak_log");
    const blockedBefore = level.getBlockState(blockedPos);

    expect(region.setBlock(writablePos, writableState, 2)).toBe(true);
    expect(level.getBlockState(writablePos).getBlock()).toBe(writableState.getBlock());

    expect(region.setBlock(blockedPos, writableState, 2)).toBe(false);
    expect(level.getBlockState(blockedPos)).toBe(blockedBefore);

    expect(() => region.getBlockState(farReadPos)).toThrow(/dependency window/);
  });
});
