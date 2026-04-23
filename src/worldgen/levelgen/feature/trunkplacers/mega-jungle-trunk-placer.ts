import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { GiantTrunkPlacer } from "./giant-trunk-placer";

export class MegaJungleTrunkPlacer extends GiantTrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    const foliageAttachments = [...super.placeTrunk(level, consumer, random, height, pos, config)];

    for (let branchY = height - 2 - random.nextInt(4); branchY > height / 2; branchY -= 2 + random.nextInt(4)) {
      const angle = random.nextFloat() * Math.PI * 2.0;
      let branchX = 0;
      let branchZ = 0;

      for (let branchLength = 0; branchLength < 5; branchLength++) {
        branchX = Math.trunc(1.5 + Math.cos(angle) * branchLength);
        branchZ = Math.trunc(1.5 + Math.sin(angle) * branchLength);
        const branchPos = pos.offset(branchX, branchY - 3 + Math.trunc(branchLength / 2), branchZ);
        GiantTrunkPlacer.placeLog(level, consumer, random, branchPos, config);
      }

      foliageAttachments.push(new FoliageAttachment(pos.offset(branchX, branchY, branchZ), -2, false));
    }

    return foliageAttachments;
  }
}
