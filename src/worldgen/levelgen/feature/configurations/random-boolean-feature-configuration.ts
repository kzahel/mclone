import type { ConfiguredFeature } from "../configured-feature";
import type { FeatureConfiguration } from "./feature-configuration";

export class RandomBooleanFeatureConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly featureTrue: () => ConfiguredFeature<any, any>,
    public readonly featureFalse: () => ConfiguredFeature<any, any>,
  ) {}
}
