import type { FeaturePlaceContext } from "./feature-place-context";
import { Feature } from "./feature";
import { DecoratedFeatureConfiguration } from "./configurations/decorated-feature-configuration";
import { DecorationContext } from "../placement/decoration-context";

export class DecoratedFeature extends Feature<DecoratedFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<DecoratedFeatureConfiguration>): boolean {
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
  }
}
