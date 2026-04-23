import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockPlacer } from "./block-placer";

export class SimpleBlockPlacer extends BlockPlacer {
  public static readonly INSTANCE = new SimpleBlockPlacer();

  private constructor() {
    super();
  }

  public override place(level: WorldGenLevel, pos: BlockPos, state: BlockState, _random: SimpleRandomSource): void {
    level.setBlock(pos, state, 2);
  }
}
