import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { ChanceDecoratorConfiguration } from "../feature/configurations/chance-decorator-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class ChanceDecorator extends RepeatingDecorator<ChanceDecoratorConfiguration> {
  protected override count(random: SimpleRandomSource, config: ChanceDecoratorConfiguration, _pos: BlockPos): number {
    return random.nextFloat() < 1.0 / config.chance ? 1 : 0;
  }
}
