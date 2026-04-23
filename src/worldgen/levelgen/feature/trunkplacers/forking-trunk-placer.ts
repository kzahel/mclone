import { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { TrunkPlacer } from "./trunk-placer";

function getRandomHorizontalDirection(random: SimpleRandomSource): Direction {
  return Direction.Plane.HORIZONTAL.stream()[random.nextInt(4)]!;
}

export class ForkingTrunkPlacer extends TrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    TrunkPlacer.setDirtAt(level, consumer, random, pos.below(), config);
    const foliageAttachments: FoliageAttachment[] = [];
    const direction = getRandomHorizontalDirection(random);
    const bendStart = height - random.nextInt(4) - 1;
    let bendLength = 3 - random.nextInt(3);
    const mutablePos = new BlockPos.MutableBlockPos();
    let trunkX = pos.getX();
    let trunkZ = pos.getZ();
    let topY = 0;

    for (let index = 0; index < height; index++) {
      const y = pos.getY() + index;
      if (index >= bendStart && bendLength > 0) {
        trunkX += direction.getStepX();
        trunkZ += direction.getStepZ();
        bendLength--;
      }

      if (TrunkPlacer.placeLog(level, consumer, random, mutablePos.set(trunkX, y, trunkZ), config)) {
        topY = y + 1;
      }
    }

    foliageAttachments.push(new FoliageAttachment(new BlockPos(trunkX, topY, trunkZ), 1, false));
    trunkX = pos.getX();
    trunkZ = pos.getZ();
    const secondDirection = getRandomHorizontalDirection(random);
    if (secondDirection !== direction) {
      const secondBendStart = bendStart - random.nextInt(2) - 1;
      let secondBendLength = 1 + random.nextInt(3);
      topY = 0;

      for (let index = secondBendStart; index < height && secondBendLength > 0; secondBendLength--) {
        if (index >= 1) {
          const y = pos.getY() + index;
          trunkX += secondDirection.getStepX();
          trunkZ += secondDirection.getStepZ();
          if (TrunkPlacer.placeLog(level, consumer, random, mutablePos.set(trunkX, y, trunkZ), config)) {
            topY = y + 1;
          }
        }

        index++;
      }

      if (topY > 1) {
        foliageAttachments.push(new FoliageAttachment(new BlockPos(trunkX, topY, trunkZ), 0, false));
      }
    }

    return foliageAttachments;
  }
}
