import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class Spread32AboveDecorator extends FeatureDecorator<NoneDecoratorConfiguration> {
  public override getPositions(
    _context: DecorationContext,
    random: SimpleRandomSource,
    _config: NoneDecoratorConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    return [new BlockPos(pos.getX(), random.nextInt(Math.max(pos.getY(), 0) + 32), pos.getZ())];
  }
}
