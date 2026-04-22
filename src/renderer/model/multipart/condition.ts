import type { BlockState } from "../../../world/level/block/state/block-state";
import type { StateDefinition } from "../../../world/level/block/state/state-definition";
import type { Block } from "../../../world/level/block/block";

export interface Condition {
  getPredicate(definition: StateDefinition<Block, BlockState>): (state: BlockState) => boolean;
}

class ConstantCondition implements Condition {
  public constructor(private readonly predicateFactory: () => (state: BlockState) => boolean) {}

  public getPredicate(_definition: StateDefinition<Block, BlockState>): (state: BlockState) => boolean {
    return this.predicateFactory();
  }
}

export const TRUE_CONDITION: Condition = new ConstantCondition(() => () => true);
export const FALSE_CONDITION: Condition = new ConstantCondition(() => () => false);
