import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockStateProvider } from "./block-state-provider";

export interface WeightedStateEntry {
  readonly state: BlockState;
  readonly weight: number;
}

export class WeightedStateProvider extends BlockStateProvider {
  private readonly totalWeight: number;

  public constructor(private readonly entries: readonly WeightedStateEntry[]) {
    super();
    this.totalWeight = entries.reduce((sum, entry) => sum + entry.weight, 0);
  }

  public override getState(random: SimpleRandomSource, _pos: BlockPos): BlockState {
    let weight = random.nextInt(this.totalWeight);
    for (const entry of this.entries) {
      weight -= entry.weight;
      if (weight < 0) {
        return entry.state;
      }
    }

    return this.entries[this.entries.length - 1]!.state;
  }
}
