import { RangeDecoratorConfiguration } from "../feature/configurations/range-decorator-configuration";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { DecorationContext } from "./decoration-context";
import { VerticalDecorator } from "./vertical-decorator";

export class RangeDecorator extends VerticalDecorator<RangeDecoratorConfiguration> {
  protected override y(
    context: DecorationContext,
    random: SimpleRandomSource,
    config: RangeDecoratorConfiguration,
    _y: number,
  ): number {
    return config.height.sample(random, {
      minY: context.getMinBuildHeight(),
      genDepth: context.getGenDepth(),
    });
  }
}
