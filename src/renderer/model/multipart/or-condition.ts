import type { BlockState } from "../../../world/level/block/state/block-state";
import type { StateDefinition } from "../../../world/level/block/state/state-definition";
import type { Block } from "../../../world/level/block/block";
import { type Condition } from "./condition";

export class OrCondition implements Condition {
  public static readonly TOKEN = "OR";

  public constructor(private readonly conditions: readonly Condition[]) {}

  public getPredicate(definition: StateDefinition<Block, BlockState>): (state: BlockState) => boolean {
    const predicates = this.conditions.map((condition) => condition.getPredicate(definition));
    return (state) => predicates.some((predicate) => predicate(state));
  }
}
