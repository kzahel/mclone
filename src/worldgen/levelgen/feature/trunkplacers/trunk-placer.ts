import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { Feature } from "../feature";
import { TreeFeature } from "../tree-feature";
import type { TreeConfiguration } from "../configurations/tree-configuration";
import type { FoliageAttachment } from "../foliageplacers/foliage-placer";

const GRASS_BLOCK_LOCATION = "minecraft:grass_block";
const MYCELIUM_LOCATION = "minecraft:mycelium";

function hasLocation(state: BlockState, location: string): boolean {
  return state.getBlock().getLocation()?.toString() === location;
}

export abstract class TrunkPlacer {
  public constructor(
    protected readonly baseHeight: number,
    protected readonly heightRandA: number,
    protected readonly heightRandB: number,
  ) {}

  public abstract placeTrunk(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    height: number,
    pos: BlockPos,
    config: TreeConfiguration,
  ): readonly FoliageAttachment[];

  public getTreeHeight(random: SimpleRandomSource): number {
    return this.baseHeight + random.nextInt(this.heightRandA + 1) + random.nextInt(this.heightRandB + 1);
  }

  private static isDirt(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => Feature.isDirt(state) && !hasLocation(state, GRASS_BLOCK_LOCATION) && !hasLocation(state, MYCELIUM_LOCATION));
  }

  protected static setDirtAt(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
  ): void {
    if (config.forceDirt || !TrunkPlacer.isDirt(level, pos)) {
      consumer(pos, config.dirtProvider.getState(random, pos));
    }
  }

  protected static placeLog(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
    transform: (state: BlockState) => BlockState = (state) => state,
  ): boolean {
    if (TreeFeature.validTreePos(level, pos)) {
      consumer(pos, transform(config.trunkProvider.getState(random, pos)));
      return true;
    }

    return false;
  }

  protected static placeLogIfFree(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    pos: BlockPos.MutableBlockPos,
    config: TreeConfiguration,
  ): void {
    if (TreeFeature.isFree(level, pos)) {
      TrunkPlacer.placeLog(level, consumer, random, pos, config);
    }
  }
}
