import { Registry } from "../../../core/registry";
import { DefaultFlowerFeature } from "./default-flower-feature";
import { DesertWellFeature } from "./desert-well-feature";
import { DecoratedFeature } from "./decorated-feature";
import { LakeFeature } from "./lake-feature";
import { BambooFeature } from "./bamboo-feature";
import { ProbabilityFeatureConfiguration } from "./configurations/probability-feature-configuration";
import { RandomBooleanFeatureConfiguration } from "./configurations/random-boolean-feature-configuration";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import { Feature } from "./feature";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";
import { DiskConfiguration } from "./configurations/disk-configuration";
import { DripstoneClusterConfiguration } from "./configurations/dripstone-cluster-configuration";
import { GlowLichenConfiguration } from "./configurations/glow-lichen-configuration";
import { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { ReplaceBlockConfiguration } from "./configurations/replace-block-configuration";
import { SmallDripstoneConfiguration } from "./configurations/small-dripstone-configuration";
import { SimpleRandomFeatureConfiguration } from "./configurations/simple-random-feature-configuration";
import { SimpleBlockConfiguration } from "./configurations/simple-block-configuration";
import { DiskReplaceFeature } from "./disk-replace-feature";
import { DripstoneClusterFeature } from "./dripstone-cluster-feature";
import { GlowLichenFeature } from "./glow-lichen-feature";
import { HugeBrownMushroomFeature } from "./huge-brown-mushroom-feature";
import { HugeRedMushroomFeature } from "./huge-red-mushroom-feature";
import { IcePatchFeature } from "./ice-patch-feature";
import { IceSpikeFeature } from "./ice-spike-feature";
import { KelpFeature } from "./kelp-feature";
import { OreFeature } from "./ore-feature";
import { MonsterRoomFeature } from "./monster-room-feature";
import { FossilFeature } from "./fossil-feature";
import { CoralClawFeature } from "./coral-claw-feature";
import { CoralMushroomFeature } from "./coral-mushroom-feature";
import { CoralTreeFeature } from "./coral-tree-feature";
import { SeagrassFeature } from "./seagrass-feature";
import { SeaPickleFeature } from "./sea-pickle-feature";
import { SnowAndFreezeFeature } from "./snow-and-freeze-feature";
import { SpringConfiguration } from "./configurations/spring-configuration";
import { TreeConfiguration } from "./configurations/tree-configuration";
import { RandomPatchFeature } from "./random-patch-feature";
import { RandomBooleanSelectorFeature } from "./random-boolean-selector-feature";
import { RandomSelectorFeature } from "./random-selector-feature";
import { ReplaceBlockFeature } from "./replace-block-feature";
import { SimpleBlockFeature } from "./simple-block-feature";
import { SmallDripstoneFeature } from "./small-dripstone-feature";
import { SimpleRandomSelectorFeature } from "./simple-random-selector-feature";
import { SpringFeature } from "./spring-feature";
import { TreeFeature } from "./tree-feature";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";
import { FossilFeatureConfiguration } from "./configurations/fossil-feature-configuration";
import { VinesFeature } from "./vines-feature";

function register<C extends FeatureConfiguration, F extends Feature<C>>(name: string, feature: F): F {
  return Registry.register(Registry.FEATURE, name, feature) as F;
}

export const Features = {
  TREE: register("tree", new TreeFeature()),
  FLOWER: register("flower", new DefaultFlowerFeature()),
  NO_BONEMEAL_FLOWER: register("no_bonemeal_flower", new DefaultFlowerFeature()),
  RANDOM_PATCH: register("random_patch", new RandomPatchFeature()),
  REPLACE_SINGLE_BLOCK: register("replace_single_block", new ReplaceBlockFeature()),
  SIMPLE_BLOCK: register("simple_block", new SimpleBlockFeature()),
  DECORATED: register("decorated", new DecoratedFeature()),
  RANDOM_BOOLEAN_SELECTOR: register("random_boolean_selector", new RandomBooleanSelectorFeature()),
  RANDOM_SELECTOR: register("random_selector", new RandomSelectorFeature()),
  SIMPLE_RANDOM_SELECTOR: register("simple_random_selector", new SimpleRandomSelectorFeature()),
  LAKE: register("lake", new LakeFeature()),
  SPRING: register("spring", new SpringFeature()),
  SEAGRASS: register("seagrass", new SeagrassFeature()),
  KELP: register("kelp", new KelpFeature()),
  BAMBOO: register("bamboo", new BambooFeature()),
  ORE: register("ore", new OreFeature()),
  MONSTER_ROOM: register("monster_room", new MonsterRoomFeature()),
  FOSSIL: register("fossil", new FossilFeature()),
  GLOW_LICHEN: register("glow_lichen", new GlowLichenFeature()),
  DRIPSTONE_CLUSTER: register("dripstone_cluster", new DripstoneClusterFeature()),
  SMALL_DRIPSTONE: register("small_dripstone", new SmallDripstoneFeature()),
  CORAL_TREE: register("coral_tree", new CoralTreeFeature()),
  CORAL_MUSHROOM: register("coral_mushroom", new CoralMushroomFeature()),
  CORAL_CLAW: register("coral_claw", new CoralClawFeature()),
  SEA_PICKLE: register("sea_pickle", new SeaPickleFeature()),
  HUGE_RED_MUSHROOM: register("huge_red_mushroom", new HugeRedMushroomFeature()),
  HUGE_BROWN_MUSHROOM: register("huge_brown_mushroom", new HugeBrownMushroomFeature()),
  ICE_SPIKE: register("ice_spike", new IceSpikeFeature()),
  DESERT_WELL: register("desert_well", new DesertWellFeature()),
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
  | Feature<GlowLichenConfiguration>
  | Feature<DripstoneClusterConfiguration>
  | Feature<SmallDripstoneConfiguration>
  | Feature<BlockStateConfiguration>
  | Feature<DiskConfiguration>
  | Feature<ReplaceBlockConfiguration>
  | Feature<SpringConfiguration>
  | Feature<ProbabilityFeatureConfiguration>
  | Feature<FossilFeatureConfiguration>
  | Feature<NoneFeatureConfiguration>;
