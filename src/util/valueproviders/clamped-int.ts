import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { IntProvider } from "./int-provider";

export class ClampedInt extends IntProvider {
  private constructor(
    private readonly source: IntProvider,
    private readonly minInclusive: number,
    private readonly maxInclusive: number,
  ) {
    super();
    if (maxInclusive < minInclusive) {
      throw new RangeError(`Max must be at least min, min_inclusive: ${minInclusive}, max_inclusive: ${maxInclusive}`);
    }
  }

  public static of(source: IntProvider, minInclusive: number, maxInclusive: number): ClampedInt {
    return new ClampedInt(source, minInclusive, maxInclusive);
  }

  public override sample(random: SimpleRandomSource): number {
    return Math.min(Math.max(this.source.sample(random), this.minInclusive), this.maxInclusive);
  }
}
