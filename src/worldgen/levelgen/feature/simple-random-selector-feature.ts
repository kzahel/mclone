import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { SimpleRandomFeatureConfiguration } from "./configurations/simple-random-feature-configuration";

export class SimpleRandomSelectorFeature extends Feature<SimpleRandomFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<SimpleRandomFeatureConfiguration>): boolean {
    const random = context.random();
    const config = context.config();
    const level = context.level();
    const pos = context.origin();
    const chunkGenerator = context.chunkGenerator();
    const feature = config.features[random.nextInt(config.features.length)]!();
    return feature.place(level, chunkGenerator, random, pos);
  }
}
