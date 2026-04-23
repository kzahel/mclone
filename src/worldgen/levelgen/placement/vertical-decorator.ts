import { BlockPos } from "../../../core/block-pos";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export abstract class VerticalDecorator<DC extends DecoratorConfiguration> extends FeatureDecorator<DC> {
  protected abstract y(context: DecorationContext, random: SimpleRandomSource, config: DC, y: number): number;

  public override getPositions(
    context: DecorationContext,
    random: SimpleRandomSource,
    config: DC,
    pos: BlockPos,
  ): readonly BlockPos[] {
    return [new BlockPos(pos.getX(), this.y(context, random, config, pos.getY()), pos.getZ())];
  }
}
