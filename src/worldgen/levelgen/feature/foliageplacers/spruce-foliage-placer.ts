import type { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class SpruceFoliagePlacer extends FoliagePlacer {
  public constructor(
    radius: IntProvider,
    offset: IntProvider,
    private readonly trunkHeight: IntProvider,
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
    const pos = attachment.pos();
    let radius = random.nextInt(2);
    let limit = 1;
    let resetRadius = 0;

    for (let foliageY = offset; foliageY >= -foliageHeight; foliageY--) {
      this.placeLeavesRow(level, consumer, random, config, pos, radius, foliageY, attachment.doubleTrunk());
      if (radius >= limit) {
        radius = resetRadius;
        resetRadius = 1;
        limit = Math.min(limit + 1, foliageRadius + attachment.radiusOffset());
      } else {
        radius++;
      }
    }
  }

  public override foliageHeight(random: SimpleRandomSource, treeHeight: number, _config: TreeConfiguration): number {
    return Math.max(4, treeHeight - this.trunkHeight.sample(random));
  }

  protected override shouldSkipLocation(
    _random: SimpleRandomSource,
    dx: number,
    _y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return dx === radius && dz === radius && radius > 0;
  }
}
