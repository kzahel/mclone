import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";
import { ConfiguredDecorator } from "../placement/configured-decorator";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "./configurations/decorator-configuration";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import { FeaturePlaceContext } from "./feature-place-context";
import { Features } from "./features";
import { Feature } from "./feature";

export class ConfiguredFeature<FC extends FeatureConfiguration, F extends Feature<FC>> {
  public constructor(
    public readonly feature: F,
    public readonly config: FC,
  ) {}

  public decorated(
    decorator: ConfiguredDecorator<DecoratorConfiguration>,
  ): ConfiguredFeature<DecoratedFeatureConfiguration, Feature<DecoratedFeatureConfiguration>> {
    return Features.DECORATED.configured(new DecoratedFeatureConfiguration(() => this as never, decorator));
  }

  public place(level: WorldGenLevel, chunkGenerator: NoiseBasedChunkGenerator, random: SimpleRandomSource, origin: BlockPos): boolean {
    return this.feature.place(new FeaturePlaceContext(level, chunkGenerator, random, origin, this.config));
  }
}
