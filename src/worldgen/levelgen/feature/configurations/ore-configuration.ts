import { BlockTags } from "../../../../tags/block-tags";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import { TagMatchTest } from "../../../../world/level/levelgen/structure/templatesystem/tag-match-test";
import type { RuleTest } from "../../../../world/level/levelgen/structure/templatesystem/rule-test";
import type { FeatureConfiguration } from "./feature-configuration";

export class OreConfiguration implements FeatureConfiguration {
  public readonly targetStates: readonly OreConfiguration.TargetBlockState[];
  public readonly size: number;
  public readonly discardChanceOnAirExposure: number;

  public constructor(targetStates: readonly OreConfiguration.TargetBlockState[], size: number, discardChanceOnAirExposure?: number);
  public constructor(target: RuleTest, state: BlockState, size: number, discardChanceOnAirExposure?: number);
  public constructor(
    first: readonly OreConfiguration.TargetBlockState[] | RuleTest,
    second: number | BlockState,
    third?: number,
    fourth = 0.0,
  ) {
    if (Array.isArray(first)) {
      this.targetStates = [...first];
      this.size = second as number;
      this.discardChanceOnAirExposure = third ?? 0.0;
      return;
    }

    this.targetStates = [new OreConfiguration.TargetBlockState(first as RuleTest, second as BlockState)];
    this.size = third ?? 0;
    this.discardChanceOnAirExposure = fourth;
  }

  public static target(target: RuleTest, state: BlockState): OreConfiguration.TargetBlockState {
    return new OreConfiguration.TargetBlockState(target, state);
  }
}

export namespace OreConfiguration {
  export class Predicates {
    public static readonly NATURAL_STONE = new TagMatchTest(BlockTags.BASE_STONE_OVERWORLD);
    public static readonly STONE_ORE_REPLACEABLES = new TagMatchTest(BlockTags.STONE_ORE_REPLACEABLES);
    public static readonly DEEPSLATE_ORE_REPLACEABLES = new TagMatchTest(BlockTags.DEEPSLATE_ORE_REPLACEABLES);

    private constructor() {}
  }

  export class TargetBlockState {
    public constructor(
      public readonly target: RuleTest,
      public readonly state: BlockState,
    ) {}
  }
}
