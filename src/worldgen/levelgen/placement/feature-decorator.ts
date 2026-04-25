import type { DecorationContext } from "./decoration-context";
import { ConfiguredDecorator } from "./configured-decorator";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";

export abstract class FeatureDecorator<DC extends DecoratorConfiguration> {
  public configured(config: DC): ConfiguredDecorator<DC> {
    return new ConfiguredDecorator(this, config);
  }

  public abstract getPositions(context: DecorationContext, random: SimpleRandomSource, config: DC, pos: BlockPos): Iterable<BlockPos>;
}
