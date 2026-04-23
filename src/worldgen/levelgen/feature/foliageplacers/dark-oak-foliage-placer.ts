import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";
import { BlockPos } from "../../../../core/block-pos";

export class DarkOakFoliagePlacer extends FoliagePlacer {
  protected override createFoliageLayered(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    _height: number,
    attachment: FoliageAttachment,
    _foliageHeight: number,
    foliageRadius: number,
    offset: number,
  ): void {
    const pos = attachment.pos().above(offset);
    const doubleTrunk = attachment.doubleTrunk();
    if (doubleTrunk) {
      this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + 2, -1, doubleTrunk);
      this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + 3, 0, doubleTrunk);
      this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + 2, 1, doubleTrunk);
      if (random.nextBoolean()) {
        this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius, 2, doubleTrunk);
      }
    } else {
      this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + 2, -1, doubleTrunk);
      this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + 1, 0, doubleTrunk);
    }
  }

  public override foliageHeight(_random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return 4;
  }

  protected override shouldSkipLocation(
    _random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    doubleTrunk: boolean,
  ): boolean {
    if (y === -1 && !doubleTrunk) {
      return dx === radius && dz === radius;
    }

    return y === 1 ? dx + dz > (radius * 2) - 2 : false;
  }

  protected override shouldSkipLocationSigned(
    random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    doubleTrunk: boolean,
  ): boolean {
    return y !== 0 || !doubleTrunk || (dx !== -radius && dx < radius) || (dz !== -radius && dz < radius)
      ? super.shouldSkipLocationSigned(random, dx, y, dz, radius, doubleTrunk)
      : true;
  }
}
