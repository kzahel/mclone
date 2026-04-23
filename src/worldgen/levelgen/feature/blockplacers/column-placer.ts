import { Direction } from "../../../../core/direction";
import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../../world/level/world-gen-level";
import { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockPlacer } from "./block-placer";

export class ColumnPlacer extends BlockPlacer {
  public constructor(private readonly size: IntProvider) {
    super();
  }

  public override place(level: WorldGenLevel, pos: BlockPos, state: BlockState, random: SimpleRandomSource): void {
    const mutablePos = pos.mutable();
    const count = this.size.sample(random);

    for (let index = 0; index < count; index++) {
      level.setBlock(mutablePos, state, 2);
      mutablePos.move(Direction.UP);
    }
  }
}
