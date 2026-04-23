import type { DecoratorConfiguration } from "./decorator-configuration";
import type { FeatureConfiguration } from "./feature-configuration";

export class FrequencyWithExtraChanceDecoratorConfiguration implements DecoratorConfiguration, FeatureConfiguration {
  public constructor(
    public readonly count: number,
    public readonly extraChance: number,
    public readonly extraCount: number,
  ) {}
}
