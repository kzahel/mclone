import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export abstract class RepeatingDecorator<DC extends DecoratorConfiguration> extends FeatureDecorator<DC> {
  protected abstract count(random: SimpleRandomSource, config: DC, pos: BlockPos): number;

  public override getPositions(
    _context: DecorationContext,
    random: SimpleRandomSource,
    config: DC,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const positions: BlockPos[] = [];
    const count = this.count(random, config, pos);
    for (let index = 0; index < count; index++) {
      positions.push(pos);
    }

    return positions;
  }
}
