import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { Fluids } from "../../../../src/world/level/material/fluids";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { BlockStateConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/block-state-configuration";
import { RandomPatchConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/random-patch-configuration";
import { SpringConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/spring-configuration";
import { SimpleBlockPlacer } from "../../../../src/worldgen/levelgen/feature/blockplacers/simple-block-placer";
import { SimpleStateProvider } from "../../../../src/worldgen/levelgen/feature/stateproviders/simple-state-provider";
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

function createStoneLevel(airState: BlockState, stoneState: BlockState): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 96);
  for (let y = 0; y <= 24; y++) {
    for (let z = 0; z < 64; z++) {
      for (let x = 0; x < 64; x++) {
        level.setBlock(new BlockPos(x, y, z), stoneState);
      }
    }
  }

  return level;
}

describe("Water features", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("lake feature carves a translated water pocket out of solid stone", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const waterState = getState("minecraft:water");
    const level = createStoneLevel(blocks.airState, stoneState);
    const generator = createGenerator();
    const feature = Features.LAKE.configured(new BlockStateConfiguration(waterState));

    expect(feature.place(level, generator, new WorldgenRandom(1234n), new BlockPos(32, 30, 32))).toBe(true);

    let waterCount = 0;
    let carvedAirCount = 0;
    for (let y = 0; y < 32; y++) {
      for (let z = 24; z < 40; z++) {
        for (let x = 24; x < 40; x++) {
          const state = level.getBlockState(new BlockPos(x, y, z));
          if (state.is(waterState.getBlock())) {
            waterCount++;
          } else if (state.isAir() && y <= 24) {
            carvedAirCount++;
          }
        }
      }
    }

    expect(waterCount).toBeGreaterThan(0);
    expect(carvedAirCount).toBeGreaterThan(0);
  });

  test("spring feature places a translated water source when the rock and hole counts match", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const waterState = getState("minecraft:water");
    const level = createStoneLevel(blocks.airState, stoneState);
    const generator = createGenerator();
    const feature = Features.SPRING.configured(
      new SpringConfiguration(Fluids.WATER.defaultFluidState(), true, 4, 1, new Set([stoneState.getBlock()])),
    );
    const origin = new BlockPos(16, 16, 16);

    level.setBlock(origin, blocks.airState);
    level.setBlock(origin.south(), blocks.airState);

    expect(feature.place(level, generator, new WorldgenRandom(1n), origin)).toBe(true);
    expect(level.getBlockState(origin).is(waterState.getBlock())).toBe(true);
  });

  test("waterlily vegetation survives on translated surface water", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const waterState = getState("minecraft:water");
    const lilyPadState = getState("minecraft:lily_pad");
    const level = createStoneLevel(blocks.airState, stoneState);
    const generator = createGenerator();

    level.setBlock(new BlockPos(8, 10, 8), waterState);
    level.setBlock(new BlockPos(8, 11, 8), blocks.airState);

    expect(lilyPadState.canSurvive(level, new BlockPos(8, 11, 8))).toBe(true);
    expect(
      Features.RANDOM_PATCH.configured(
        new RandomPatchConfiguration(
          new SimpleStateProvider(lilyPadState),
          SimpleBlockPlacer.INSTANCE,
          new Set(),
          new Set(),
          1,
          0,
          0,
          0,
          false,
          false,
          false,
        ),
      ).place(level, generator, new WorldgenRandom(2n), new BlockPos(8, 11, 8)),
    ).toBe(true);
    expect(level.getBlockState(new BlockPos(8, 11, 8)).is(lilyPadState.getBlock())).toBe(true);
  });
});
