import type { ConfiguredFeature } from "../configured-feature";
import type { FeatureConfiguration } from "./feature-configuration";

export class SimpleRandomFeatureConfiguration implements FeatureConfiguration {
  public constructor(public readonly features: readonly (() => ConfiguredFeature<any, any>)[]) {}
}
