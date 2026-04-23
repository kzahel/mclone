import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { RandomFeatureConfiguration } from "./configurations/random-feature-configuration";

export class RandomSelectorFeature extends Feature<RandomFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<RandomFeatureConfiguration>): boolean {
    const config = context.config();
    const random = context.random();
    const level = context.level();
    const chunkGenerator = context.chunkGenerator();
    const origin = context.origin();

    for (const feature of config.features) {
      if (random.nextFloat() < feature.chance) {
        return feature.place(level, chunkGenerator, random, origin);
      }
    }

    return config.defaultFeature.place(level, chunkGenerator, random, origin);
  }
}
