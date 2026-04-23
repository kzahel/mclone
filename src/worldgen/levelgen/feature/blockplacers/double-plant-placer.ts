import { DoublePlantBlock } from "../../../../world/level/block/double-plant-block";
import { BlockPlacer } from "./block-placer";
import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";

export class DoublePlantPlacer extends BlockPlacer {
  public static readonly INSTANCE = new DoublePlantPlacer();

  public override place(level: WorldGenLevel, pos: BlockPos, state: BlockState, _random: SimpleRandomSource): void {
    DoublePlantBlock.placeAt(level, state, pos, 2);
  }
}
