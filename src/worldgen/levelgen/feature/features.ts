import { Registry } from "../../../core/registry";
import { DefaultFlowerFeature } from "./default-flower-feature";
import { DecoratedFeature } from "./decorated-feature";
import { LakeFeature } from "./lake-feature";
import { ProbabilityFeatureConfiguration } from "./configurations/probability-feature-configuration";
import { RandomBooleanFeatureConfiguration } from "./configurations/random-boolean-feature-configuration";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import { Feature } from "./feature";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";
import { DiskConfiguration } from "./configurations/disk-configuration";
import { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { SimpleRandomFeatureConfiguration } from "./configurations/simple-random-feature-configuration";
import { SimpleBlockConfiguration } from "./configurations/simple-block-configuration";
import { DiskReplaceFeature } from "./disk-replace-feature";
import { HugeBrownMushroomFeature } from "./huge-brown-mushroom-feature";
import { HugeRedMushroomFeature } from "./huge-red-mushroom-feature";
import { IcePatchFeature } from "./ice-patch-feature";
import { IceSpikeFeature } from "./ice-spike-feature";
import { KelpFeature } from "./kelp-feature";
import { SeagrassFeature } from "./seagrass-feature";
import { SnowAndFreezeFeature } from "./snow-and-freeze-feature";
import { SpringConfiguration } from "./configurations/spring-configuration";
import { TreeConfiguration } from "./configurations/tree-configuration";
import { RandomPatchFeature } from "./random-patch-feature";
import { RandomBooleanSelectorFeature } from "./random-boolean-selector-feature";
import { RandomSelectorFeature } from "./random-selector-feature";
import { SimpleBlockFeature } from "./simple-block-feature";
import { SimpleRandomSelectorFeature } from "./simple-random-selector-feature";
import { SpringFeature } from "./spring-feature";
import { TreeFeature } from "./tree-feature";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";
import { VinesFeature } from "./vines-feature";

function register<C extends FeatureConfiguration, F extends Feature<C>>(name: string, feature: F): F {
  return Registry.register(Registry.FEATURE, name, feature) as F;
}

export const Features = {
  TREE: register("tree", new TreeFeature()),
  FLOWER: register("flower", new DefaultFlowerFeature()),
  NO_BONEMEAL_FLOWER: register("no_bonemeal_flower", new DefaultFlowerFeature()),
  RANDOM_PATCH: register("random_patch", new RandomPatchFeature()),
  SIMPLE_BLOCK: register("simple_block", new SimpleBlockFeature()),
  DECORATED: register("decorated", new DecoratedFeature()),
  RANDOM_BOOLEAN_SELECTOR: register("random_boolean_selector", new RandomBooleanSelectorFeature()),
  RANDOM_SELECTOR: register("random_selector", new RandomSelectorFeature()),
  SIMPLE_RANDOM_SELECTOR: register("simple_random_selector", new SimpleRandomSelectorFeature()),
  LAKE: register("lake", new LakeFeature()),
  SPRING: register("spring", new SpringFeature()),
  SEAGRASS: register("seagrass", new SeagrassFeature()),
  KELP: register("kelp", new KelpFeature()),
  HUGE_RED_MUSHROOM: register("huge_red_mushroom", new HugeRedMushroomFeature()),
  HUGE_BROWN_MUSHROOM: register("huge_brown_mushroom", new HugeBrownMushroomFeature()),
  ICE_SPIKE: register("ice_spike", new IceSpikeFeature()),
  FREEZE_TOP_LAYER: register("freeze_top_layer", new SnowAndFreezeFeature()),
  VINES: register("vines", new VinesFeature()),
  DISK: register("disk", new DiskReplaceFeature()),
  ICE_PATCH: register("ice_patch", new IcePatchFeature()),
} as const;

export type SimpleVegetationFeature =
  | Feature<TreeConfiguration>
  | Feature<HugeMushroomFeatureConfiguration>
  | Feature<SimpleBlockConfiguration>
  | Feature<RandomPatchConfiguration>
  | Feature<DecoratedFeatureConfiguration>
  | Feature<RandomBooleanFeatureConfiguration>
  | Feature<RandomFeatureConfiguration>
  | Feature<SimpleRandomFeatureConfiguration>
  | Feature<BlockStateConfiguration>
  | Feature<DiskConfiguration>
  | Feature<SpringConfiguration>
  | Feature<ProbabilityFeatureConfiguration>
  | Feature<NoneFeatureConfiguration>;
