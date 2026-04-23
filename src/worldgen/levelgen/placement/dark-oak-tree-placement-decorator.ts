import { BlockPos } from "../../../core/block-pos";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorator } from "./feature-decorator";

export class DarkOakTreePlacementDecorator extends FeatureDecorator<NoneDecoratorConfiguration> {
  public override getPositions(
    _context: DecorationContext,
    random: SimpleRandomSource,
    _config: NoneDecoratorConfiguration,
    pos: BlockPos,
  ): readonly BlockPos[] {
    const positions: BlockPos[] = [];
    for (let index = 0; index < 16; index++) {
      const cellX = Math.floor(index / 4);
      const cellZ = index % 4;
      const worldX = (cellX * 4) + 1 + random.nextInt(3) + pos.getX();
      const worldZ = (cellZ * 4) + 1 + random.nextInt(3) + pos.getZ();
      positions.push(new BlockPos(worldX, pos.getY(), worldZ));
    }

    return positions;
  }
}
