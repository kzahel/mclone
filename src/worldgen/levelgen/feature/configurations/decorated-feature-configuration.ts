import type { ConfiguredFeature } from "../configured-feature";
import type { ConfiguredDecorator } from "../../placement/configured-decorator";
import type { FeatureConfiguration } from "./feature-configuration";

export class DecoratedFeatureConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly feature: () => ConfiguredFeature<FeatureConfiguration, import("../feature").Feature<FeatureConfiguration>>,
    public readonly decorator: ConfiguredDecorator<DecoratorConfiguration>,
  ) {}
}

import type { DecoratorConfiguration } from "./decorator-configuration";
