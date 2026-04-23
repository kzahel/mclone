import { BlockPos } from "../../../core/block-pos";
import { BlockTags } from "../../../tags/block-tags";
import type { WorldGenLevel } from "../world-gen-level";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";

export class BambooSaplingBlock extends Block {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    return level.getBlockState(pos.below()).is(BlockTags.BAMBOO_PLANTABLE_ON);
  }
}
