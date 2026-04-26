import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Direction } from "../../../../src/core/direction";
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

  test("acacia placement uses the translated forking trunk and acacia foliage path", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const feature = TreeFeatures.ACACIA;
    const logState = getState("minecraft:acacia_log");
    const leavesState = getState("minecraft:acacia_leaves");
    const origin = new BlockPos(16, 11, 16);
    const expectedHeight = feature.config.trunkPlacer.getTreeHeight(new WorldgenRandom(2468n));

    expect(feature.place(level, generator, new WorldgenRandom(2468n), origin)).toBe(true);

    let logCount = 0;
    let leafCount = 0;
    let foundOffsetLog = false;
    for (let y = 10; y < 28; y++) {
      for (let z = 8; z <= 24; z++) {
        for (let x = 8; x <= 24; x++) {
          const state = level.getBlockState(new BlockPos(x, y, z));
          if (state.is(logState.getBlock())) {
            logCount++;
            if ((x !== origin.getX() || z !== origin.getZ()) && y > origin.getY()) {
              foundOffsetLog = true;
            }
          } else if (state.is(leavesState.getBlock())) {
            leafCount++;
            expect(state.getValue(BlockStateProperties.DISTANCE)).toBeLessThan(7);
          }
        }
      }
    }

    expect(level.getBlockState(origin).is(logState.getBlock())).toBe(true);
    expect(logCount).toBeGreaterThanOrEqual(expectedHeight);
    expect(foundOffsetLog).toBe(true);
    expect(leafCount).toBeGreaterThan(0);
  });

  test("mega spruce and mega pine placement use the translated giant-trunk, mega-pine foliage, and podzol ground decorator paths", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();
    const logState = getState("minecraft:spruce_log");
    const leavesState = getState("minecraft:spruce_leaves");
    const podzolState = getState("minecraft:podzol");

    for (const [feature, seed] of [
      [TreeFeatures.MEGA_SPRUCE, 97531n],
      [TreeFeatures.MEGA_PINE, 86420n],
    ] as const) {
      const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      const origin = new BlockPos(16, 11, 16);
      const expectedHeight = feature.config.trunkPlacer.getTreeHeight(new WorldgenRandom(seed));

      expect(feature.place(level, generator, new WorldgenRandom(seed), origin)).toBe(true);

      expect(level.getBlockState(origin).is(logState.getBlock())).toBe(true);
      expect(level.getBlockState(origin.east()).is(logState.getBlock())).toBe(true);
      expect(level.getBlockState(origin.south()).is(logState.getBlock())).toBe(true);
      expect(level.getBlockState(origin.east().south()).is(logState.getBlock())).toBe(true);

      let logCount = 0;
      let leafCount = 0;
      let podzolCount = 0;
      for (let y = 10; y < 40; y++) {
        for (let z = 6; z <= 26; z++) {
          for (let x = 6; x <= 26; x++) {
            const state = level.getBlockState(new BlockPos(x, y, z));
            if (state.is(logState.getBlock())) {
              logCount++;
            } else if (state.is(leavesState.getBlock())) {
              leafCount++;
              expect(state.getValue(BlockStateProperties.DISTANCE)).toBeLessThan(7);
            } else if (state.is(podzolState.getBlock())) {
              podzolCount++;
            }
          }
        }
      }

      expect(logCount).toBeGreaterThanOrEqual((expectedHeight * 2) + 1);
      expect(leafCount).toBeGreaterThan(0);
      expect(podzolCount).toBeGreaterThan(0);
    }
  });

  test("jungle placement uses the translated cocoa and vine decorators", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const feature = TreeFeatures.JUNGLE_TREE;
    const logState = getState("minecraft:jungle_log");
    const leavesState = getState("minecraft:jungle_leaves");
    const vineState = getState("minecraft:vine");
    const cocoaState = getState("minecraft:cocoa");
    const origin = new BlockPos(16, 11, 16);

    expect(feature.place(level, generator, new WorldgenRandom(16n), origin)).toBe(true);

    let logCount = 0;
    let leafCount = 0;
    let vineCount = 0;
    let cocoaCount = 0;
    for (let y = 10; y < 40; y++) {
      for (let z = 0; z < 32; z++) {
        for (let x = 0; x < 32; x++) {
          const state = level.getBlockState(new BlockPos(x, y, z));
          if (state.is(logState.getBlock())) {
            logCount++;
          } else if (state.is(leavesState.getBlock())) {
            leafCount++;
            expect(state.getValue(BlockStateProperties.DISTANCE)).toBeLessThan(7);
          } else if (state.is(vineState.getBlock())) {
            vineCount++;
          } else if (state.is(cocoaState.getBlock())) {
            cocoaCount++;
          }
        }
      }
    }

    expect(logCount).toBeGreaterThan(0);
    expect(leafCount).toBeGreaterThan(0);
    expect(vineCount).toBeGreaterThan(0);
    expect(cocoaCount).toBeGreaterThan(0);
  });

  test("swamp-oak placement uses the translated leaf-vine decorator", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();
    const feature = TreeFeatures.SWAMP_OAK;
    const vineState = getState("minecraft:vine");
    const origin = new BlockPos(16, 11, 16);

    let placedLevel: StaticRenderLevel | undefined;
    let vineCount = 0;
    for (let seed = 0n; seed < 512n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (!feature.place(candidate, generator, new WorldgenRandom(seed), origin)) {
        continue;
      }

      let candidateVineCount = 0;
      for (let y = 11; y < 32; y++) {
        for (let z = 0; z < 32; z++) {
          for (let x = 0; x < 32; x++) {
            if (candidate.getBlockState(new BlockPos(x, y, z)).is(vineState.getBlock())) {
              candidateVineCount++;
            }
          }
        }
      }

      if (candidateVineCount > 0) {
        placedLevel = candidate;
        vineCount = candidateVineCount;
        break;
      }
    }

    expect(placedLevel).toBeDefined();
    expect(vineCount).toBeGreaterThan(0);
  });

  test("bee-decorated oak placement can emit a translated bee nest block", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();
    const feature = TreeFeatures.OAK_BEES_005;
    const beeNestState = getState("minecraft:bee_nest");
    const origin = new BlockPos(16, 11, 16);

    let placedLevel: StaticRenderLevel | undefined;
    const beeNestPositions: BlockPos[] = [];

    for (let seed = 0n; seed < 512n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (!feature.place(candidate, generator, new WorldgenRandom(seed), origin)) {
        continue;
      }

      const candidateBeeNestPositions: BlockPos[] = [];
      for (let y = 11; y < 32; y++) {
        for (let z = 0; z < 32; z++) {
          for (let x = 0; x < 32; x++) {
            const pos = new BlockPos(x, y, z);
            if (candidate.getBlockState(pos).is(beeNestState.getBlock())) {
              candidateBeeNestPositions.push(pos);
            }
          }
        }
      }

      if (candidateBeeNestPositions.length > 0) {
        placedLevel = candidate;
        beeNestPositions.push(...candidateBeeNestPositions);
        break;
      }
    }

    expect(placedLevel).toBeDefined();
    expect(beeNestPositions).toHaveLength(1);

    const placedBeeNest = placedLevel!.getBlockState(beeNestPositions[0]!);
    expect(placedBeeNest.getValue(BlockStateProperties.HORIZONTAL_FACING)).toBe(Direction.SOUTH);
    expect(placedBeeNest.getValue(BlockStateProperties.LEVEL_HONEY)).toBe(0);
  });
});
