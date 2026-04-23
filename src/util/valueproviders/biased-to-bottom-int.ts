import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { IntProvider } from "./int-provider";

export class BiasedToBottomInt extends IntProvider {
  private constructor(
    private readonly minInclusive: number,
    private readonly maxInclusive: number,
  ) {
    super();
    if (maxInclusive < minInclusive) {
      throw new RangeError(`Max must be at least min, min_inclusive: ${minInclusive}, max_inclusive: ${maxInclusive}`);
    }
  }

  public static of(minInclusive: number, maxInclusive: number): BiasedToBottomInt {
    return new BiasedToBottomInt(minInclusive, maxInclusive);
  }

  public override sample(random: SimpleRandomSource): number {
    return this.minInclusive + random.nextInt(random.nextInt((this.maxInclusive - this.minInclusive) + 1) + 1);
  }
}
