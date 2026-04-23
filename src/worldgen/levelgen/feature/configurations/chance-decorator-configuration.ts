import type { DecoratorConfiguration } from "./decorator-configuration";

export class ChanceDecoratorConfiguration implements DecoratorConfiguration {
  public constructor(public readonly chance: number) {}
}
