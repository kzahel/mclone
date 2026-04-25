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
  ): Iterable<BlockPos> {
    return this.generatePositions(context, random, config, pos);
  }

  private *generatePositions(
    context: DecorationContext,
    random: SimpleRandomSource,
    config: DecoratedDecoratorConfiguration,
    pos: BlockPos,
  ): IterableIterator<BlockPos> {
    for (const outerPos of config.outer().getPositions(context, random, pos)) {
      yield* config.inner().getPositions(context, random, outerPos);
    }
  }
}
