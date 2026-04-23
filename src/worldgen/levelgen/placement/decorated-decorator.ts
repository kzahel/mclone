import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { DecoratedDecoratorConfiguration } from "../feature/configurations/decorated-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class DecoratedDecorator extends FeatureDecorator<DecoratedDecoratorConfiguration> {
  public override getPositions(
    context: DecorationContext,
    random: SimpleRandomSource,
    config: DecoratedDecoratorConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const positions: BlockPos[] = [];
    for (const outerPos of config.outer().getPositions(context, random, pos)) {
      positions.push(...config.inner().getPositions(context, random, outerPos));
    }

    return positions;
  }
}
