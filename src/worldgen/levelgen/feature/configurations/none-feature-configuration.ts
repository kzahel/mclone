import type { FeatureConfiguration } from "./feature-configuration";

export class NoneFeatureConfiguration implements FeatureConfiguration {
  public static readonly INSTANCE = new NoneFeatureConfiguration();

  private constructor() {}
}
