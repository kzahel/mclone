import { BlockPos } from "../../../../core/block-pos";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { BooleanProperty } from "../../../../world/level/block/state/properties/boolean-property";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";

const VINE_LOCATION = new ResourceLocation("minecraft:vine");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export abstract class TreeDecorator {
  protected static placeVine(consumer: (pos: BlockPos, state: BlockState) => void, pos: BlockPos, property: BooleanProperty): void {
    consumer(pos, getRequiredState(VINE_LOCATION).setValue(property, true));
  }

  public abstract place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    trunkPositions: readonly BlockPos[],
    leafPositions: readonly BlockPos[],
  ): void;
}
