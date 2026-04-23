import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import { BlockStateProperties } from "../../../../src/world/level/block/state/properties/block-state-properties";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { TreeFeatures } from "../../../../src/worldgen/levelgen/feature/tree-features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function createGenerator(): NoiseBasedChunkGenerator {
  const biomeSource = new OverworldBiomeSource(12345n);
  return new NoiseBasedChunkGenerator(biomeSource, 12345n);
}

function createFlatLevel(airState: BlockState, surfaceState: BlockState): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 64);
  for (let z = 0; z < 32; z++) {
    for (let x = 0; x < 32; x++) {
      level.setBlock(new BlockPos(x, 10, z), surfaceState);
    }
  }

  return level;
}

describe("TreeFeature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("oak placement uses the translated trunk and foliage path and updates leaf distance", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const feature = TreeFeatures.OAK;
    const logState = getState("minecraft:oak_log");
    const leavesState = getState("minecraft:oak_leaves");
    const origin = new BlockPos(16, 11, 16);
    const expectedHeight = feature.config.trunkPlacer.getTreeHeight(new WorldgenRandom(1234n));

    expect(feature.place(level, generator, new WorldgenRandom(1234n), origin)).toBe(true);

    let trunkHeight = 0;
    while (level.getBlockState(origin.above(trunkHeight)).is(logState.getBlock())) {
      trunkHeight++;
    }

    expect(trunkHeight).toBe(expectedHeight);

    let leafCount = 0;
    let foundDistanceOneLeaf = false;
    for (let y = 11; y < 24; y++) {
      for (let z = 10; z <= 22; z++) {
        for (let x = 10; x <= 22; x++) {
          const state = level.getBlockState(new BlockPos(x, y, z));
          if (!state.is(leavesState.getBlock())) {
            continue;
          }

          leafCount++;
          const distance = state.getValue(BlockStateProperties.DISTANCE);
          expect(distance).toBeLessThan(7);
          if (distance === 1) {
            foundDistanceOneLeaf = true;
          }
        }
      }
    }

    expect(leafCount).toBeGreaterThan(0);
    expect(foundDistanceOneLeaf).toBe(true);
  });

  test("dark oak placement uses the translated 2x2 trunk and dark-oak foliage path", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const feature = TreeFeatures.DARK_OAK;
    const logState = getState("minecraft:dark_oak_log");
    const leavesState = getState("minecraft:dark_oak_leaves");
    const origin = new BlockPos(16, 11, 16);
    const expectedHeight = feature.config.trunkPlacer.getTreeHeight(new WorldgenRandom(4321n));

    expect(feature.place(level, generator, new WorldgenRandom(4321n), origin)).toBe(true);

    expect(level.getBlockState(origin).is(logState.getBlock())).toBe(true);
    expect(level.getBlockState(origin.east()).is(logState.getBlock())).toBe(true);
    expect(level.getBlockState(origin.south()).is(logState.getBlock())).toBe(true);
    expect(level.getBlockState(origin.east().south()).is(logState.getBlock())).toBe(true);

    let logCount = 0;
    let leafCount = 0;
    for (let y = 10; y < 28; y++) {
      for (let z = 8; z <= 24; z++) {
        for (let x = 8; x <= 24; x++) {
          const state = level.getBlockState(new BlockPos(x, y, z));
          if (state.is(logState.getBlock())) {
            logCount++;
          } else if (state.is(leavesState.getBlock())) {
            leafCount++;
            expect(state.getValue(BlockStateProperties.DISTANCE)).toBeLessThan(7);
          }
        }
      }
    }

    expect(logCount).toBeGreaterThanOrEqual(expectedHeight * 4);
    expect(leafCount).toBeGreaterThan(0);
  });
});
