import type { BlockState } from "../../../../world/level/block/state/block-state";
import { BlockStateMatchTest } from "../../../../world/level/levelgen/structure/templatesystem/block-state-match-test";
import type { FeatureConfiguration } from "./feature-configuration";
import { OreConfiguration } from "./ore-configuration";

export class ReplaceBlockConfiguration implements FeatureConfiguration {
  public readonly targetStates: readonly OreConfiguration.TargetBlockState[];

  public constructor(replacedState: BlockState, state: BlockState);
  public constructor(targetStates: readonly OreConfiguration.TargetBlockState[]);
  public constructor(
    targetStatesOrReplacedState: readonly OreConfiguration.TargetBlockState[] | BlockState,
    state?: BlockState,
  ) {
    if (state !== undefined) {
      this.targetStates = [OreConfiguration.target(new BlockStateMatchTest(targetStatesOrReplacedState as BlockState), state)];
      return;
    }

    this.targetStates = targetStatesOrReplacedState as readonly OreConfiguration.TargetBlockState[];
  }
}
