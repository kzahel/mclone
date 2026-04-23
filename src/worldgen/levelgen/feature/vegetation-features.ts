import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import { BiasedToBottomInt } from "../../../util/valueproviders/biased-to-bottom-int";
import type { Block } from "../../../world/level/block/block";
import { SweetBerryBushBlock } from "../../../world/level/block/sweet-berry-bush-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "./configurations/frequency-with-extra-chance-decorator-configuration";
import { HeightmapConfiguration } from "./configurations/heightmap-configuration";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { WaterDepthThresholdConfiguration } from "./configurations/water-depth-threshold-configuration";
import { ColumnPlacer } from "./blockplacers/column-placer";
import { DoublePlantPlacer } from "./blockplacers/double-plant-placer";
import { SimpleBlockPlacer } from "./blockplacers/simple-block-placer";
import { Features } from "./features";
import { SimpleStateProvider } from "./stateproviders/simple-state-provider";
import { WeightedStateProvider } from "./stateproviders/weighted-state-provider";
import { TreeFeatures } from "./tree-features";
import { FeatureDecorators } from "../placement/feature-decorators";
import { NoneDecoratorConfiguration } from "./configurations/none-decorator-configuration";

const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const LARGE_FERN_LOCATION = new ResourceLocation("minecraft:large_fern");
const SWEET_BERRY_BUSH_LOCATION = new ResourceLocation("minecraft:sweet_berry_bush");
const BROWN_MUSHROOM_LOCATION = new ResourceLocation("minecraft:brown_mushroom");
const RED_MUSHROOM_LOCATION = new ResourceLocation("minecraft:red_mushroom");
const PUMPKIN_LOCATION = new ResourceLocation("minecraft:pumpkin");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");
const CACTUS_LOCATION = new ResourceLocation("minecraft:cactus");

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

function heightmapSquare() {
  return heightmapDecorator(Heightmap.Types.MOTION_BLOCKING).squared();
}

function spread32AboveDecorator() {
  return FeatureDecorators.SPREAD_32_ABOVE.configured(NoneDecoratorConfiguration.INSTANCE);
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

function createBrownMushroomConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(BROWN_MUSHROOM_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .noProjection()
    .build();
}

function createRedMushroomConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(RED_MUSHROOM_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .noProjection()
    .build();
}

function createLargeFernConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(LARGE_FERN_LOCATION)),
    DoublePlantPlacer.INSTANCE,
  )
    .triesCount(64)
    .noProjection()
    .build();
}

function createSweetBerryBushConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(SWEET_BERRY_BUSH_LOCATION).setValue(SweetBerryBushBlock.AGE, 3)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .whitelistSet(new Set([getRequiredState(GRASS_BLOCK_LOCATION).getBlock()]))
    .noProjection()
    .build();
}

function createPumpkinConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(PUMPKIN_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .whitelistSet(new Set([getRequiredState(GRASS_BLOCK_LOCATION).getBlock()]))
    .noProjection()
    .build();
}

function createSugarCaneConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(SUGAR_CANE_LOCATION)),
    new ColumnPlacer(BiasedToBottomInt.of(2, 4)),
  )
    .triesCount(20)
    .xspreadCount(4)
    .yspreadCount(0)
    .zspreadCount(4)
    .noProjection()
    .needWaterAdjacency()
    .build();
}

function createCactusConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(CACTUS_LOCATION)),
    new ColumnPlacer(BiasedToBottomInt.of(1, 3)),
  )
    .triesCount(10)
    .noProjection()
    .build();
}

export class VegetationFeatures {
  public static get BROWN_MUSHROOM_NORMAL() {
    return Features.RANDOM_PATCH.configured(createBrownMushroomConfig()).decorated(heightmapDoubleSquare()).rarity(4);
  }

  public static get BROWN_MUSHROOM_TAIGA() {
    return Features.RANDOM_PATCH.configured(createBrownMushroomConfig()).rarity(4).decorated(heightmapSquare());
  }

  public static get PATCH_BERRY_DECORATED() {
    return Features.RANDOM_PATCH.configured(createSweetBerryBushConfig()).decorated(heightmapDoubleSquare()).rarity(12);
  }

  public static get PATCH_BERRY_SPARSE() {
    return Features.RANDOM_PATCH.configured(createSweetBerryBushConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_CACTUS() {
    return Features.RANDOM_PATCH.configured(createCactusConfig());
  }

  public static get PATCH_CACTUS_DECORATED() {
    return VegetationFeatures.PATCH_CACTUS.decorated(heightmapDoubleSquare()).count(5);
  }

  public static get PATCH_CACTUS_DESERT() {
    return VegetationFeatures.PATCH_CACTUS.decorated(heightmapDoubleSquare()).count(10);
  }

  public static get PATCH_GRASS_BADLANDS() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_GRASS_TAIGA_2() {
    return Features.RANDOM_PATCH.configured(createTaigaGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_LARGE_FERN() {
    return Features.RANDOM_PATCH.configured(createLargeFernConfig()).decorated(spread32AboveDecorator()).decorated(heightmapSquare()).count(7);
  }

  public static get PATCH_PUMPKIN() {
    return Features.RANDOM_PATCH.configured(createPumpkinConfig()).decorated(heightmapDoubleSquare()).rarity(32);
  }

  public static get PATCH_SUGAR_CANE() {
    return Features.RANDOM_PATCH.configured(createSugarCaneConfig()).decorated(heightmapDoubleSquare()).count(10);
  }

  public static get PATCH_SUGAR_CANE_BADLANDS() {
    return Features.RANDOM_PATCH.configured(createSugarCaneConfig()).decorated(heightmapDoubleSquare()).count(13);
  }

  public static get PATCH_SUGAR_CANE_DESERT() {
    return Features.RANDOM_PATCH.configured(createSugarCaneConfig()).decorated(heightmapDoubleSquare()).count(60);
  }

  public static get PATCH_SUGAR_CANE_SWAMP() {
    return Features.RANDOM_PATCH.configured(createSugarCaneConfig()).decorated(heightmapDoubleSquare()).count(20);
  }

  public static get RED_MUSHROOM_NORMAL() {
    return Features.RANDOM_PATCH.configured(createRedMushroomConfig()).decorated(heightmapDoubleSquare()).rarity(8);
  }

  public static get RED_MUSHROOM_TAIGA() {
    return Features.RANDOM_PATCH.configured(createRedMushroomConfig()).rarity(8).decorated(heightmapDoubleSquare());
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
