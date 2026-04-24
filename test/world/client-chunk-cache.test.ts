import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../src/worldgen/biome/chunk-biome-container";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";
import type { Block } from "../../src/world/level/block/block";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { buildChunkSnapshot, createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { DoublePlantBlock } from "../../src/world/level/block/double-plant-block";
import { BlockStateProperties } from "../../src/world/level/block/state/properties/block-state-properties";
import { DoubleBlockHalf } from "../../src/world/level/block/state/properties/double-block-half";

describe("ClientChunkCache", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("hydrates chunk snapshots into a read-only renderer cache", () => {
    const blocks = registerGeneratedRenderBlocks();
    const lilac = Registry.BLOCK.get(new ResourceLocation("minecraft:lilac")) as Block | undefined;
    if (lilac === undefined) {
      throw new Error("missing lilac registration");
    }

    const sourceChunk = new LevelChunk(0, 0, blocks.airState);
    sourceChunk.setBlockState(
      new BlockPos(1, 80, 1),
      blocks.blockStateById[ChunkBlockId.GRASS_BLOCK]!.setValue(BlockStateProperties.SNOWY, true),
    );
    sourceChunk.setBlockState(
      new BlockPos(2, 81, 2),
      lilac.defaultBlockState().setValue(DoublePlantBlock.HALF, DoubleBlockHalf.UPPER),
    );

    const biomeSource = new OverworldBiomeSource(12345n);
    const cache = new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: 0,
      height: 256,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    });

    cache.applyChunkSnapshot(
      buildChunkSnapshot(
        sourceChunk,
        new ChunkBiomeContainer(0, 256, 0, 0, biomeSource).writeBiomes(),
        0,
        256,
      ),
    );

    expect(cache.getChunk(0, 0, false)).not.toBeNull();
    expect(cache.getBlockState(new BlockPos(1, 80, 1)).getValue(BlockStateProperties.SNOWY)).toBe(true);
    expect(cache.getBlockState(new BlockPos(2, 81, 2)).getValue(DoublePlantBlock.HALF)).toBe(DoubleBlockHalf.UPPER);
    expect(cache.setBlock(new BlockPos(3, 82, 3), blocks.blockStateById[ChunkBlockId.STONE]!)).toBe(false);
    expect(cache.applyChunkUnload(0, 0)).toBe(true);
    expect(cache.getChunk(0, 0, false)).toBeNull();
  });
});
