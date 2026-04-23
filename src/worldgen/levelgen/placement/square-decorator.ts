import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class SquareDecorator extends FeatureDecorator<NoneDecoratorConfiguration> {
  public override getPositions(
    _context: DecorationContext,
    random: SimpleRandomSource,
    _config: NoneDecoratorConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    return [new BlockPos(random.nextInt(16) + pos.getX(), pos.getY(), random.nextInt(16) + pos.getZ())];
  }
}
