import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";
import { DecorationContext } from "../placement/decoration-context";
import { ConfiguredDecorator } from "../placement/configured-decorator";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "./configurations/decorator-configuration";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import { FeaturePlaceContext } from "./feature-place-context";

interface PlaceableFeature<FC extends FeatureConfiguration> {
  place(context: FeaturePlaceContext<FC>): boolean;
}

const DECORATED_FEATURE: PlaceableFeature<DecoratedFeatureConfiguration> = {
  place(context): boolean {
    let placed = false;
    const level = context.level();
    const config = context.config();
    const chunkGenerator = context.chunkGenerator();
    const random = context.random();
    const origin = context.origin();
    const feature = config.feature();
    for (const position of config.decorator.getPositions(new DecorationContext(level, chunkGenerator), random, origin)) {
      if (feature.place(level, chunkGenerator, random, position)) {
        placed = true;
      }
    }

    return placed;
  },
};

export class ConfiguredFeature<FC extends FeatureConfiguration, F extends PlaceableFeature<FC>> {
  public constructor(
    public readonly feature: F,
    public readonly config: FC,
  ) {}

  public decorated(
    decorator: ConfiguredDecorator<DecoratorConfiguration>,
  ): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return new ConfiguredFeature(DECORATED_FEATURE, new DecoratedFeatureConfiguration(() => this as never, decorator));
  }

  public place(level: WorldGenLevel, chunkGenerator: NoiseBasedChunkGenerator, random: SimpleRandomSource, origin: BlockPos): boolean {
    return this.feature.place(new FeaturePlaceContext(level, chunkGenerator, random, origin, this.config));
  }
}
