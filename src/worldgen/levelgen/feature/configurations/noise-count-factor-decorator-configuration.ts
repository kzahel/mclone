import type { DecoratorConfiguration } from "./decorator-configuration";

export class NoiseCountFactorDecoratorConfiguration implements DecoratorConfiguration {
  public constructor(
    public readonly noiseToCountRatio: number,
    public readonly noiseFactor: number,
    public readonly noiseOffset: number,
  ) {}
}
