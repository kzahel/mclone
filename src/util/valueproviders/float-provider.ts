import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";

export abstract class FloatProvider {
  public abstract sample(random: SimpleRandomSource): number;
}
