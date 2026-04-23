import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class MegaJungleFoliagePlacer extends FoliagePlacer {
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
    const layerHeight = attachment.doubleTrunk() ? foliageHeight : 1 + random.nextInt(2);

    for (let foliageY = offset; foliageY >= offset - layerHeight; foliageY--) {
      const radius = foliageRadius + attachment.radiusOffset() + 1 - foliageY;
      this.placeLeavesRow(level, consumer, random, config, attachment.pos(), radius, foliageY, attachment.doubleTrunk());
    }
  }

  public override foliageHeight(_random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return this.height;
  }

  protected override shouldSkipLocation(
    _random: SimpleRandomSource,
    dx: number,
    _y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return dx + dz >= 7 ? true : dx * dx + dz * dz > radius * radius;
  }
}
