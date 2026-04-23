import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import { SnowyDirtBlock } from "../../../../src/world/level/block/snowy-dirt-block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { getLayeredBiomeByKey } from "../../../../src/worldgen/biome/biome-data";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../../../src/worldgen/chunk/chunk-block-buffer";
import { DiskConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/disk-configuration";
import { NoneFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-feature-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { getOverworldSurfaceTopMaterial } from "../../../../src/worldgen/surface/surface-builders";
import { UniformInt } from "../../../../src/util/valueproviders/uniform-int";
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

function createSurfaceLevel(
  airState: BlockState,
  belowState: BlockState,
  surfaceState: BlockState,
  options: {
    readonly biomeKey?: string;
    readonly blockLight?: number;
  } = {},
): StaticRenderLevel {
  const level = new StaticRenderLevel(
    airState,
    15,
    options.blockLight ?? 0,
    0,
    64,
    undefined,
    undefined,
    undefined,
    getLayeredBiomeByKey(options.biomeKey ?? "minecraft:snowy_tundra"),
  );

  for (let z = 0; z < 32; z++) {
    for (let x = 0; x < 32; x++) {
      for (let y = 0; y < 10; y++) {
        level.setBlock(new BlockPos(x, y, z), belowState);
      }
      level.setBlock(new BlockPos(x, 10, z), surfaceState);
    }
  }

  return level;
}

describe("Cold surface features", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("freeze-top-layer places translated snow and ice and updates snowy dirt", () => {
    const blocks = registerGeneratedRenderBlocks();
    const dirtState = getState("minecraft:dirt");
    const grassState = getState("minecraft:grass_block");
    const waterState = getState("minecraft:water");
    const snowState = getState("minecraft:snow");
    const iceState = getState("minecraft:ice");
    const level = createSurfaceLevel(blocks.airState, dirtState, grassState, {
      biomeKey: "minecraft:snowy_tundra",
      blockLight: 0,
    });
    const generator = createGenerator();
    const feature = Features.FREEZE_TOP_LAYER.configured(NoneFeatureConfiguration.INSTANCE);

    level.setBlock(new BlockPos(4, 10, 4), waterState);

    expect(feature.place(level, generator, new WorldgenRandom(1234n), new BlockPos(0, 0, 0))).toBe(true);
    expect(level.getBlockState(new BlockPos(0, 11, 0)).is(snowState.getBlock())).toBe(true);
    expect(level.getBlockState(new BlockPos(0, 10, 0)).getValue(SnowyDirtBlock.SNOWY)).toBe(true);
    expect(level.getBlockState(new BlockPos(4, 10, 4)).is(iceState.getBlock())).toBe(true);
    expect(level.getBlockState(new BlockPos(4, 11, 4)).is(snowState.getBlock())).toBe(false);
  });

  test("ice patch uses the translated disk placement path on snow-block terrain", () => {
    const blocks = registerGeneratedRenderBlocks();
    const dirtState = getState("minecraft:dirt");
    const grassState = getState("minecraft:grass_block");
    const podzolState = getState("minecraft:podzol");
    const coarseDirtState = getState("minecraft:coarse_dirt");
    const myceliumState = getState("minecraft:mycelium");
    const snowBlockState = getState("minecraft:snow_block");
    const iceState = getState("minecraft:ice");
    const packedIceState = getState("minecraft:packed_ice");
    const level = createSurfaceLevel(blocks.airState, dirtState, snowBlockState, {
      biomeKey: "minecraft:ice_spikes",
      blockLight: 0,
    });
    const generator = createGenerator();
    const feature = Features.ICE_PATCH.configured(
      new DiskConfiguration(packedIceState, UniformInt.of(2, 3), 1, [
        dirtState,
        grassState,
        podzolState,
        coarseDirtState,
        myceliumState,
        snowBlockState,
        iceState,
      ]),
    );

    expect(feature.place(level, generator, new WorldgenRandom(777n), new BlockPos(16, 11, 16))).toBe(true);

    let packedIceCount = 0;
    for (let y = 9; y <= 10; y++) {
      for (let z = 10; z <= 22; z++) {
        for (let x = 10; x <= 22; x++) {
          if (level.getBlockState(new BlockPos(x, y, z)).is(packedIceState.getBlock())) {
            packedIceCount++;
          }
        }
      }
    }

    expect(packedIceCount).toBeGreaterThan(0);
  });

  test("ice spike uses the translated packed-ice spike path", () => {
    const blocks = registerGeneratedRenderBlocks();
    const dirtState = getState("minecraft:dirt");
    const snowBlockState = getState("minecraft:snow_block");
    const packedIceState = getState("minecraft:packed_ice");
    const level = createSurfaceLevel(blocks.airState, dirtState, snowBlockState, {
      biomeKey: "minecraft:ice_spikes",
      blockLight: 0,
    });
    const generator = createGenerator();
    const feature = Features.ICE_SPIKE.configured(NoneFeatureConfiguration.INSTANCE);

    expect(feature.place(level, generator, new WorldgenRandom(2468n), new BlockPos(16, 11, 16))).toBe(true);

    let packedIceCount = 0;
    let highestPackedIceY = 0;
    for (let y = 10; y < 48; y++) {
      for (let z = 8; z <= 24; z++) {
        for (let x = 8; x <= 24; x++) {
          if (!level.getBlockState(new BlockPos(x, y, z)).is(packedIceState.getBlock())) {
            continue;
          }

          packedIceCount++;
          highestPackedIceY = Math.max(highestPackedIceY, y);
        }
      }
    }

    expect(packedIceCount).toBeGreaterThan(0);
    expect(highestPackedIceY).toBeGreaterThan(16);
  });

  test("snowy beach and ice spikes now resolve to the translated cold-surface top materials", () => {
    registerGeneratedRenderBlocks();

    expect(getOverworldSurfaceTopMaterial(getLayeredBiomeByKey("minecraft:snowy_beach"))).toBe(ChunkBlockId.SAND);
    expect(getOverworldSurfaceTopMaterial(getLayeredBiomeByKey("minecraft:ice_spikes"))).toBe(ChunkBlockId.SNOW_BLOCK);
  });
});
