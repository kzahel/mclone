import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { FloatProvider } from "./float-provider";

export class UniformFloat extends FloatProvider {
  private constructor(
    private readonly minInclusive: number,
    private readonly maxExclusive: number,
  ) {
    super();
  }

  public static of(minInclusive: number, maxExclusive: number): UniformFloat {
    if (maxExclusive <= minInclusive) {
      throw new Error("Max must be larger than min");
    }

    return new UniformFloat(minInclusive, maxExclusive);
  }

  public override sample(random: SimpleRandomSource): number {
    return this.minInclusive + (random.nextFloat() * (this.maxExclusive - this.minInclusive));
  }
}
