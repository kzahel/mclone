import { SimpleRandomSource, longPartsToBigInt } from "../worldgen/prng/simple-random-source";

export class JavaRandom {
  private readonly random: SimpleRandomSource;

  public constructor(seed: bigint | number = 0) {
    this.random = new SimpleRandomSource(seed);
  }

  public setSeed(seed: bigint | number): void {
    this.random.setSeed(seed);
  }

  public nextLong(): bigint {
    return longPartsToBigInt(this.random.nextLong());
  }
}
