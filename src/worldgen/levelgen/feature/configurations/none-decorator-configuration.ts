import type { DecoratorConfiguration } from "./decorator-configuration";

export class NoneDecoratorConfiguration implements DecoratorConfiguration {
  public static readonly INSTANCE = new NoneDecoratorConfiguration();

  private constructor() {}
}
