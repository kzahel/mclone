import type { BlockState } from "../../../world/level/block/state/block-state";
import type { StateDefinition } from "../../../world/level/block/state/state-definition";
import type { Block } from "../../../world/level/block/block";
import { type Condition } from "./condition";

export class AndCondition implements Condition {
  public static readonly TOKEN = "AND";

  public constructor(private readonly conditions: readonly Condition[]) {}

  public getPredicate(definition: StateDefinition<Block, BlockState>): (state: BlockState) => boolean {
    const predicates = this.conditions.map((condition) => condition.getPredicate(definition));
    return (state) => predicates.every((predicate) => predicate(state));
  }
}
