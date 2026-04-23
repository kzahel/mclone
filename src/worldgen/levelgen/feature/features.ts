import { Registry } from "../../../core/registry";
import { DecoratedFeature } from "./decorated-feature";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import { Feature } from "./feature";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";
import { SimpleBlockConfiguration } from "./configurations/simple-block-configuration";
import { RandomPatchFeature } from "./random-patch-feature";
import { SimpleBlockFeature } from "./simple-block-feature";
import type { FeatureConfiguration } from "./configurations/feature-configuration";

function register<C extends FeatureConfiguration, F extends Feature<C>>(name: string, feature: F): F {
  return Registry.register(Registry.FEATURE, name, feature) as F;
}

export const Features = {
  RANDOM_PATCH: register("random_patch", new RandomPatchFeature()),
  SIMPLE_BLOCK: register("simple_block", new SimpleBlockFeature()),
  DECORATED: register("decorated", new DecoratedFeature()),
} as const;

export type SimpleVegetationFeature =
  | Feature<SimpleBlockConfiguration>
  | Feature<RandomPatchConfiguration>
  | Feature<DecoratedFeatureConfiguration>;
