import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "../feature/configurations/frequency-with-extra-chance-decorator-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class CountWithExtraChanceDecorator extends RepeatingDecorator<FrequencyWithExtraChanceDecoratorConfiguration> {
  protected override count(
    random: SimpleRandomSource,
    config: FrequencyWithExtraChanceDecoratorConfiguration,
    _pos: BlockPos,
  ): number {
    return config.count + (random.nextFloat() < config.extraChance ? config.extraCount : 0);
  }
}
