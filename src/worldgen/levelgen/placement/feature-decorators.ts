import { Registry } from "../../../core/registry";
import { CountConfiguration } from "../feature/configurations/count-configuration";
import { DecoratedDecoratorConfiguration } from "../feature/configurations/decorated-decorator-configuration";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";
import { HeightmapConfiguration } from "../feature/configurations/heightmap-configuration";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import { CountDecorator } from "./count-decorator";
import { DecoratedDecorator } from "./decorated-decorator";
import { FeatureDecorator } from "./feature-decorator";
import { HeightmapDecorator } from "./heightmap-decorator";
import { NopePlacementDecorator } from "./nope-placement-decorator";
import { SquareDecorator } from "./square-decorator";

function register<T extends DecoratorConfiguration, G extends FeatureDecorator<T>>(name: string, decorator: G): G {
  return Registry.register(Registry.DECORATOR, name, decorator) as G;
}

export const FeatureDecorators = {
  NOPE: register("nope", new NopePlacementDecorator()),
  DECORATED: register("decorated", new DecoratedDecorator()),
  SQUARE: register("square", new SquareDecorator()),
  COUNT: register("count", new CountDecorator()),
  HEIGHTMAP: register("heightmap", new HeightmapDecorator()),
} as const;

export type SimpleFeatureDecorator =
  | FeatureDecorator<NoneDecoratorConfiguration>
  | FeatureDecorator<CountConfiguration>
  | FeatureDecorator<HeightmapConfiguration>
  | FeatureDecorator<DecoratedDecoratorConfiguration>;
