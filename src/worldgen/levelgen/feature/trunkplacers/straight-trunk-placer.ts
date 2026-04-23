import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { TrunkPlacer } from "./trunk-placer";

export class StraightTrunkPlacer extends TrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    StraightTrunkPlacer.setDirtAt(level, consumer, random, pos.below(), config);

    for (let y = 0; y < height; y++) {
      StraightTrunkPlacer.placeLog(level, consumer, random, pos.above(y), config);
    }

    return [new FoliageAttachment(pos.above(height), 0, false)];
  }
}
