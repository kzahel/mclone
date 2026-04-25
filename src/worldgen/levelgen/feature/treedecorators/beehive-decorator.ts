import { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import type { Block } from "../../../../world/level/block/block";
import { BeeNestBlock } from "../../../../world/level/block/bee-nest-block";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { Feature } from "../feature";
import { TreeDecorator } from "./tree-decorator";

const BEE_NEST_LOCATION = new ResourceLocation("minecraft:bee_nest");
const BEEHIVE_OFFSETS = [Direction.WEST, Direction.EAST, Direction.SOUTH] as const;

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function getRandomOffset(random: SimpleRandomSource): Direction {
  return BEEHIVE_OFFSETS[random.nextInt(BEEHIVE_OFFSETS.length)]!;
}

export class BeehiveDecorator extends TreeDecorator {
  public constructor(private readonly probability: number) {
    super();
  }

  public override place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    trunkPositions: readonly BlockPos[],
    leafPositions: readonly BlockPos[],
  ): void {
    if (random.nextFloat() >= this.probability || trunkPositions.length === 0) {
      return;
    }

    const hiveY =
      leafPositions.length > 0
        ? Math.max(leafPositions[0]!.getY() - 1, trunkPositions[0]!.getY())
        : Math.min(trunkPositions[0]!.getY() + 1 + random.nextInt(3), trunkPositions[trunkPositions.length - 1]!.getY());
    const trunkCandidates = trunkPositions.filter((pos) => pos.getY() === hiveY);
    if (trunkCandidates.length === 0) {
      return;
    }

    const nestPos = trunkCandidates[random.nextInt(trunkCandidates.length)]!.relative(getRandomOffset(random));
    if (Feature.isAir(level, nestPos) && Feature.isAir(level, nestPos.relative(Direction.SOUTH))) {
      consumer(nestPos, getRequiredState(BEE_NEST_LOCATION).setValue(BeeNestBlock.FACING, Direction.SOUTH));
      // TypeScript: bee nest block-entity occupants are deferred; parity here is the generated block placement.
    }
  }
}
