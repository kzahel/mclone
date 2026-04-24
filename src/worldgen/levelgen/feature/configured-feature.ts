import { BlockPos } from "../../../core/block-pos";
import { ConstantInt } from "../../../util/valueproviders/constant-int";
import { type VerticalAnchor, trapezoidHeight, uniformHeight } from "../../carver/carver-config";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";
import type { CooperativeGenerationYield } from "../cooperative-generation";
import { DecorationContext } from "../placement/decoration-context";
import { ConfiguredDecorator } from "../placement/configured-decorator";
import { FeatureDecorators } from "../placement/feature-decorators";
import type { IntProvider } from "../../../util/valueproviders/int-provider";
import { UniformInt } from "../../../util/valueproviders/uniform-int";
import { ChanceDecoratorConfiguration } from "./configurations/chance-decorator-configuration";
import { CountConfiguration } from "./configurations/count-configuration";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "./configurations/decorator-configuration";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import { NoneDecoratorConfiguration } from "./configurations/none-decorator-configuration";
import { RangeDecoratorConfiguration } from "./configurations/range-decorator-configuration";
import { FeaturePlaceContext } from "./feature-place-context";
import { WeightedConfiguredFeature } from "./weighted-configured-feature";

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

  public count(count: number | IntProvider): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.decorated(FeatureDecorators.COUNT.configured(new CountConfiguration(typeof count === "number" ? ConstantInt.of(count) : count)));
  }

  public countRandom(maxInclusive: number): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.count(UniformInt.of(0, maxInclusive));
  }

  public rarity(chance: number): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.decorated(FeatureDecorators.CHANCE.configured(new ChanceDecoratorConfiguration(chance)));
  }

  public range(
    config: RangeDecoratorConfiguration,
  ): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.decorated(FeatureDecorators.RANGE.configured(config));
  }

  public rangeUniform(
    minInclusive: VerticalAnchor,
    maxInclusive: VerticalAnchor,
  ): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.range(new RangeDecoratorConfiguration(uniformHeight(minInclusive, maxInclusive)));
  }

  public rangeTriangle(
    minInclusive: VerticalAnchor,
    maxInclusive: VerticalAnchor,
  ): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.range(new RangeDecoratorConfiguration(trapezoidHeight(minInclusive, maxInclusive)));
  }

  public squared(): ConfiguredFeature<DecoratedFeatureConfiguration, PlaceableFeature<DecoratedFeatureConfiguration>> {
    return this.decorated(FeatureDecorators.SQUARE.configured(NoneDecoratorConfiguration.INSTANCE));
  }

  public weighted(chance: number): WeightedConfiguredFeature {
    return new WeightedConfiguredFeature(this as unknown as ConfiguredFeature<any, any>, chance);
  }

  public place(level: WorldGenLevel, chunkGenerator: NoiseBasedChunkGenerator, random: SimpleRandomSource, origin: BlockPos): boolean {
    return this.feature.place(new FeaturePlaceContext(level, chunkGenerator, random, origin, this.config));
  }

  public async placeCooperative(
    level: WorldGenLevel,
    chunkGenerator: NoiseBasedChunkGenerator,
    random: SimpleRandomSource,
    origin: BlockPos,
    yieldStep: CooperativeGenerationYield,
  ): Promise<boolean> {
    if (this.config instanceof DecoratedFeatureConfiguration) {
      let placed = false;
      const feature = this.config.feature();
      for (const position of this.config.decorator.getPositions(new DecorationContext(level, chunkGenerator), random, origin)) {
        await yieldStep();
        if (await feature.placeCooperative(level, chunkGenerator, random, position, yieldStep)) {
          placed = true;
        }
      }

      return placed;
    }

    await yieldStep();
    return this.place(level, chunkGenerator, random, origin);
  }
}
