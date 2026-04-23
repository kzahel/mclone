import { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { TreeFeature } from "../tree-feature";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { TrunkPlacer } from "./trunk-placer";

function getRandomHorizontalDirection(random: SimpleRandomSource): Direction {
  return Direction.Plane.HORIZONTAL.stream()[random.nextInt(4)]!;
}

export class DarkOakTrunkPlacer extends TrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    const foliageAttachments: FoliageAttachment[] = [];
    const below = pos.below();
    TrunkPlacer.setDirtAt(level, consumer, random, below, config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.east(), config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.south(), config);
    TrunkPlacer.setDirtAt(level, consumer, random, below.south().east(), config);
    const direction = getRandomHorizontalDirection(random);
    const bendStart = height - random.nextInt(4);
    let bendLength = 2 - random.nextInt(3);
    const baseX = pos.getX();
    const baseY = pos.getY();
    const baseZ = pos.getZ();
    let trunkX = baseX;
    let trunkZ = baseZ;
    const topY = baseY + height - 1;

    for (let index = 0; index < height; index++) {
      if (index >= bendStart && bendLength > 0) {
        trunkX += direction.getStepX();
        trunkZ += direction.getStepZ();
        bendLength--;
      }

      const y = baseY + index;
      const trunkPos = new BlockPos(trunkX, y, trunkZ);
      if (TreeFeature.isAirOrLeaves(level, trunkPos)) {
        TrunkPlacer.placeLog(level, consumer, random, trunkPos, config);
        TrunkPlacer.placeLog(level, consumer, random, trunkPos.east(), config);
        TrunkPlacer.placeLog(level, consumer, random, trunkPos.south(), config);
        TrunkPlacer.placeLog(level, consumer, random, trunkPos.east().south(), config);
      }
    }

    foliageAttachments.push(new FoliageAttachment(new BlockPos(trunkX, topY, trunkZ), 0, true));

    for (let dx = -1; dx <= 2; dx++) {
      for (let dz = -1; dz <= 2; dz++) {
        if ((dx < 0 || dx > 1 || dz < 0 || dz > 1) && random.nextInt(3) <= 0) {
          const branchHeight = random.nextInt(3) + 2;

          for (let branchIndex = 0; branchIndex < branchHeight; branchIndex++) {
            TrunkPlacer.placeLog(level, consumer, random, new BlockPos(baseX + dx, topY - branchIndex - 1, baseZ + dz), config);
          }

          foliageAttachments.push(new FoliageAttachment(new BlockPos(trunkX + dx, topY, trunkZ + dz), 0, false));
        }
      }
    }

    return foliageAttachments;
  }
}
