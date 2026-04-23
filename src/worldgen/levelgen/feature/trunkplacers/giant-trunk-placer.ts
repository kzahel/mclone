import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { TrunkPlacer } from "./trunk-placer";

export class GiantTrunkPlacer extends TrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    const below = pos.below();
    TrunkPlacer.setDirtAt(level, consumer, random, below, config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.east(), config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.south(), config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.south().east(), config);
    const mutable = new BlockPos.MutableBlockPos();

    for (let y = 0; y < height; y++) {
      this.placeLogIfFreeWithOffset(level, consumer, random, mutable, config, pos, 0, y, 0);
      if (y < height - 1) {
        this.placeLogIfFreeWithOffset(level, consumer, random, mutable, config, pos, 1, y, 0);
        this.placeLogIfFreeWithOffset(level, consumer, random, mutable, config, pos, 1, y, 1);
        this.placeLogIfFreeWithOffset(level, consumer, random, mutable, config, pos, 0, y, 1);
      }
    }

    return [new FoliageAttachment(pos.above(height), 0, true)];
  }

  private placeLogIfFreeWithOffset(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    mutable: BlockPos.MutableBlockPos,
    config: TreeConfiguration,
    pos: BlockPos,
    offsetX: number,
    offsetY: number,
    offsetZ: number,
  ): void {
    mutable.setWithOffset(pos, offsetX, offsetY, offsetZ);
    TrunkPlacer.placeLogIfFree(level, consumer, random, mutable, config);
  }
}
