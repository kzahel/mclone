import { BlockPos } from "../../../../core/block-pos";
import { VineBlock } from "../../../../world/level/block/vine-block";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { BooleanProperty } from "../../../../world/level/block/state/properties/boolean-property";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { TreeDecorator } from "./tree-decorator";

function isAir(level: LevelSimulatedReader, pos: BlockPos): boolean {
  return level.isStateAtPosition(pos, (state) => state.isAir());
}

export class LeaveVineDecorator extends TreeDecorator {
  public static readonly INSTANCE = new LeaveVineDecorator();

  private constructor() {
    super();
  }

  public override place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    _trunkPositions: readonly BlockPos[],
    leafPositions: readonly BlockPos[],
  ): void {
    for (const pos of leafPositions) {
      if (random.nextInt(4) === 0) {
        const west = pos.west();
        if (isAir(level, west)) {
          this.addHangingVine(level, west, VineBlock.EAST, consumer);
        }
      }

      if (random.nextInt(4) === 0) {
        const east = pos.east();
        if (isAir(level, east)) {
          this.addHangingVine(level, east, VineBlock.WEST, consumer);
        }
      }

      if (random.nextInt(4) === 0) {
        const north = pos.north();
        if (isAir(level, north)) {
          this.addHangingVine(level, north, VineBlock.SOUTH, consumer);
        }
      }

      if (random.nextInt(4) === 0) {
        const south = pos.south();
        if (isAir(level, south)) {
          this.addHangingVine(level, south, VineBlock.NORTH, consumer);
        }
      }
    }
  }

  private addHangingVine(
    level: LevelSimulatedReader,
    pos: BlockPos,
    property: BooleanProperty,
    consumer: (pos: BlockPos, state: BlockState) => void,
  ): void {
    TreeDecorator.placeVine(consumer, pos, property);
    let remaining = 4;

    for (let current = pos.below(); isAir(level, current) && remaining > 0; current = current.below()) {
      TreeDecorator.placeVine(consumer, current, property);
      remaining--;
    }
  }
}
