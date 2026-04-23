import { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import type { Block } from "../../../../world/level/block/block";
import { CocoaBlock } from "../../../../world/level/block/cocoa-block";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { TreeDecorator } from "./tree-decorator";

const COCOA_LOCATION = new ResourceLocation("minecraft:cocoa");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function isAir(level: LevelSimulatedReader, pos: BlockPos): boolean {
  return level.isStateAtPosition(pos, (state) => state.isAir());
}

export class CocoaDecorator extends TreeDecorator {
  public constructor(private readonly probability: number) {
    super();
  }

  public override place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    trunkPositions: readonly BlockPos[],
    _leafPositions: readonly BlockPos[],
  ): void {
    if (random.nextFloat() >= this.probability || trunkPositions.length === 0) {
      return;
    }

    const baseY = trunkPositions[0]!.getY();
    for (const trunkPos of trunkPositions) {
      if (trunkPos.getY() - baseY > 2) {
        continue;
      }

      for (const direction of Direction.Plane.HORIZONTAL) {
        if (random.nextFloat() <= 0.25) {
          const opposite = direction.getOpposite();
          const cocoaPos = trunkPos.offset(opposite.getStepX(), 0, opposite.getStepZ());
          if (isAir(level, cocoaPos)) {
            consumer(
              cocoaPos,
              getRequiredState(COCOA_LOCATION).setValue(CocoaBlock.AGE, random.nextInt(3)).setValue(CocoaBlock.FACING, direction),
            );
          }
        }
      }
    }
  }
}
