import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class MegaPineFoliagePlacer extends FoliagePlacer {
  public constructor(
    radius: IntProvider,
    offset: IntProvider,
    private readonly crownHeight: IntProvider,
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
    let previousRadius = 0;

    for (let y = pos.getY() - foliageHeight + offset; y <= pos.getY() + offset; y++) {
      const yFromTop = pos.getY() - y;
      const radius = foliageRadius + attachment.radiusOffset() + Math.floor((yFromTop / foliageHeight) * 3.5);
      const rowRadius = yFromTop > 0 && radius === previousRadius && (y & 1) === 0 ? radius + 1 : radius;
      this.placeLeavesRow(level, consumer, random, config, new BlockPos(pos.getX(), y, pos.getZ()), rowRadius, 0, attachment.doubleTrunk());
      previousRadius = radius;
    }
  }

  public override foliageHeight(random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return this.crownHeight.sample(random);
  }

  protected override shouldSkipLocation(
    _random: SimpleRandomSource,
    dx: number,
    _y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return dx + dz >= 7 || (dx * dx) + (dz * dz) > radius * radius;
  }
}
