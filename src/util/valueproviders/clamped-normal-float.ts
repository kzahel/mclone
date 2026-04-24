import { clamp } from "../mth";
import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { FloatProvider } from "./float-provider";

export class ClampedNormalFloat extends FloatProvider {
  private constructor(
    private readonly mean: number,
    private readonly deviation: number,
    private readonly min: number,
    private readonly max: number,
  ) {
    super();
  }

  public static of(mean: number, deviation: number, min: number, max: number): ClampedNormalFloat {
    if (max < min) {
      throw new Error(`Max must be larger than min: [${min}, ${max}]`);
    }

    return new ClampedNormalFloat(mean, deviation, min, max);
  }

  public static sample(random: SimpleRandomSource, mean: number, deviation: number, min: number, max: number): number {
    return clamp((random.nextGaussian() * deviation) + mean, min, max);
  }

  public override sample(random: SimpleRandomSource): number {
    return ClampedNormalFloat.sample(random, this.mean, this.deviation, this.min, this.max);
  }
}
