import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";
import { IntProvider } from "./int-provider";

export class ConstantInt extends IntProvider {
  private constructor(private readonly value: number) {
    super();
  }

  public static of(value: number): ConstantInt {
    return new ConstantInt(value);
  }

  public override sample(_random: SimpleRandomSource): number {
    return this.value;
  }
}
