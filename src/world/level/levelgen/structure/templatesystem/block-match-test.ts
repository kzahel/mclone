import type { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import type { Block } from "../../../block/block";
import type { BlockState } from "../../../block/state/block-state";
import { RuleTest } from "./rule-test";
import { RuleTestType } from "./rule-test-type";

export class BlockMatchTest extends RuleTest {
  public constructor(private readonly block: Block) {
    super();
  }

  public override test(state: BlockState, _random: SimpleRandomSource): boolean {
    return state.is(this.block);
  }

  protected override getType(): RuleTestType {
    return RuleTestType.BLOCK_TEST;
  }
}
