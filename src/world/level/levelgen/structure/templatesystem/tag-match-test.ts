import type { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import type { BlockState } from "../../../block/state/block-state";
import type { BlockTag } from "../../../../../tags/block-tags";
import { RuleTest } from "./rule-test";
import { RuleTestType } from "./rule-test-type";

export class TagMatchTest extends RuleTest {
  public constructor(private readonly tag: BlockTag) {
    super();
  }

  public override test(state: BlockState, _random: SimpleRandomSource): boolean {
    return state.is(this.tag);
  }

  protected override getType(): RuleTestType {
    return RuleTestType.TAG_TEST;
  }
}
