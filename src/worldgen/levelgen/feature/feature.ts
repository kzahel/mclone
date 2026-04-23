import { BlockPos } from "../../../core/block-pos";
import { BlockTags } from "../../../tags/block-tags";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../world/level/level-simulated-reader";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { ConfiguredFeature } from "./configured-feature";
import type { FeatureConfiguration } from "./configurations/feature-configuration";
import type { FeaturePlaceContext } from "./feature-place-context";

export abstract class Feature<FC extends FeatureConfiguration> {
  public static isDirt(state: BlockState): boolean {
    return state.is(BlockTags.DIRT);
  }

  public static isGrassOrDirt(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, Feature.isDirt);
  }

  public static isAir(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => state.isAir());
  }

  public configured(config: FC): ConfiguredFeature<FC, Feature<FC>> {
    return new ConfiguredFeature(this, config);
  }

  protected setBlock(level: WorldGenLevel, pos: BlockPos, state: BlockState): void {
    level.setBlock(pos, state, 3);
  }

  protected safeSetBlock(level: WorldGenLevel, pos: BlockPos, state: BlockState, predicate: (state: BlockState) => boolean): void {
    if (predicate(level.getBlockState(pos))) {
      level.setBlock(pos, state, 2);
    }
  }

  public abstract place(context: FeaturePlaceContext<FC>): boolean;
}
