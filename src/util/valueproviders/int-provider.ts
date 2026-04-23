import type { SimpleRandomSource } from "../../worldgen/prng/simple-random-source";

export abstract class IntProvider {
  public abstract sample(random: SimpleRandomSource): number;
}
