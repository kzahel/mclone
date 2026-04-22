import {
  SimpleRandomSource,
  type Int64Parts,
  type LongSeed,
  longPartsToBigInt,
} from "./simple-random-source";

const BASE_CHUNK_MULTIPLIER_X = 341_873_128_712n;
const BASE_CHUNK_MULTIPLIER_Z = 132_897_987_541n;
const SLIME_CHUNK_MULTIPLIER_X_SQUARED = 4_987_142n;
const SLIME_CHUNK_MULTIPLIER_X = 5_947_611n;
const SLIME_CHUNK_MULTIPLIER_Z_SQUARED = 4_392_871n;
const SLIME_CHUNK_MULTIPLIER_Z = 389_711n;

function asBigInt(value: number): bigint {
  if (!Number.isInteger(value)) {
    throw new RangeError("Expected an integer coordinate");
  }

  return BigInt(value);
}

function wrapLong(value: bigint): bigint {
  return BigInt.asIntN(64, value);
}

export class WorldgenRandom extends SimpleRandomSource {
  private count = 0;

  public constructor(seed: LongSeed = 0) {
    super(seed);
  }

  public getCount(): number {
    return this.count;
  }

  public override nextInt(): number;
  public override nextInt(bound: number): number;
  public override nextInt(bound?: number): number {
    this.count++;
    return bound === undefined ? super.nextInt() : super.nextInt(bound);
  }

  public override nextLong(): Int64Parts {
    this.count += 2;
    return super.nextLong();
  }

  public override nextBoolean(): boolean {
    this.count++;
    return super.nextBoolean();
  }

  public override nextFloat(): number {
    this.count++;
    return super.nextFloat();
  }

  public override nextDouble(): number {
    this.count += 2;
    return super.nextDouble();
  }

  public override nextGaussian(): number {
    return super.nextGaussian();
  }

  public override consumeCount(count: number): void {
    super.consumeCount(count);
  }

  public setBaseChunkSeed(chunkX: number, chunkZ: number): bigint {
    const seed = wrapLong((asBigInt(chunkX) * BASE_CHUNK_MULTIPLIER_X) + (asBigInt(chunkZ) * BASE_CHUNK_MULTIPLIER_Z));
    this.setSeed(seed);
    return seed;
  }

  public setDecorationSeed(levelSeed: LongSeed, minChunkBlockX: number, minChunkBlockZ: number): bigint {
    this.setSeed(levelSeed);
    const xMultiplier = longPartsToBigInt(this.nextLong()) | 1n;
    const zMultiplier = longPartsToBigInt(this.nextLong()) | 1n;
    const seed =
      wrapLong((asBigInt(minChunkBlockX) * xMultiplier) + (asBigInt(minChunkBlockZ) * zMultiplier)) ^ BigInt(levelSeed);
    this.setSeed(seed);
    return seed;
  }

  public setFeatureSeed(decorationSeed: LongSeed, index: number, decorationStep: number): bigint {
    const seed = wrapLong(BigInt(decorationSeed) + asBigInt(index) + (10_000n * asBigInt(decorationStep)));
    this.setSeed(seed);
    return seed;
  }

  public setLargeFeatureSeed(baseSeed: LongSeed, chunkX: number, chunkZ: number): bigint {
    this.setSeed(baseSeed);
    const xMultiplier = longPartsToBigInt(this.nextLong());
    const zMultiplier = longPartsToBigInt(this.nextLong());
    const seed = wrapLong((asBigInt(chunkX) * xMultiplier) ^ (asBigInt(chunkZ) * zMultiplier) ^ BigInt(baseSeed));
    this.setSeed(seed);
    return seed;
  }

  public setBaseStoneSeed(levelSeed: LongSeed, x: number, y: number, z: number): bigint {
    this.setSeed(levelSeed);
    const xMultiplier = longPartsToBigInt(this.nextLong());
    const yMultiplier = longPartsToBigInt(this.nextLong());
    const zMultiplier = longPartsToBigInt(this.nextLong());
    const seed =
      wrapLong((asBigInt(x) * xMultiplier) ^ (asBigInt(y) * yMultiplier) ^ (asBigInt(z) * zMultiplier) ^ BigInt(levelSeed));
    this.setSeed(seed);
    return seed;
  }

  public setLargeFeatureWithSalt(levelSeed: LongSeed, regionX: number, regionZ: number, salt: number): bigint {
    const seed =
      wrapLong(
        (asBigInt(regionX) * BASE_CHUNK_MULTIPLIER_X) +
          (asBigInt(regionZ) * BASE_CHUNK_MULTIPLIER_Z) +
          BigInt(levelSeed) +
          asBigInt(salt),
      );
    this.setSeed(seed);
    return seed;
  }

  public static seedSlimeChunk(chunkX: number, chunkZ: number, levelSeed: LongSeed, salt: LongSeed): WorldgenRandom {
    const x = asBigInt(chunkX);
    const z = asBigInt(chunkZ);
    const seed = wrapLong(
      BigInt(levelSeed) +
        (x * x * SLIME_CHUNK_MULTIPLIER_X_SQUARED) +
        (x * SLIME_CHUNK_MULTIPLIER_X) +
        (z * z * SLIME_CHUNK_MULTIPLIER_Z_SQUARED) +
        (z * SLIME_CHUNK_MULTIPLIER_Z) ^
        BigInt(salt),
    );
    return new WorldgenRandom(seed);
  }
}
