import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { TreeFeature } from "../tree-feature";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";

export abstract class FoliagePlacer {
  public constructor(
    protected readonly radius: IntProvider,
    protected readonly offsetValue: IntProvider,
  ) {}

  public createFoliage(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    height: number,
    attachment: FoliageAttachment,
    foliageHeight: number,
    foliageRadius: number,
  ): void {
    this.createFoliageLayered(
      level,
      consumer,
      random,
      config,
      height,
      attachment,
      foliageHeight,
      foliageRadius,
      this.offsetValue.sample(random),
    );
  }

  protected abstract createFoliageLayered(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    height: number,
    attachment: FoliageAttachment,
    foliageHeight: number,
    foliageRadius: number,
    offset: number,
  ): void;

  public abstract foliageHeight(random: SimpleRandomSource, treeHeight: number, config: TreeConfiguration): number;

  public foliageRadius(random: SimpleRandomSource, _trunkHeight: number): number {
    return this.radius.sample(random);
  }

  protected abstract shouldSkipLocation(
    random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    doubleTrunk: boolean,
  ): boolean;

  protected shouldSkipLocationSigned(
    random: SimpleRandomSource,
    dx: number,
    y: number,
    dz: number,
    radius: number,
    doubleTrunk: boolean,
  ): boolean {
    let signedDx: number;
    let signedDz: number;
    if (doubleTrunk) {
      signedDx = Math.min(Math.abs(dx), Math.abs(dx - 1));
      signedDz = Math.min(Math.abs(dz), Math.abs(dz - 1));
    } else {
      signedDx = Math.abs(dx);
      signedDz = Math.abs(dz);
    }

    return this.shouldSkipLocation(random, signedDx, y, signedDz, radius, doubleTrunk);
  }

  protected placeLeavesRow(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    pos: BlockPos,
    radius: number,
    y: number,
    doubleTrunk: boolean,
  ): void {
    const extra = doubleTrunk ? 1 : 0;
    const mutable = new BlockPos.MutableBlockPos();

    for (let dx = -radius; dx <= radius + extra; dx++) {
      for (let dz = -radius; dz <= radius + extra; dz++) {
        if (!this.shouldSkipLocationSigned(random, dx, y, dz, radius, doubleTrunk)) {
          mutable.setWithOffset(pos, dx, y, dz);
          FoliagePlacer.tryPlaceLeaf(level, consumer, random, config, mutable);
        }
      }
    }
  }

  protected static tryPlaceLeaf(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    config: TreeConfiguration,
    pos: BlockPos,
  ): void {
    if (TreeFeature.validTreePos(level, pos)) {
      consumer(pos, config.foliageProvider.getState(random, pos));
    }
  }
}

export class FoliageAttachment {
  public constructor(
    private readonly posValue: BlockPos,
    private readonly radiusOffsetValue: number,
    private readonly doubleTrunkValue: boolean,
  ) {}

  public pos(): BlockPos {
    return this.posValue;
  }

  public radiusOffset(): number {
    return this.radiusOffsetValue;
  }

  public doubleTrunk(): boolean {
    return this.doubleTrunkValue;
  }
}
