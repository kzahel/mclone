import type { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import type { BlockState } from "../../../block/state/block-state";
import type { RuleTestType } from "./rule-test-type";

export abstract class RuleTest {
  public abstract test(state: BlockState, random: SimpleRandomSource): boolean;

  protected abstract getType(): RuleTestType;

  public type(): RuleTestType {
    return this.getType();
  }
}
