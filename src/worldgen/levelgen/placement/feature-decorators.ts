import { Registry } from "../../../core/registry";
import { CountConfiguration } from "../feature/configurations/count-configuration";
import { DecoratedDecoratorConfiguration } from "../feature/configurations/decorated-decorator-configuration";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "../feature/configurations/frequency-with-extra-chance-decorator-configuration";
import { HeightmapConfiguration } from "../feature/configurations/heightmap-configuration";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import { WaterDepthThresholdConfiguration } from "../feature/configurations/water-depth-threshold-configuration";
import { CountDecorator } from "./count-decorator";
import { CountWithExtraChanceDecorator } from "./count-with-extra-chance-decorator";
import { DecoratedDecorator } from "./decorated-decorator";
import { FeatureDecorator } from "./feature-decorator";
import { HeightmapDecorator } from "./heightmap-decorator";
import { HeightmapSpreadDoubleDecorator } from "./heightmap-spread-double-decorator";
import { NopePlacementDecorator } from "./nope-placement-decorator";
import { SquareDecorator } from "./square-decorator";
import { WaterDepthThresholdDecorator } from "./water-depth-threshold-decorator";

function register<T extends DecoratorConfiguration, G extends FeatureDecorator<T>>(name: string, decorator: G): G {
  return Registry.register(Registry.DECORATOR, name, decorator) as G;
}

export const FeatureDecorators = {
  NOPE: register("nope", new NopePlacementDecorator()),
  DECORATED: register("decorated", new DecoratedDecorator()),
  SQUARE: register("square", new SquareDecorator()),
  COUNT: register("count", new CountDecorator()),
  COUNT_EXTRA: register("count_extra", new CountWithExtraChanceDecorator()),
  HEIGHTMAP: register("heightmap", new HeightmapDecorator()),
  HEIGHTMAP_SPREAD_DOUBLE: register("heightmap_spread_double", new HeightmapSpreadDoubleDecorator()),
  WATER_DEPTH_THRESHOLD: register("water_depth_threshold", new WaterDepthThresholdDecorator()),
} as const;

export type SimpleFeatureDecorator =
  | FeatureDecorator<NoneDecoratorConfiguration>
  | FeatureDecorator<CountConfiguration>
  | FeatureDecorator<FrequencyWithExtraChanceDecoratorConfiguration>
  | FeatureDecorator<HeightmapConfiguration>
  | FeatureDecorator<DecoratedDecoratorConfiguration>
  | FeatureDecorator<WaterDepthThresholdConfiguration>;
