import type { DecoratorConfiguration } from "./decorator-configuration";

export class NoiseDependantDecoratorConfiguration implements DecoratorConfiguration {
  public constructor(
    public readonly noiseLevel: number,
    public readonly belowNoise: number,
    public readonly aboveNoise: number,
  ) {}
}
