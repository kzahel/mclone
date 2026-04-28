import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { BoundingBox } from "../../../../src/world/level/levelgen/structure/bounding-box";
import { StructureTemplate } from "../../../../src/world/level/levelgen/structure/templatesystem/structure-template";
import { Mirror } from "../../../../src/world/level/block/mirror";
import { Rotation } from "../../../../src/world/level/block/rotation";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { GenerationStep } from "../../../../src/worldgen/levelgen/generation-step";
import { Heightmap } from "../../../../src/worldgen/levelgen/heightmap";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import { DecoratedDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-decorator-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { ChanceDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/chance-decorator-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { OVERWORLD_FOSSIL_CONFIGURATION } from "../../../../src/worldgen/levelgen/feature/fossil-feature-defaults";
import { UndergroundFeatures } from "../../../../src/worldgen/levelgen/feature/underground-features";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";
import { FOSSIL_TEMPLATE_DATA } from "../../../../src/worldgen/levelgen/feature/fossil-template-data";

const ROTATIONS = [Rotation.NONE, Rotation.CLOCKWISE_90, Rotation.CLOCKWISE_180, Rotation.COUNTERCLOCKWISE_90] as const;

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

function createSolidFossilLevel(airState: BlockState, stoneState: BlockState): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 96);
  for (let z = 0; z < 64; z++) {
    for (let x = 0; x < 64; x++) {
      for (let y = 0; y < 64; y++) {
        level.setBlock(new BlockPos(x, y, z), stoneState);
      }
    }
  }

  return level;
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

function getUndergroundStructureFeatures(biomeKey: string): readonly ConfiguredFeature<any, any>[] {
  return (getOverworldBiomeGenerationSettings(biomeKey).features()[GenerationStep.Decoration.UNDERGROUND_STRUCTURES] ?? []).map((feature) => feature());
}

function countBlocksInChunk(level: StaticRenderLevel, origin: BlockPos, predicate: (state: BlockState) => boolean): number {
  let count = 0;
  const minX = Math.floor(origin.getX() / 16) * 16;
  const minZ = Math.floor(origin.getZ() / 16) * 16;
  for (let y = level.getMinBuildHeight(); y < level.getMaxBuildHeight(); y++) {
    for (let z = minZ; z < minZ + 16; z++) {
      for (let x = minX; x < minX + 16; x++) {
        if (predicate(level.getBlockState(new BlockPos(x, y, z)))) {
          count++;
        }
      }
    }
  }

  return count;
}

function previewFossilBoundingBox(level: StaticRenderLevel, seed: bigint, origin: BlockPos): BoundingBox {
  const random = new WorldgenRandom(seed);
  const rotation = ROTATIONS[random.nextInt(ROTATIONS.length)]!;
  const templateIndex = random.nextInt(OVERWORLD_FOSSIL_CONFIGURATION.fossilStructures.length);
  const templateKey = OVERWORLD_FOSSIL_CONFIGURATION.fossilStructures[templateIndex]!.toString() as keyof typeof FOSSIL_TEMPLATE_DATA;
  const templateData = FOSSIL_TEMPLATE_DATA[templateKey];
  const [sizeX, sizeY, sizeZ] = templateData.size;
  const rotatedSizeX = rotation === Rotation.CLOCKWISE_90 || rotation === Rotation.COUNTERCLOCKWISE_90 ? sizeZ : sizeX;
  const rotatedSizeZ = rotation === Rotation.CLOCKWISE_90 || rotation === Rotation.COUNTERCLOCKWISE_90 ? sizeX : sizeZ;
  const offsetX = random.nextInt(16 - rotatedSizeX);
  const offsetZ = random.nextInt(16 - rotatedSizeZ);
  let minimumHeight = level.getMaxBuildHeight();

  for (let x = 0; x < rotatedSizeX; x++) {
    for (let z = 0; z < rotatedSizeZ; z++) {
      minimumHeight = Math.min(
        minimumHeight,
        level.getHeight(Heightmap.Types.OCEAN_FLOOR_WG, origin.getX() + x + offsetX, origin.getZ() + z + offsetZ),
      );
    }
  }

  const y = Math.max(minimumHeight - 15 - random.nextInt(10), level.getMinBuildHeight() + 10);
  const anchor = new BlockPos(origin.getX() + offsetX, y, origin.getZ() + offsetZ);
  const zeroPos = StructureTemplate.getZeroPositionWithTransform(anchor, Mirror.NONE, rotation, sizeX, sizeZ);
  const first = StructureTemplate.transform(BlockPos.ZERO, Mirror.NONE, rotation, BlockPos.ZERO);
  const second = StructureTemplate.transform(new BlockPos(sizeX - 1, sizeY - 1, sizeZ - 1), Mirror.NONE, rotation, BlockPos.ZERO);
  return BoundingBox.fromCorners(first, second).move(zeroPos);
}

function findSeedWithVisibleCoalAndBone(generator: NoiseBasedChunkGenerator, airState: BlockState, stoneState: BlockState, origin: BlockPos): bigint {
  const boneBlock = getState("minecraft:bone_block").getBlock();
  const coalOreBlock = getState("minecraft:coal_ore").getBlock();

  for (let seed = 0n; seed < 256n; seed++) {
    const level = createSolidFossilLevel(airState, stoneState);
    const placed = Features.FOSSIL.configured(OVERWORLD_FOSSIL_CONFIGURATION).place(level, generator, new WorldgenRandom(seed), origin);
    if (!placed) {
      continue;
    }

    const boneCount = countBlocksInChunk(level, origin, (state) => state.is(boneBlock));
    const coalCount = countBlocksInChunk(level, origin, (state) => state.is(coalOreBlock));
    if (boneCount > 0 && coalCount > 0) {
      return seed;
    }
  }

  throw new Error("Could not find a deterministic fossil seed with both bone and coal blocks");
}

describe("Fossil feature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("configured fossil matches the vanilla rarity path", () => {
    registerGeneratedRenderBlocks();

    const { current, decoratorConfigs } = unwrapConfiguredFeature(UndergroundFeatures.FOSSIL);
    const chanceConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is ChanceDecoratorConfiguration =>
      decoratorConfig instanceof ChanceDecoratorConfiguration
    );

    expect(current.feature).toBe(Features.FOSSIL);
    expect(current.config).toBe(OVERWORLD_FOSSIL_CONFIGURATION);
    expect(chanceConfig?.chance).toBe(64);
  });

  test("desert and swamp biome settings place fossils with the translated vanilla ordering", () => {
    registerGeneratedRenderBlocks();

    const desert = getUndergroundStructureFeatures("minecraft:desert").map((feature) => unwrapConfiguredFeature(feature).current.feature);
    const desertHills = getUndergroundStructureFeatures("minecraft:desert_hills").map((feature) => unwrapConfiguredFeature(feature).current.feature);
    const desertLakes = getUndergroundStructureFeatures("minecraft:desert_lakes").map((feature) => unwrapConfiguredFeature(feature).current.feature);
    const swamp = getUndergroundStructureFeatures("minecraft:swamp").map((feature) => unwrapConfiguredFeature(feature).current.feature);
    const swampHills = getUndergroundStructureFeatures("minecraft:swamp_hills").map((feature) => unwrapConfiguredFeature(feature).current.feature);

    expect(desert).toEqual([Features.FOSSIL, Features.MONSTER_ROOM]);
    expect(desertHills).toEqual([Features.MONSTER_ROOM]);
    expect(desertLakes).toEqual([Features.MONSTER_ROOM]);
    expect(swamp).toEqual([Features.FOSSIL, Features.MONSTER_ROOM]);
    expect(swampHills).toEqual([Features.MONSTER_ROOM, Features.FOSSIL]);
  });

  test("fossils place translated bone and coal blocks in a solid chunk and reject empty-corner boxes", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const boneBlock = getState("minecraft:bone_block").getBlock();
    const coalOreBlock = getState("minecraft:coal_ore").getBlock();
    const generator = createGenerator();
    const origin = new BlockPos(16, 0, 16);
    const seed = findSeedWithVisibleCoalAndBone(generator, blocks.airState, stoneState, origin);

    const placedLevel = createSolidFossilLevel(blocks.airState, stoneState);
    const placed = Features.FOSSIL.configured(OVERWORLD_FOSSIL_CONFIGURATION).place(placedLevel, generator, new WorldgenRandom(seed), origin);

    expect(placed).toBe(true);
    expect(countBlocksInChunk(placedLevel, origin, (state) => state.is(boneBlock))).toBeGreaterThan(0);
    expect(countBlocksInChunk(placedLevel, origin, (state) => state.is(coalOreBlock))).toBeGreaterThan(0);

    const rejectedLevel = createSolidFossilLevel(blocks.airState, stoneState);
    const rejectedBox = previewFossilBoundingBox(rejectedLevel, seed, origin);
    const rejectedCorners: BlockPos[] = [];
    rejectedBox.forAllCorners((pos) => {
      if (rejectedCorners.length < OVERWORLD_FOSSIL_CONFIGURATION.maxEmptyCornersAllowed + 1) {
        rejectedCorners.push(new BlockPos(pos.getX(), pos.getY(), pos.getZ()));
      }
    });
    for (const corner of rejectedCorners) {
      rejectedLevel.setBlock(corner, blocks.airState);
    }

    expect(Features.FOSSIL.configured(OVERWORLD_FOSSIL_CONFIGURATION).place(rejectedLevel, generator, new WorldgenRandom(seed), origin)).toBe(false);
  });
});
