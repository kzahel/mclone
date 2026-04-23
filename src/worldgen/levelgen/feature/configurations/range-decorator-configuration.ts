import type { HeightProvider } from "../../../carver/carver-config";
import type { DecoratorConfiguration } from "./decorator-configuration";
import type { FeatureConfiguration } from "./feature-configuration";

export class RangeDecoratorConfiguration implements DecoratorConfiguration, FeatureConfiguration {
  public constructor(public readonly height: HeightProvider) {}
}
