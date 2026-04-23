import type { FeatureConfiguration } from "./feature-configuration";
import type { ConfiguredFeature } from "../configured-feature";
import type { WeightedConfiguredFeature } from "../weighted-configured-feature";

export class RandomFeatureConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly features: readonly WeightedConfiguredFeature[],
    public readonly defaultFeature: ConfiguredFeature<any, any>,
  ) {}
}
