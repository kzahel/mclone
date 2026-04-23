import { BlockPos } from "../../core/block-pos";
import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { GeneratedRenderBlockPalette } from "../../world/level/generated-render-blocks";
import { ChunkBlockId } from "../chunk/chunk-block-buffer";
import { WorldgenRandom } from "../prng/worldgen-random";
import { Heightmap } from "./heightmap";
import { NoiseBasedChunkGenerator } from "./noise-based-chunk-generator";
import { SimpleBlockPlacer } from "./feature/blockplacers/simple-block-placer";
import { CountConfiguration } from "./feature/configurations/count-configuration";
import type { DecoratorConfiguration } from "./feature/configurations/decorator-configuration";
import { HeightmapConfiguration } from "./feature/configurations/heightmap-configuration";
import { NoneDecoratorConfiguration } from "./feature/configurations/none-decorator-configuration";
import { RandomPatchConfiguration } from "./feature/configurations/random-patch-configuration";
import { SimpleBlockConfiguration } from "./feature/configurations/simple-block-configuration";
import { Features } from "./feature/features";
import { SimpleStateProvider } from "./feature/stateproviders/simple-state-provider";
import { TreeFeatures } from "./feature/tree-features";
import type { ConfiguredDecorator } from "./placement/configured-decorator";
import { FeatureDecorators } from "./placement/feature-decorators";

const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const DANDELION_LOCATION = new ResourceLocation("minecraft:dandelion");
const OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:oak_sapling");
const CACTUS_LOCATION = new ResourceLocation("minecraft:cactus");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");

function getRegisteredBlockState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function findSurfaceY(level: GeneratedRenderLevel, x: number, z: number): number {
  return level.getHeight(Heightmap.Types.WORLD_SURFACE_WG, x, z) - 1;
}

function placeOakTree(level: GeneratedRenderLevel, generator: NoiseBasedChunkGenerator, levelSeed: bigint, x: number, z: number, featureIndex: number): void {
  const groundY = findSurfaceY(level, x, z);
  TreeFeatures.OAK.place(level, generator, createFeatureRandom(levelSeed, new BlockPos(x, 0, z), featureIndex), new BlockPos(x, groundY + 1, z));
}

function createCountSquareHeightmapDecorator(count: number) {
  let decorator: ConfiguredDecorator<DecoratorConfiguration> = FeatureDecorators.HEIGHTMAP.configured(
    new HeightmapConfiguration(Heightmap.Types.WORLD_SURFACE_WG),
  );
  decorator = decorator.decorated(FeatureDecorators.SQUARE.configured(NoneDecoratorConfiguration.INSTANCE));
  decorator = decorator.decorated(FeatureDecorators.COUNT.configured(new CountConfiguration(count)));
  return decorator;
}

function createRandomPatchFeature(state: BlockState, count: number, tries: number, xspread: number, zspread: number) {
  return Features.RANDOM_PATCH.configured(
    new RandomPatchConfiguration(
      new SimpleStateProvider(state),
      SimpleBlockPlacer.INSTANCE,
      new Set(),
      new Set(),
      tries,
      xspread,
      2,
      zspread,
      false,
      false,
      false,
    ),
  ).decorated(createCountSquareHeightmapDecorator(count));
}

function createFeatureRandom(levelSeed: bigint, origin: BlockPos, featureIndex: number): WorldgenRandom {
  const random = new WorldgenRandom(0n);
  const decorationSeed = random.setDecorationSeed(levelSeed, origin.getX(), origin.getZ());
  random.setFeatureSeed(decorationSeed, featureIndex, 0);
  return random;
}

function placeConfiguredFeature(
  level: GeneratedRenderLevel,
  generator: NoiseBasedChunkGenerator,
  levelSeed: bigint,
  feature: ReturnType<typeof createRandomPatchFeature>,
  origin: BlockPos,
  featureIndex: number,
): void {
  feature.place(level, generator, createFeatureRandom(levelSeed, origin, featureIndex), origin);
}

function placeSimpleColumnFeature(
  level: GeneratedRenderLevel,
  generator: NoiseBasedChunkGenerator,
  levelSeed: bigint,
  state: BlockState,
  x: number,
  z: number,
  height: number,
  featureIndex: number,
): void {
  const feature = Features.SIMPLE_BLOCK.configured(new SimpleBlockConfiguration(new SimpleStateProvider(state)));
  const random = createFeatureRandom(levelSeed, new BlockPos(x, 0, z), featureIndex);
  const groundY = findSurfaceY(level, x, z);
  for (let offsetY = 0; offsetY < height; offsetY++) {
    feature.place(level, generator, random, new BlockPos(x, groundY + 1 + offsetY, z));
  }
}

export function decorateSmokeSceneWithFeatures(
  level: GeneratedRenderLevel,
  generatedBlocks: GeneratedRenderBlockPalette,
  generator: NoiseBasedChunkGenerator,
  levelSeed: bigint,
): void {
  const waterState = generatedBlocks.blockStateById[ChunkBlockId.WATER]!;
  const snowState = generatedBlocks.blockStateById[ChunkBlockId.SNOW]!;
  const sandState = generatedBlocks.blockStateById[ChunkBlockId.SAND]!;
  const grassPlantState = getRegisteredBlockState(GRASS_LOCATION);
  const fernState = getRegisteredBlockState(FERN_LOCATION);
  const dandelionState = getRegisteredBlockState(DANDELION_LOCATION);
  const oakSaplingState = getRegisteredBlockState(OAK_SAPLING_LOCATION);
  const cactusState = getRegisteredBlockState(CACTUS_LOCATION);
  const sugarCaneState = getRegisteredBlockState(SUGAR_CANE_LOCATION);

  for (let z = 35; z <= 39; z++) {
    for (let x = 42; x <= 47; x++) {
      level.setBlock(new BlockPos(x, 84, z), waterState);
    }
  }

  for (let z = 30; z <= 34; z++) {
    for (let x = 41; x <= 45; x++) {
      level.setBlock(new BlockPos(x, 84, z), snowState);
    }
  }

  placeOakTree(level, generator, levelSeed, 26, 37, 0);
  placeOakTree(level, generator, levelSeed, 33, 34, 1);

  placeConfiguredFeature(level, generator, levelSeed, createRandomPatchFeature(grassPlantState, 4, 36, 5, 5), new BlockPos(16, 0, 32), 2);
  placeConfiguredFeature(level, generator, levelSeed, createRandomPatchFeature(fernState, 3, 24, 5, 5), new BlockPos(16, 0, 32), 3);
  placeConfiguredFeature(level, generator, levelSeed, createRandomPatchFeature(dandelionState, 2, 18, 4, 4), new BlockPos(24, 0, 32), 4);
  placeConfiguredFeature(level, generator, levelSeed, createRandomPatchFeature(oakSaplingState, 1, 10, 4, 4), new BlockPos(24, 0, 32), 5);

  for (const [x, z, height, featureIndex] of [
    [41, 39, 2, 6],
    [46, 37, 2, 7],
  ] as const) {
    const groundY = findSurfaceY(level, x, z);
    level.setBlock(new BlockPos(x, groundY, z), sandState);
    placeSimpleColumnFeature(level, generator, levelSeed, sugarCaneState, x, z, height, featureIndex);
  }

  for (const [x, z, height, featureIndex] of [
    [45, 44, 3, 8],
    [47, 43, 2, 9],
  ] as const) {
    const groundY = findSurfaceY(level, x, z);
    level.setBlock(new BlockPos(x, groundY, z), sandState);
    placeSimpleColumnFeature(level, generator, levelSeed, cactusState, x, z, height, featureIndex);
  }
}
