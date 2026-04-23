import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { IntProvider } from "./int-provider";

export class UniformInt extends IntProvider {
  private constructor(
    private readonly minInclusive: number,
    private readonly maxInclusive: number,
  ) {
    super();
  }

  public static of(minInclusive: number, maxInclusive: number): UniformInt {
    return new UniformInt(minInclusive, maxInclusive);
  }

  public override sample(random: SimpleRandomSource): number {
    return this.minInclusive + random.nextInt((this.maxInclusive - this.minInclusive) + 1);
  }
}
