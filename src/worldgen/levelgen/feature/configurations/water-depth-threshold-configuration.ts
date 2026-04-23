import type { DecoratorConfiguration } from "./decorator-configuration";
import type { FeatureConfiguration } from "./feature-configuration";

export class WaterDepthThresholdConfiguration implements DecoratorConfiguration, FeatureConfiguration {
  public constructor(public readonly maxWaterDepth: number) {}
}
