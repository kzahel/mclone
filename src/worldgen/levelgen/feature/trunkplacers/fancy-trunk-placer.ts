import { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import { floor } from "../../../../util/mth";
import { RotatedPillarBlock } from "../../../../world/level/block/rotated-pillar-block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { TreeFeature } from "../tree-feature";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import { FoliageAttachment } from "../foliageplacers/foliage-placer";
import { TrunkPlacer } from "./trunk-placer";

const TRUNK_HEIGHT_SCALE = 0.618;
const CLUSTER_DENSITY_MAGIC = 1.382;
const BRANCH_SLOPE = 0.381;
const BRANCH_LENGTH_MAGIC = 0.328;

interface FoliageCoords {
  readonly attachment: FoliageAttachment;
  readonly branchBase: number;
}

function treeShape(treeHeight: number, y: number): number {
  if (y < treeHeight * 0.3) {
    return -1;
  }

  const halfHeight = treeHeight / 2;
  const heightDelta = halfHeight - y;
  let radius = Math.sqrt((halfHeight * halfHeight) - (heightDelta * heightDelta));
  if (heightDelta === 0) {
    radius = halfHeight;
  } else if (Math.abs(heightDelta) >= halfHeight) {
    return 0;
  }

  return radius * 0.5;
}

export class FancyTrunkPlacer extends TrunkPlacer {
  public override placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[] {
    const treeHeight = height + 2;
    const trunkHeight = floor(treeHeight * TRUNK_HEIGHT_SCALE);
    TrunkPlacer.setDirtAt(level, consumer, random, pos.below(), config);
    const clusterCount = Math.min(1, floor(CLUSTER_DENSITY_MAGIC + ((((1.0 * treeHeight) / 13.0) ** 2))));
    const topY = pos.getY() + trunkHeight;
    let foliageY = treeHeight - 5;
    const foliageCoords: FoliageCoords[] = [
      {
        attachment: new FoliageAttachment(pos.above(foliageY), 0, false),
        branchBase: topY,
      },
    ];

    for (; foliageY >= 0; foliageY--) {
      const shape = treeShape(treeHeight, foliageY);
      if (shape < 0) {
        continue;
      }

      for (let clusterIndex = 0; clusterIndex < clusterCount; clusterIndex++) {
        const branchLength = shape * (random.nextFloat() + BRANCH_LENGTH_MAGIC);
        const branchAngle = random.nextFloat() * 2.0 * Math.PI;
        const branchX = branchLength * Math.sin(branchAngle) + 0.5;
        const branchZ = branchLength * Math.cos(branchAngle) + 0.5;
        const branchPos = pos.offset(branchX, foliageY - 1, branchZ);
        const branchTop = branchPos.above(5);
        if (!this.makeLimb(level, consumer, random, branchPos, branchTop, false, config)) {
          continue;
        }

        const deltaX = pos.getX() - branchPos.getX();
        const deltaZ = pos.getZ() - branchPos.getZ();
        const branchBaseY = branchPos.getY() - (Math.sqrt((deltaX * deltaX) + (deltaZ * deltaZ)) * BRANCH_SLOPE);
        const branchBase = branchBaseY > topY ? topY : Math.trunc(branchBaseY);
        const trunkPos = new BlockPos(pos.getX(), branchBase, pos.getZ());
        if (this.makeLimb(level, consumer, random, trunkPos, branchPos, false, config)) {
          foliageCoords.push({
            attachment: new FoliageAttachment(branchPos, 0, false),
            branchBase: trunkPos.getY(),
          });
        }
      }
    }

    this.makeLimb(level, consumer, random, pos, pos.above(trunkHeight), true, config);
    this.makeBranches(level, consumer, random, treeHeight, pos, foliageCoords, config);
    return foliageCoords
      .filter((coords) => this.trimBranches(treeHeight, coords.branchBase - pos.getY()))
      .map((coords) => coords.attachment);
  }

  private makeLimb(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    start: BlockPos,
    end: BlockPos,
    placeLogs: boolean,
    config: TreeConfiguration,
  ): boolean {
    if (!placeLogs && start.equals(end)) {
      return true;
    }

    const delta = end.offset(-start.getX(), -start.getY(), -start.getZ());
    const steps = this.getSteps(delta);
    const stepX = delta.getX() / steps;
    const stepY = delta.getY() / steps;
    const stepZ = delta.getZ() / steps;

    for (let step = 0; step <= steps; step++) {
      const limbPos = start.offset(0.5 + (step * stepX), 0.5 + (step * stepY), 0.5 + (step * stepZ));
      if (placeLogs) {
        TrunkPlacer.placeLog(level, consumer, random, limbPos, config, (state) =>
          state.setValue(RotatedPillarBlock.AXIS, this.getLogAxis(start, limbPos)),
        );
      } else if (!TreeFeature.isFree(level, limbPos)) {
        return false;
      }
    }

    return true;
  }

  private getSteps(pos: BlockPos): number {
    return Math.max(Math.abs(pos.getX()), Math.max(Math.abs(pos.getY()), Math.abs(pos.getZ())));
  }

  private getLogAxis(start: BlockPos, end: BlockPos): Direction.Axis {
    let axis = Direction.Axis.Y;
    const deltaX = Math.abs(end.getX() - start.getX());
    const deltaZ = Math.abs(end.getZ() - start.getZ());
    const maxDelta = Math.max(deltaX, deltaZ);
    if (maxDelta > 0) {
      axis = deltaX === maxDelta ? Direction.Axis.X : Direction.Axis.Z;
    }

    return axis;
  }

  private trimBranches(treeHeight: number, branchHeight: number): boolean {
    return branchHeight >= treeHeight * 0.2;
  }

  private makeBranches(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    treeHeight: number,
    pos: BlockPos,
    foliageCoords: readonly FoliageCoords[],
    config: TreeConfiguration,
  ): void {
    for (const foliage of foliageCoords) {
      const branchBase = foliage.branchBase;
      const branchPos = new BlockPos(pos.getX(), branchBase, pos.getZ());
      if (!branchPos.equals(foliage.attachment.pos()) && this.trimBranches(treeHeight, branchBase - pos.getY())) {
        this.makeLimb(level, consumer, random, branchPos, foliage.attachment.pos(), true, config);
      }
    }
  }
}
