import type { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import type { BlockState } from "../../../block/state/block-state";
import { RuleTest } from "./rule-test";
import { RuleTestType } from "./rule-test-type";

export class BlockStateMatchTest extends RuleTest {
  public constructor(private readonly blockState: BlockState) {
    super();
  }

  public override test(state: BlockState, _random: SimpleRandomSource): boolean {
    return state === this.blockState;
  }

  protected override getType(): RuleTestType {
    return RuleTestType.BLOCKSTATE_TEST;
  }
}
