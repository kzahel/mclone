import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "./configurations/frequency-with-extra-chance-decorator-configuration";
import { HeightmapConfiguration } from "./configurations/heightmap-configuration";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { WaterDepthThresholdConfiguration } from "./configurations/water-depth-threshold-configuration";
import { SimpleBlockPlacer } from "./blockplacers/simple-block-placer";
import { Features } from "./features";
import { SimpleStateProvider } from "./stateproviders/simple-state-provider";
import { WeightedStateProvider } from "./stateproviders/weighted-state-provider";
import { TreeFeatures } from "./tree-features";
import { FeatureDecorators } from "../placement/feature-decorators";

const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function heightmapDecorator(type: Heightmap.Types) {
  return FeatureDecorators.HEIGHTMAP.configured(new HeightmapConfiguration(type));
}

function heightmapDoubleDecorator(type: Heightmap.Types) {
  return FeatureDecorators.HEIGHTMAP_SPREAD_DOUBLE.configured(new HeightmapConfiguration(type));
}

function heightmapWithTreeThreshold() {
  return heightmapDecorator(Heightmap.Types.OCEAN_FLOOR).decorated(
    FeatureDecorators.WATER_DEPTH_THRESHOLD.configured(new WaterDepthThresholdConfiguration(0)),
  );
}

function heightmapWithTreeThresholdSquared() {
  return heightmapWithTreeThreshold().squared();
}

function heightmapDoubleSquare() {
  return heightmapDoubleDecorator(Heightmap.Types.MOTION_BLOCKING).squared();
}

function createDefaultGrassConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(GRASS_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(32)
    .build();
}

function createTaigaGrassConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new WeightedStateProvider([
      { state: getRequiredState(GRASS_LOCATION), weight: 1 },
      { state: getRequiredState(FERN_LOCATION), weight: 4 },
    ]),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(32)
    .build();
}

export class VegetationFeatures {
  public static get PATCH_GRASS_BADLANDS() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_GRASS_TAIGA_2() {
    return Features.RANDOM_PATCH.configured(createTaigaGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get TAIGA_VEGETATION() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.PINE.weighted(0.33333334)], TreeFeatures.SPRUCE),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(FeatureDecorators.COUNT_EXTRA.configured(new FrequencyWithExtraChanceDecoratorConfiguration(10, 0.1, 1)));
  }

  public static get TREES_MOUNTAIN() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.SPRUCE.weighted(0.666), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(FeatureDecorators.COUNT_EXTRA.configured(new FrequencyWithExtraChanceDecoratorConfiguration(0, 0.1, 1)));
  }

  public static get TREES_MOUNTAIN_EDGE() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.SPRUCE.weighted(0.666), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(FeatureDecorators.COUNT_EXTRA.configured(new FrequencyWithExtraChanceDecoratorConfiguration(3, 0.1, 1)));
  }
}
