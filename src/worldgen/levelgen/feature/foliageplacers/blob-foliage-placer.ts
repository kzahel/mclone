import type { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class BlobFoliagePlacer extends FoliagePlacer {
  public constructor(
    radius: IntProvider,
    offset: IntProvider,
    protected readonly height: number,
  ) {
    super(radius, offset);
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
      const radius = Math.max(foliageRadius + attachment.radiusOffset() - 1 - Math.floor(foliageY / 2), 0);
      this.placeLeavesRow(level, consumer, random, config, attachment.pos(), radius, foliageY, attachment.doubleTrunk());
    }
  }

  public override foliageHeight(_random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return this.height;
  }

  protected override shouldSkipLocation(
    random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return dx === radius && dz === radius && (random.nextInt(2) === 0 || y === 0);
  }
}
