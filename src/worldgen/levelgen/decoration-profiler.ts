import { DecoratedFeatureConfiguration } from "./feature/configurations/decorated-feature-configuration";
import { DecoratedDecoratorConfiguration } from "./feature/configurations/decorated-decorator-configuration";
import type { DecoratorConfiguration } from "./feature/configurations/decorator-configuration";
import type { ConfiguredFeature } from "./feature/configured-feature";
import type { ConfiguredDecorator } from "./placement/configured-decorator";

export interface BiomeDecorationFeatureInfo {
  readonly stepIndex: number;
  readonly featureIndex: number;
  readonly featureName: string;
  readonly configName: string;
  readonly decoratorConfigNames: readonly string[];
}

export interface BiomeDecorationProfiler {
  beginFeature(info: BiomeDecorationFeatureInfo): void;
  endFeature(info: BiomeDecorationFeatureInfo, placed: boolean, elapsedMs: number): void;
}

function constructorName(value: unknown): string {
  if (typeof value !== "object" || value === null) {
    return typeof value;
  }

  return value.constructor.name || "anonymous";
}

function appendDecoratorConfigNames(
  decorator: ConfiguredDecorator<DecoratorConfiguration>,
  names: string[],
): void {
  const config = decorator.config();
  if (config instanceof DecoratedDecoratorConfiguration) {
    appendDecoratorConfigNames(config.outer(), names);
    appendDecoratorConfigNames(config.inner(), names);
    return;
  }

  names.push(constructorName(config));
}

export function describeConfiguredFeature(
  stepIndex: number,
  featureIndex: number,
  feature: ConfiguredFeature<any, any>,
): BiomeDecorationFeatureInfo {
  let unwrapped = feature;
  const decoratorConfigNames: string[] = [];
  while (unwrapped.config instanceof DecoratedFeatureConfiguration) {
    appendDecoratorConfigNames(unwrapped.config.decorator, decoratorConfigNames);
    unwrapped = unwrapped.config.feature() as ConfiguredFeature<any, any>;
  }

  return {
    stepIndex,
    featureIndex,
    featureName: constructorName(unwrapped.feature),
    configName: constructorName(unwrapped.config),
    decoratorConfigNames,
  };
}

export function monotonicDecorationNowMs(): number {
  return globalThis.performance?.now() ?? Date.now();
}
