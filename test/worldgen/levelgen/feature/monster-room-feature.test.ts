import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { GenerationStep } from "../../../../src/worldgen/levelgen/generation-step";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import { DecoratedDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-decorator-configuration";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { NoneDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-decorator-configuration";
import { NoneFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-feature-configuration";
import { RangeDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/range-decorator-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { UndergroundFeatures } from "../../../../src/worldgen/levelgen/feature/underground-features";
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

function collectDecoratorConfigs(config: DecoratorConfiguration, target: DecoratorConfiguration[]): void {
  target.push(config);
  if (config instanceof DecoratedDecoratorConfiguration) {
    collectDecoratorConfigs(config.outer().config(), target);
    collectDecoratorConfigs(config.inner().config(), target);
  }
}

function unwrapConfiguredFeature(feature: ConfiguredFeature<any, any>): {
  readonly current: ConfiguredFeature<any, any>;
  readonly decoratorConfigs: readonly DecoratorConfiguration[];
} {
  const decoratorConfigs: DecoratorConfiguration[] = [];
  let current = feature;

  while (current.config instanceof DecoratedFeatureConfiguration) {
    collectDecoratorConfigs(current.config.decorator.config(), decoratorConfigs);
    current = current.config.feature();
  }

  return { current, decoratorConfigs };
}

function createEnclosedRoom(
  airState: BlockState,
  solidState: BlockState,
  origin: BlockPos,
  roomWidth: number,
  roomDepth: number,
): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 48);
  const minX = -roomWidth - 1;
  const maxX = roomWidth + 1;
  const minZ = -roomDepth - 1;
  const maxZ = roomDepth + 1;

  for (let offsetX = minX; offsetX <= maxX; offsetX++) {
    for (let offsetY = -1; offsetY <= 4; offsetY++) {
      for (let offsetZ = minZ; offsetZ <= maxZ; offsetZ++) {
        level.setBlock(origin.offset(offsetX, offsetY, offsetZ), solidState);
      }
    }
  }

  return level;
}

function countBlocksInBox(
  level: StaticRenderLevel,
  origin: BlockPos,
  radiusX: number,
  radiusZ: number,
  predicate: (state: BlockState) => boolean,
): number {
  let count = 0;
  for (let offsetX = -radiusX; offsetX <= radiusX; offsetX++) {
    for (let offsetY = -1; offsetY <= 3; offsetY++) {
      for (let offsetZ = -radiusZ; offsetZ <= radiusZ; offsetZ++) {
        if (predicate(level.getBlockState(origin.offset(offsetX, offsetY, offsetZ)))) {
          count++;
        }
      }
    }
  }

  return count;
}

describe("Monster room feature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("configured monster room matches the vanilla full-range square count chain", () => {
    registerGeneratedRenderBlocks();

    const { current, decoratorConfigs } = unwrapConfiguredFeature(UndergroundFeatures.MONSTER_ROOM);
    const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
      decoratorConfig instanceof CountConfiguration
    );

    expect(current.feature).toBe(Features.MONSTER_ROOM);
    expect(countConfig?.count().sample(new WorldgenRandom(0n))).toBe(8);
    expect(decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration)).toBe(true);
    expect(decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration)).toBe(true);
  });

  test("plains, mushroom fields, and oceans all carry the translated underground monster room feature", () => {
    registerGeneratedRenderBlocks();

    const plains = getOverworldBiomeGenerationSettings("minecraft:plains").features()[GenerationStep.Decoration.UNDERGROUND_STRUCTURES] ?? [];
    const mushroomFields = getOverworldBiomeGenerationSettings("minecraft:mushroom_fields").features()[GenerationStep.Decoration.UNDERGROUND_STRUCTURES] ?? [];
    const ocean = getOverworldBiomeGenerationSettings("minecraft:ocean").features()[GenerationStep.Decoration.UNDERGROUND_STRUCTURES] ?? [];

    expect(plains.map((feature) => unwrapConfiguredFeature(feature()).current.feature)).toContain(Features.MONSTER_ROOM);
    expect(mushroomFields.map((feature) => unwrapConfiguredFeature(feature()).current.feature)).toContain(Features.MONSTER_ROOM);
    expect(ocean.map((feature) => unwrapConfiguredFeature(feature()).current.feature)).toContain(Features.MONSTER_ROOM);
  });

  test("monster room carves a room, keeps the doorway, and places translated dungeon blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const caveAirState = getState("minecraft:cave_air");
    const chestState = getState("minecraft:chest");
    const spawnerState = getState("minecraft:spawner");
    const cobblestoneBlock = getState("minecraft:cobblestone").getBlock();
    const mossyCobblestoneBlock = getState("minecraft:mossy_cobblestone").getBlock();
    const generator = createGenerator();
    const origin = new BlockPos(16, 16, 16);
    const seed = 12345n;
    const preview = new WorldgenRandom(seed);
    const roomWidth = preview.nextInt(2) + 2;
    const roomDepth = preview.nextInt(2) + 2;
    const level = createEnclosedRoom(blocks.airState, stoneState, origin, roomWidth, roomDepth);

    level.setBlock(origin.offset(-roomWidth - 1, 0, 0), blocks.airState);
    level.setBlock(origin.offset(-roomWidth - 1, 1, 0), blocks.airState);

    const placed = Features.MONSTER_ROOM.configured(NoneFeatureConfiguration.INSTANCE).place(level, generator, new WorldgenRandom(seed), origin);

    expect(placed).toBe(true);
    expect(level.getBlockState(origin)).toBe(spawnerState);
    expect(level.getBlockState(origin.above())).toBe(caveAirState);
    expect(level.getBlockState(origin.offset(-roomWidth - 1, 0, 0))).toBe(blocks.airState);

    const cobbleCount = countBlocksInBox(level, origin, roomWidth + 1, roomDepth + 1, (state) =>
      state.is(cobblestoneBlock) || state.is(mossyCobblestoneBlock)
    );
    const chestCount = countBlocksInBox(level, origin, roomWidth, roomDepth, (state) => state.is(chestState.getBlock()));

    expect(cobbleCount).toBeGreaterThan(0);
    expect(chestCount).toBeLessThanOrEqual(2);
  });

  test("monster room rejects fully sealed stone boxes with no valid wall openings", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const generator = createGenerator();
    const origin = new BlockPos(16, 16, 16);
    const seed = 12345n;
    const preview = new WorldgenRandom(seed);
    const roomWidth = preview.nextInt(2) + 2;
    const roomDepth = preview.nextInt(2) + 2;
    const level = createEnclosedRoom(blocks.airState, stoneState, origin, roomWidth, roomDepth);

    const placed = Features.MONSTER_ROOM.configured(NoneFeatureConfiguration.INSTANCE).place(level, generator, new WorldgenRandom(seed), origin);

    expect(placed).toBe(false);
    expect(level.getBlockState(origin)).toBe(stoneState);
  });
});
