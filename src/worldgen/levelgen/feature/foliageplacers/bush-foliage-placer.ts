import { BlobFoliagePlacer } from "./blob-foliage-placer";
import type { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "./foliage-placer";

export class BushFoliagePlacer extends BlobFoliagePlacer {
  public constructor(radius: IntProvider, offset: IntProvider, height: number) {
    super(radius, offset, height);
  }

  protected override createFoliageLayered(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    _height: number,
    attachment: FoliageAttachment,
    foliageHeight: number,
    foliageRadius: number,
    offset: number,
  ): void {
    for (let foliageY = offset; foliageY >= offset - foliageHeight; foliageY--) {
      const radius = foliageRadius + attachment.radiusOffset() - 1 - foliageY;
      this.placeLeavesRow(level, consumer, random, config, attachment.pos(), radius, foliageY, attachment.doubleTrunk());
    }
  }

  protected override shouldSkipLocation(
    random: SimpleRandomSource,
    dx: number,
    _y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return dx === radius && dz === radius && random.nextInt(2) === 0;
  }
}
