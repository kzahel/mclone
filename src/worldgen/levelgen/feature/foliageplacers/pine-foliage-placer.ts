import type { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class PineFoliagePlacer extends FoliagePlacer {
  public constructor(
    radius: IntProvider,
    offset: IntProvider,
    private readonly heightValue: IntProvider,
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
    let radius = 0;
    for (let foliageY = offset; foliageY >= offset - foliageHeight; foliageY--) {
      this.placeLeavesRow(level, consumer, random, config, attachment.pos(), radius, foliageY, attachment.doubleTrunk());
      if (radius >= 1 && foliageY === offset - foliageHeight + 1) {
        radius--;
      } else if (radius < foliageRadius + attachment.radiusOffset()) {
        radius++;
      }
    }
  }

  public override foliageRadius(random: SimpleRandomSource, trunkHeight: number): number {
    return super.foliageRadius(random, trunkHeight) + random.nextInt(Math.max(trunkHeight + 1, 1));
  }

  public override foliageHeight(random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return this.heightValue.sample(random);
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
