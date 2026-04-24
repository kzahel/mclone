import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import { DataLayer } from "../../src/world/level/chunk/data-layer";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../src/worldgen/biome/chunk-biome-container";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";
import type { Block } from "../../src/world/level/block/block";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { buildChunkSnapshot, createBlockStateResolver, type ChunkSnapshot } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { LightLayer } from "../../src/world/level/light-layer";
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

  test("hydrates chunk light snapshots for renderer brightness lookups", () => {
    const blocks = registerGeneratedRenderBlocks();
    const biomeSource = new OverworldBiomeSource(12345n);
    const cache = new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: 0,
      height: 256,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
      skyLight: 15,
      blockLight: 15,
    });

    const sky = new DataLayer();
    sky.set(1, 0, 1, 12);
    const skyAbove = new DataLayer();
    skyAbove.set(1, 0, 1, 9);
    const block = new DataLayer();
    block.set(1, 0, 1, 7);
    const snapshot: ChunkSnapshot = {
      chunkX: 0,
      chunkZ: 0,
      biomes: new ChunkBiomeContainer(0, 256, 0, 0, biomeSource).writeBiomes(),
      sections: [],
      light: {
        sky: [
          { y: 5, data: sky.getData() },
          { y: 6, data: skyAbove.getData() },
        ],
        block: [{ y: 5, data: block.getData() }],
        lightCorrect: true,
      },
      blockTicks: [],
      liquidTicks: [],
    };

    cache.applyChunkSnapshot(snapshot);

    expect(cache.getBrightness(LightLayer.SKY, new BlockPos(1, 80, 1))).toBe(12);
    expect(cache.getBrightness(LightLayer.BLOCK, new BlockPos(1, 80, 1))).toBe(7);
    expect(cache.getRawBrightness(new BlockPos(1, 80, 1), 3)).toBe(9);
    expect(cache.getBrightness(LightLayer.SKY, new BlockPos(1, 96, 1))).toBe(9);
    expect(cache.getBrightness(LightLayer.BLOCK, new BlockPos(1, 81, 1))).toBe(0);
    expect(cache.getChunkSnapshot(0, 0)?.light?.lightCorrect).toBe(true);

    const blockDelta = new DataLayer();
    blockDelta.set(1, 0, 1, 5);
    expect(cache.applyChunkLightDelta({
      type: "chunk_light_delta",
      chunkX: 0,
      chunkZ: 0,
      light: {
        sky: [{ y: 5 }],
        block: [{ y: 5, data: blockDelta.getData() }],
      },
    })).toBe(true);
    expect(cache.getBrightness(LightLayer.SKY, new BlockPos(1, 80, 1))).toBe(0);
    expect(cache.getBrightness(LightLayer.BLOCK, new BlockPos(1, 80, 1))).toBe(5);
    expect(cache.getChunkSnapshot(0, 0)?.light?.sky.find((section) => section.y === 5)?.data).toEqual(new Uint8Array(DataLayer.SIZE));

    expect(cache.applyChunkUnload(0, 0)).toBe(true);
    expect(cache.applyChunkLightDelta({
      type: "chunk_light_delta",
      chunkX: 0,
      chunkZ: 0,
      light: { block: [{ y: 5, data: blockDelta.getData() }] },
    })).toBe(false);
    expect(cache.getBrightness(LightLayer.BLOCK, new BlockPos(1, 80, 1))).toBe(15);
  });
});
