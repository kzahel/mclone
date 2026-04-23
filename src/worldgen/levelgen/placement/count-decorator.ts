import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { CountConfiguration } from "../feature/configurations/count-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class CountDecorator extends RepeatingDecorator<CountConfiguration> {
  protected override count(random: SimpleRandomSource, config: CountConfiguration, _pos: BlockPos): number {
    return config.count().sample(random);
  }
}
