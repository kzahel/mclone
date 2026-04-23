import { Registry } from "../../../core/registry";
import { DecoratedFeature } from "./decorated-feature";
import { LakeFeature } from "./lake-feature";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import { Feature } from "./feature";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { SimpleRandomFeatureConfiguration } from "./configurations/simple-random-feature-configuration";
import { SimpleBlockConfiguration } from "./configurations/simple-block-configuration";
import { SpringConfiguration } from "./configurations/spring-configuration";
import { TreeConfiguration } from "./configurations/tree-configuration";
import { RandomPatchFeature } from "./random-patch-feature";
import { RandomSelectorFeature } from "./random-selector-feature";
import { SimpleBlockFeature } from "./simple-block-feature";
import { SimpleRandomSelectorFeature } from "./simple-random-selector-feature";
import { SpringFeature } from "./spring-feature";
import { TreeFeature } from "./tree-feature";
import type { FeatureConfiguration } from "./configurations/feature-configuration";

function register<C extends FeatureConfiguration, F extends Feature<C>>(name: string, feature: F): F {
  return Registry.register(Registry.FEATURE, name, feature) as F;
}

export const Features = {
  TREE: register("tree", new TreeFeature()),
  RANDOM_PATCH: register("random_patch", new RandomPatchFeature()),
  SIMPLE_BLOCK: register("simple_block", new SimpleBlockFeature()),
  DECORATED: register("decorated", new DecoratedFeature()),
  RANDOM_SELECTOR: register("random_selector", new RandomSelectorFeature()),
  SIMPLE_RANDOM_SELECTOR: register("simple_random_selector", new SimpleRandomSelectorFeature()),
  LAKE: register("lake", new LakeFeature()),
  SPRING: register("spring", new SpringFeature()),
} as const;

export type SimpleVegetationFeature =
  | Feature<TreeConfiguration>
  | Feature<SimpleBlockConfiguration>
  | Feature<RandomPatchConfiguration>
  | Feature<DecoratedFeatureConfiguration>
  | Feature<RandomFeatureConfiguration>
  | Feature<SimpleRandomFeatureConfiguration>
  | Feature<BlockStateConfiguration>
  | Feature<SpringConfiguration>;
