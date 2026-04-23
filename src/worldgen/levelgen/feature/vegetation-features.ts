import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import { BiasedToBottomInt } from "../../../util/valueproviders/biased-to-bottom-int";
import { ClampedInt } from "../../../util/valueproviders/clamped-int";
import { UniformInt } from "../../../util/valueproviders/uniform-int";
import type { Block } from "../../../world/level/block/block";
import { HugeMushroomBlock } from "../../../world/level/block/huge-mushroom-block";
import { SweetBerryBushBlock } from "../../../world/level/block/sweet-berry-bush-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "./configurations/frequency-with-extra-chance-decorator-configuration";
import { HeightmapConfiguration } from "./configurations/heightmap-configuration";
import { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";
import { NoiseDependantDecoratorConfiguration } from "./configurations/noise-dependant-decorator-configuration";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";
import { ProbabilityFeatureConfiguration } from "./configurations/probability-feature-configuration";
import { RandomBooleanFeatureConfiguration } from "./configurations/random-boolean-feature-configuration";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { SimpleRandomFeatureConfiguration } from "./configurations/simple-random-feature-configuration";
import { WaterDepthThresholdConfiguration } from "./configurations/water-depth-threshold-configuration";
import { ColumnPlacer } from "./blockplacers/column-placer";
import { DoublePlantPlacer } from "./blockplacers/double-plant-placer";
import { SimpleBlockPlacer } from "./blockplacers/simple-block-placer";
import { Features } from "./features";
import { ForestFlowerProvider } from "./stateproviders/forest-flower-provider";
import { PlainFlowerProvider } from "./stateproviders/plain-flower-provider";
import { SimpleStateProvider } from "./stateproviders/simple-state-provider";
import { WeightedStateProvider } from "./stateproviders/weighted-state-provider";
import { TreeFeatures } from "./tree-features";
import { FeatureDecorators } from "../placement/feature-decorators";
import { NoneDecoratorConfiguration } from "./configurations/none-decorator-configuration";

const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const DANDELION_LOCATION = new ResourceLocation("minecraft:dandelion");
const LARGE_FERN_LOCATION = new ResourceLocation("minecraft:large_fern");
const SWEET_BERRY_BUSH_LOCATION = new ResourceLocation("minecraft:sweet_berry_bush");
const BROWN_MUSHROOM_LOCATION = new ResourceLocation("minecraft:brown_mushroom");
const RED_MUSHROOM_LOCATION = new ResourceLocation("minecraft:red_mushroom");
const BLUE_ORCHID_LOCATION = new ResourceLocation("minecraft:blue_orchid");
const POPPY_LOCATION = new ResourceLocation("minecraft:poppy");
const DEAD_BUSH_LOCATION = new ResourceLocation("minecraft:dead_bush");
const LILY_PAD_LOCATION = new ResourceLocation("minecraft:lily_pad");
const TALL_GRASS_LOCATION = new ResourceLocation("minecraft:tall_grass");
const LILAC_LOCATION = new ResourceLocation("minecraft:lilac");
const ROSE_BUSH_LOCATION = new ResourceLocation("minecraft:rose_bush");
const PEONY_LOCATION = new ResourceLocation("minecraft:peony");
const LILY_OF_THE_VALLEY_LOCATION = new ResourceLocation("minecraft:lily_of_the_valley");
const PUMPKIN_LOCATION = new ResourceLocation("minecraft:pumpkin");
const MELON_LOCATION = new ResourceLocation("minecraft:melon");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const PODZOL_LOCATION = new ResourceLocation("minecraft:podzol");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");
const CACTUS_LOCATION = new ResourceLocation("minecraft:cactus");
const RED_MUSHROOM_BLOCK_LOCATION = new ResourceLocation("minecraft:red_mushroom_block");
const BROWN_MUSHROOM_BLOCK_LOCATION = new ResourceLocation("minecraft:brown_mushroom_block");
const MUSHROOM_STEM_LOCATION = new ResourceLocation("minecraft:mushroom_stem");

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

function darkOakDecorator() {
  return heightmapWithTreeThreshold().decorated(FeatureDecorators.DARK_OAK_TREE.configured(NoneDecoratorConfiguration.INSTANCE));
}

function heightmapDoubleSquare() {
  return heightmapDoubleDecorator(Heightmap.Types.MOTION_BLOCKING).squared();
}

function heightmapSquare() {
  return heightmapDecorator(Heightmap.Types.MOTION_BLOCKING).squared();
}

function heightmapTopSolidSquare() {
  return heightmapDecorator(Heightmap.Types.OCEAN_FLOOR_WG).squared();
}

function heightmapOceanFloorDecorator() {
  return heightmapDecorator(Heightmap.Types.OCEAN_FLOOR);
}

function spread32AboveDecorator() {
  return FeatureDecorators.SPREAD_32_ABOVE.configured(NoneDecoratorConfiguration.INSTANCE);
}

function countExtraDecorator(count: number, extraChance: number, extraCount: number) {
  return FeatureDecorators.COUNT_EXTRA.configured(new FrequencyWithExtraChanceDecoratorConfiguration(count, extraChance, extraCount));
}

function countNoiseDecorator(noiseLevel: number, belowNoise: number, aboveNoise: number) {
  return FeatureDecorators.COUNT_NOISE.configured(new NoiseDependantDecoratorConfiguration(noiseLevel, belowNoise, aboveNoise));
}

function waterDepthThresholdDecorator(maxWaterDepth: number) {
  return FeatureDecorators.WATER_DEPTH_THRESHOLD.configured(new WaterDepthThresholdConfiguration(maxWaterDepth));
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

function createJungleGrassConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new WeightedStateProvider([
      { state: getRequiredState(GRASS_LOCATION), weight: 3 },
      { state: getRequiredState(FERN_LOCATION), weight: 1 },
    ]),
    SimpleBlockPlacer.INSTANCE,
  )
    .blacklistSet(new Set([getRequiredState(PODZOL_LOCATION)]))
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

function createTallGrassConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(TALL_GRASS_LOCATION)),
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

function createBlueOrchidConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(BLUE_ORCHID_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .build();
}

function createDefaultFlowerConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new WeightedStateProvider([
      { state: getRequiredState(POPPY_LOCATION), weight: 2 },
      { state: getRequiredState(DANDELION_LOCATION), weight: 1 },
    ]),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .build();
}

function createDeadBushConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(DEAD_BUSH_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(4)
    .build();
}

function createWaterlilyConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(LILY_PAD_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(10)
    .build();
}

function createSimpleFlowerConfig(location: ResourceLocation): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(location)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .build();
}

function createDoubleFlowerConfig(location: ResourceLocation): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(location)),
    DoublePlantPlacer.INSTANCE,
  )
    .triesCount(64)
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

function createMelonConfig(): RandomPatchConfiguration {
  return RandomPatchConfiguration.grassConfigurationBuilder(
    new SimpleStateProvider(getRequiredState(MELON_LOCATION)),
    SimpleBlockPlacer.INSTANCE,
  )
    .triesCount(64)
    .whitelistSet(new Set([getRequiredState(GRASS_BLOCK_LOCATION).getBlock()]))
    .canReplaceBlocks()
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

function createForestFlowerFeatures() {
  return [
    () => Features.RANDOM_PATCH.configured(createDoubleFlowerConfig(LILAC_LOCATION)),
    () => Features.RANDOM_PATCH.configured(createDoubleFlowerConfig(ROSE_BUSH_LOCATION)),
    () => Features.RANDOM_PATCH.configured(createDoubleFlowerConfig(PEONY_LOCATION)),
    () => Features.NO_BONEMEAL_FLOWER.configured(createSimpleFlowerConfig(LILY_OF_THE_VALLEY_LOCATION)),
  ] as const;
}

function createHugeBrownMushroomConfig(): HugeMushroomFeatureConfiguration {
  return new HugeMushroomFeatureConfiguration(
    new SimpleStateProvider(
      getRequiredState(BROWN_MUSHROOM_BLOCK_LOCATION)
        .setValue(HugeMushroomBlock.UP, true)
        .setValue(HugeMushroomBlock.DOWN, false),
    ),
    new SimpleStateProvider(
      getRequiredState(MUSHROOM_STEM_LOCATION)
        .setValue(HugeMushroomBlock.UP, false)
        .setValue(HugeMushroomBlock.DOWN, false),
    ),
    3,
  );
}

function createHugeRedMushroomConfig(): HugeMushroomFeatureConfiguration {
  return new HugeMushroomFeatureConfiguration(
    new SimpleStateProvider(getRequiredState(RED_MUSHROOM_BLOCK_LOCATION).setValue(HugeMushroomBlock.DOWN, false)),
    new SimpleStateProvider(
      getRequiredState(MUSHROOM_STEM_LOCATION)
        .setValue(HugeMushroomBlock.UP, false)
        .setValue(HugeMushroomBlock.DOWN, false),
    ),
    2,
  );
}

export class VegetationFeatures {
  public static get BROWN_MUSHROOM_NORMAL() {
    return Features.RANDOM_PATCH.configured(createBrownMushroomConfig()).decorated(heightmapDoubleSquare()).rarity(4);
  }

  public static get BROWN_MUSHROOM_TAIGA() {
    return Features.RANDOM_PATCH.configured(createBrownMushroomConfig()).rarity(4).decorated(heightmapSquare());
  }

  public static get BROWN_MUSHROOM_SWAMP() {
    return VegetationFeatures.BROWN_MUSHROOM_TAIGA.count(8);
  }

  public static get BROWN_MUSHROOM_GIANT() {
    return VegetationFeatures.BROWN_MUSHROOM_TAIGA.count(3);
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

  public static get PATCH_DEAD_BUSH_2() {
    return Features.RANDOM_PATCH.configured(createDeadBushConfig()).decorated(heightmapDoubleSquare()).count(2);
  }

  public static get PATCH_DEAD_BUSH() {
    return Features.RANDOM_PATCH.configured(createDeadBushConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_DEAD_BUSH_BADLANDS() {
    return Features.RANDOM_PATCH.configured(createDeadBushConfig()).decorated(heightmapDoubleSquare()).count(20);
  }

  public static get PATCH_GRASS_BADLANDS() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_GRASS_FOREST() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare()).count(2);
  }

  public static get PATCH_GRASS_NORMAL() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare()).count(5);
  }

  public static get PATCH_GRASS_SAVANNA() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare()).count(20);
  }

  public static get PATCH_GRASS_PLAIN() {
    return Features.RANDOM_PATCH.configured(createDefaultGrassConfig()).decorated(heightmapDoubleSquare()).decorated(countNoiseDecorator(-0.8, 5, 10));
  }

  public static get PATCH_GRASS_JUNGLE() {
    return Features.RANDOM_PATCH.configured(createJungleGrassConfig()).decorated(heightmapDoubleSquare()).count(25);
  }

  public static get PATCH_GRASS_TAIGA_2() {
    return Features.RANDOM_PATCH.configured(createTaigaGrassConfig()).decorated(heightmapDoubleSquare());
  }

  public static get PATCH_GRASS_TAIGA() {
    return Features.RANDOM_PATCH.configured(createTaigaGrassConfig()).decorated(heightmapDoubleSquare()).count(7);
  }

  public static get PATCH_LARGE_FERN() {
    return Features.RANDOM_PATCH.configured(createLargeFernConfig()).decorated(spread32AboveDecorator()).decorated(heightmapSquare()).count(7);
  }

  public static get PATCH_TALL_GRASS_2() {
    return Features.RANDOM_PATCH.configured(createTallGrassConfig())
      .decorated(spread32AboveDecorator())
      .decorated(heightmapDecorator(Heightmap.Types.MOTION_BLOCKING))
      .squared()
      .decorated(countNoiseDecorator(-0.8, 0, 7));
  }

  public static get PATCH_TALL_GRASS() {
    return Features.RANDOM_PATCH.configured(createTallGrassConfig())
      .decorated(spread32AboveDecorator())
      .decorated(heightmapSquare())
      .count(7);
  }

  public static get PATCH_WATERLILLY() {
    return Features.RANDOM_PATCH.configured(createWaterlilyConfig()).decorated(heightmapDoubleSquare()).count(4);
  }

  public static get SEAGRASS_SWAMP() {
    return Features.SEAGRASS.configured(new ProbabilityFeatureConfiguration(0.6)).count(64).decorated(heightmapTopSolidSquare());
  }

  public static get PATCH_PUMPKIN() {
    return Features.RANDOM_PATCH.configured(createPumpkinConfig()).decorated(heightmapDoubleSquare()).rarity(32);
  }

  public static get PATCH_MELON() {
    return Features.RANDOM_PATCH.configured(createMelonConfig()).decorated(heightmapDoubleSquare());
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

  public static get RED_MUSHROOM_SWAMP() {
    return VegetationFeatures.RED_MUSHROOM_TAIGA.count(8);
  }

  public static get RED_MUSHROOM_GIANT() {
    return VegetationFeatures.RED_MUSHROOM_TAIGA.count(3);
  }

  public static get FLOWER_DEFAULT() {
    return Features.FLOWER.configured(createDefaultFlowerConfig()).decorated(spread32AboveDecorator()).decorated(heightmapSquare()).count(2);
  }

  public static get FLOWER_WARM() {
    return Features.FLOWER.configured(createDefaultFlowerConfig()).decorated(spread32AboveDecorator()).decorated(heightmapSquare()).count(4);
  }

  public static get FLOWER_FOREST() {
    return Features.FLOWER.configured(
      RandomPatchConfiguration.grassConfigurationBuilder(ForestFlowerProvider.INSTANCE, SimpleBlockPlacer.INSTANCE).triesCount(64).build(),
    )
      .decorated(spread32AboveDecorator())
      .decorated(heightmapSquare())
      .count(100);
  }

  public static get FLOWER_SWAMP() {
    return Features.FLOWER.configured(createBlueOrchidConfig()).decorated(spread32AboveDecorator()).decorated(heightmapSquare());
  }

  public static get FLOWER_PLAIN() {
    return Features.FLOWER.configured(
      RandomPatchConfiguration.grassConfigurationBuilder(PlainFlowerProvider.INSTANCE, SimpleBlockPlacer.INSTANCE).triesCount(64).build(),
    );
  }

  public static get FLOWER_PLAIN_DECORATED() {
    return VegetationFeatures.FLOWER_PLAIN.decorated(spread32AboveDecorator())
      .decorated(heightmapDecorator(Heightmap.Types.MOTION_BLOCKING))
      .squared()
      .decorated(countNoiseDecorator(-0.8, 15, 4));
  }

  public static get FOREST_FLOWER_VEGETATION_COMMON() {
    return Features.SIMPLE_RANDOM_SELECTOR.configured(new SimpleRandomFeatureConfiguration(createForestFlowerFeatures()))
      .count(ClampedInt.of(UniformInt.of(-1, 3), 0, 3))
      .decorated(spread32AboveDecorator())
      .decorated(heightmapSquare())
      .count(5);
  }

  public static get FOREST_FLOWER_VEGETATION() {
    return Features.SIMPLE_RANDOM_SELECTOR.configured(new SimpleRandomFeatureConfiguration(createForestFlowerFeatures()))
      .count(ClampedInt.of(UniformInt.of(-3, 1), 0, 1))
      .decorated(spread32AboveDecorator())
      .decorated(heightmapSquare())
      .count(5);
  }

  public static get HUGE_BROWN_MUSHROOM() {
    return Features.HUGE_BROWN_MUSHROOM.configured(createHugeBrownMushroomConfig());
  }

  public static get HUGE_RED_MUSHROOM() {
    return Features.HUGE_RED_MUSHROOM.configured(createHugeRedMushroomConfig());
  }

  public static get VINES() {
    return Features.VINES.configured(NoneFeatureConfiguration.INSTANCE).squared().count(50);
  }

  public static get DARK_OAK() {
    return TreeFeatures.DARK_OAK;
  }

  public static get TREES_SHATTERED_SAVANNA() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.ACACIA.weighted(0.8)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(2, 0.1, 1));
  }

  public static get TREES_JUNGLE_EDGE() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.FANCY_OAK.weighted(0.1), TreeFeatures.JUNGLE_BUSH.weighted(0.5)], TreeFeatures.JUNGLE_TREE),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(2, 0.1, 1));
  }

  public static get TREES_JUNGLE() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration(
        [TreeFeatures.FANCY_OAK.weighted(0.1), TreeFeatures.JUNGLE_BUSH.weighted(0.5), TreeFeatures.MEGA_JUNGLE_TREE.weighted(0.33333334)],
        TreeFeatures.JUNGLE_TREE,
      ),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(50, 0.1, 1));
  }

  public static get TREES_SAVANNA() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.ACACIA.weighted(0.8)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(1, 0.1, 1));
  }

  public static get TREES_SNOWY() {
    return TreeFeatures.SPRUCE.decorated(heightmapWithTreeThresholdSquared()).decorated(countExtraDecorator(0, 0.1, 1));
  }

  public static get DARK_FOREST_VEGETATION_BROWN() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration(
        [
          VegetationFeatures.HUGE_BROWN_MUSHROOM.weighted(0.025),
          VegetationFeatures.HUGE_RED_MUSHROOM.weighted(0.05),
          VegetationFeatures.DARK_OAK.weighted(0.6666667),
          TreeFeatures.BIRCH.weighted(0.2),
          TreeFeatures.FANCY_OAK.weighted(0.1),
        ],
        TreeFeatures.OAK,
      ),
    ).decorated(darkOakDecorator());
  }

  public static get DARK_FOREST_VEGETATION_RED() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration(
        [
          VegetationFeatures.HUGE_RED_MUSHROOM.weighted(0.025),
          VegetationFeatures.HUGE_BROWN_MUSHROOM.weighted(0.05),
          VegetationFeatures.DARK_OAK.weighted(0.6666667),
          TreeFeatures.BIRCH.weighted(0.2),
          TreeFeatures.FANCY_OAK.weighted(0.1),
        ],
        TreeFeatures.OAK,
      ),
    ).decorated(darkOakDecorator());
  }

  public static get FOREST_FLOWER_TREES() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.BIRCH.weighted(0.2), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(6, 0.1, 1));
  }

  public static get BIRCH_TALL() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.SUPER_BIRCH.weighted(0.5)], TreeFeatures.BIRCH),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get TREES_GIANT_SPRUCE() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.MEGA_SPRUCE.weighted(0.33333334), TreeFeatures.PINE.weighted(0.33333334)], TreeFeatures.SPRUCE),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get TREES_GIANT() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration(
        [
          TreeFeatures.MEGA_SPRUCE.weighted(0.025641026),
          TreeFeatures.MEGA_PINE.weighted(0.30769232),
          TreeFeatures.PINE.weighted(0.33333334),
        ],
        TreeFeatures.SPRUCE,
      ),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get TREES_BIRCH() {
    return TreeFeatures.BIRCH.decorated(heightmapWithTreeThresholdSquared()).decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get TREES_WATER() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(0, 0.1, 1));
  }

  public static get BIRCH_OTHER() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.BIRCH.weighted(0.2), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get PLAIN_VEGETATION() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.FANCY_OAK.weighted(0.33333334)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(0, 0.05, 1));
  }

  public static get TAIGA_VEGETATION() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.PINE.weighted(0.33333334)], TreeFeatures.SPRUCE),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(10, 0.1, 1));
  }

  public static get TREES_SWAMP() {
    return TreeFeatures.SWAMP_OAK.decorated(heightmapOceanFloorDecorator())
      .decorated(waterDepthThresholdDecorator(1))
      .squared()
      .decorated(countExtraDecorator(2, 0.1, 1));
  }

  public static get TREES_MOUNTAIN() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.SPRUCE.weighted(0.666), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(0, 0.1, 1));
  }

  public static get TREES_MOUNTAIN_EDGE() {
    return Features.RANDOM_SELECTOR.configured(
      new RandomFeatureConfiguration([TreeFeatures.SPRUCE.weighted(0.666), TreeFeatures.FANCY_OAK.weighted(0.1)], TreeFeatures.OAK),
    )
      .decorated(heightmapWithTreeThresholdSquared())
      .decorated(countExtraDecorator(3, 0.1, 1));
  }

  public static get MUSHROOM_FIELD_VEGETATION() {
    return Features.RANDOM_BOOLEAN_SELECTOR.configured(
      new RandomBooleanFeatureConfiguration(() => VegetationFeatures.HUGE_RED_MUSHROOM, () => VegetationFeatures.HUGE_BROWN_MUSHROOM),
    ).decorated(heightmapSquare());
  }
}
