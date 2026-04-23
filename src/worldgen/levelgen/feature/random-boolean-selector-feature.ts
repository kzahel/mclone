import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { RandomBooleanFeatureConfiguration } from "./configurations/random-boolean-feature-configuration";

export class RandomBooleanSelectorFeature extends Feature<RandomBooleanFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<RandomBooleanFeatureConfiguration>): boolean {
    const config = context.config();
    const random = context.random();
    const level = context.level();
    const chunkGenerator = context.chunkGenerator();
    const origin = context.origin();
    const feature = random.nextBoolean() ? config.featureTrue() : config.featureFalse();
    return feature.place(level, chunkGenerator, random, origin);
  }
}
