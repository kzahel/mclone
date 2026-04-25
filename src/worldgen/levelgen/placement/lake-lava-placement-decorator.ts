import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { ChanceDecoratorConfiguration } from "../feature/configurations/chance-decorator-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class LakeLavaPlacementDecorator extends RepeatingDecorator<ChanceDecoratorConfiguration> {
  protected override count(random: SimpleRandomSource, _config: ChanceDecoratorConfiguration, pos: BlockPos): number {
    return pos.getY() >= 63 && random.nextInt(10) !== 0 ? 0 : 1;
  }
}
