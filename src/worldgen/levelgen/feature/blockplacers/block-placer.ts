import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";

export abstract class BlockPlacer {
  public abstract place(level: WorldGenLevel, pos: BlockPos, state: BlockState, random: SimpleRandomSource): void;
}
