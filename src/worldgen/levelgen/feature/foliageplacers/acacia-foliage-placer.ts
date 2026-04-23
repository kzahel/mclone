import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment, FoliagePlacer } from "./foliage-placer";

export class AcaciaFoliagePlacer extends FoliagePlacer {
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
    const doubleTrunk = attachment.doubleTrunk();
    const pos = attachment.pos().above(offset);
    this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + attachment.radiusOffset(), -1 - foliageHeight, doubleTrunk);
    this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius - 1, -foliageHeight, doubleTrunk);
    this.placeLeavesRow(level, consumer, random, config, pos, foliageRadius + attachment.radiusOffset() - 1, 0, doubleTrunk);
  }

  public override foliageHeight(_random: SimpleRandomSource, _treeHeight: number, _config: TreeConfiguration): number {
    return 0;
  }

  protected override shouldSkipLocation(
    _random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    _doubleTrunk: boolean,
  ): boolean {
    return y === 0
      ? (dx > 1 || dz > 1) && dx !== 0 && dz !== 0
      : dx === radius && dz === radius && radius > 0;
  }
}
