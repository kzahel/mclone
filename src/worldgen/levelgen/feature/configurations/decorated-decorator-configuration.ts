import type { ConfiguredDecorator } from "../../placement/configured-decorator";
import type { DecoratorConfiguration } from "./decorator-configuration";

export class DecoratedDecoratorConfiguration implements DecoratorConfiguration {
  public constructor(
    private readonly outerDecorator: ConfiguredDecorator<DecoratorConfiguration>,
    private readonly innerDecorator: ConfiguredDecorator<DecoratorConfiguration>,
  ) {}

  public outer(): ConfiguredDecorator<DecoratorConfiguration> {
    return this.outerDecorator;
  }

  public inner(): ConfiguredDecorator<DecoratorConfiguration> {
    return this.innerDecorator;
  }
}
