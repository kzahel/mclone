import type { FeatureConfiguration } from "./feature-configuration";

export class ProbabilityFeatureConfiguration implements FeatureConfiguration {
  public constructor(public readonly probability: number) {}
}
