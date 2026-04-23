import { ConstantInt } from "../../../../util/valueproviders/constant-int";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { DecoratorConfiguration } from "./decorator-configuration";
import type { FeatureConfiguration } from "./feature-configuration";

export class CountConfiguration implements DecoratorConfiguration, FeatureConfiguration {
  private readonly countValue: IntProvider;

  public constructor(count: number | IntProvider) {
    this.countValue = typeof count === "number" ? ConstantInt.of(count) : count;
  }

  public count(): IntProvider {
    return this.countValue;
  }
}
