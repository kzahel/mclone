import { BlockPos } from "../../../core/block-pos";
import { BlockTags } from "../../../tags/block-tags";
import { HugeMushroomBlock } from "../../../world/level/block/huge-mushroom-block";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { FeaturePlaceContext } from "./feature-place-context";
import { Feature } from "./feature";
import { HugeMushroomFeatureConfiguration } from "./configurations/huge-mushroom-feature-configuration";
import type { SimpleRandomSource } from "../../prng/simple-random-source";

export abstract class AbstractHugeMushroomFeature extends Feature<HugeMushroomFeatureConfiguration> {
  protected placeTrunk(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    pos: BlockPos,
    config: HugeMushroomFeatureConfiguration,
    height: number,
    mutablePos: BlockPos.MutableBlockPos,
  ): void {
    for (let index = 0; index < height; index++) {
      mutablePos.set(pos.getX(), pos.getY(), pos.getZ()).move(0, index, 0);
      if (!level.getBlockState(mutablePos).isSolidRender(level, mutablePos)) {
        this.setBlock(level, mutablePos, config.stemProvider.getState(random, pos));
      }
    }
  }

  protected getTreeHeight(random: SimpleRandomSource): number {
    let height = random.nextInt(3) + 4;
    if (random.nextInt(12) === 0) {
      height *= 2;
    }

    return height;
  }

  protected isValidPosition(
    level: WorldGenLevel,
    pos: BlockPos,
    height: number,
    mutablePos: BlockPos.MutableBlockPos,
    config: HugeMushroomFeatureConfiguration,
  ): boolean {
    const y = pos.getY();
    if (y < level.getMinBuildHeight() + 1 || y + height + 1 >= level.getMaxBuildHeight()) {
      return false;
    }

    const belowState = level.getBlockState(pos.below());
    if (!Feature.isDirt(belowState) && !belowState.is(BlockTags.MUSHROOM_GROW_BLOCK)) {
      return false;
    }

    for (let currentHeight = 0; currentHeight <= height; currentHeight++) {
      const radius = this.getTreeRadiusForHeight(-1, -1, config.foliageRadius, currentHeight);

      for (let dx = -radius; dx <= radius; dx++) {
        for (let dz = -radius; dz <= radius; dz++) {
          const state = level.getBlockState(mutablePos.setWithOffset(pos, dx, currentHeight, dz));
          if (!state.isAir() && !state.is(BlockTags.LEAVES)) {
            return false;
          }
        }
      }
    }

    return true;
  }

  public override place(context: FeaturePlaceContext<HugeMushroomFeatureConfiguration>): boolean {
    const level = context.level();
    const pos = context.origin();
    const random = context.random();
    const config = context.config();
    const height = this.getTreeHeight(random);
    const mutablePos = new BlockPos.MutableBlockPos();
    if (!this.isValidPosition(level, pos, height, mutablePos, config)) {
      return false;
    }

    this.makeCap(level, random, pos, height, mutablePos, config);
    this.placeTrunk(level, random, pos, config, height, mutablePos);
    return true;
  }

  protected abstract getTreeRadiusForHeight(
    totalHeight: number,
    capTopY: number,
    foliageRadius: number,
    layerY: number,
  ): number;

  protected abstract makeCap(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    pos: BlockPos,
    height: number,
    mutablePos: BlockPos.MutableBlockPos,
    config: HugeMushroomFeatureConfiguration,
  ): void;

  protected static hasDirectionalProperties(state: BlockState): boolean {
    return (
      state.hasProperty(HugeMushroomBlock.WEST) &&
      state.hasProperty(HugeMushroomBlock.EAST) &&
      state.hasProperty(HugeMushroomBlock.NORTH) &&
      state.hasProperty(HugeMushroomBlock.SOUTH)
    );
  }
}
