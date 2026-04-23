import { BlockPos } from "../../../../core/block-pos";
import { VineBlock } from "../../../../world/level/block/vine-block";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { TreeDecorator } from "./tree-decorator";

function isAir(level: LevelSimulatedReader, pos: BlockPos): boolean {
  return level.isStateAtPosition(pos, (state) => state.isAir());
}

export class TrunkVineDecorator extends TreeDecorator {
  public static readonly INSTANCE = new TrunkVineDecorator();

  private constructor() {
    super();
  }

  public override place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    trunkPositions: readonly BlockPos[],
    _leafPositions: readonly BlockPos[],
  ): void {
    for (const pos of trunkPositions) {
      if (random.nextInt(3) > 0) {
        const west = pos.west();
        if (isAir(level, west)) {
          TreeDecorator.placeVine(consumer, west, VineBlock.EAST);
        }
      }

      if (random.nextInt(3) > 0) {
        const east = pos.east();
        if (isAir(level, east)) {
          TreeDecorator.placeVine(consumer, east, VineBlock.WEST);
        }
      }

      if (random.nextInt(3) > 0) {
        const north = pos.north();
        if (isAir(level, north)) {
          TreeDecorator.placeVine(consumer, north, VineBlock.SOUTH);
        }
      }

      if (random.nextInt(3) > 0) {
        const south = pos.south();
        if (isAir(level, south)) {
          TreeDecorator.placeVine(consumer, south, VineBlock.NORTH);
        }
      }
    }
  }
}
