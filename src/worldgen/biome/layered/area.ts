import { ImprovedNoise } from "../../noise/improved-noise";
import { SimpleRandomSource } from "../../prng/simple-random-source";
import { linearCongruentialGeneratorNext, wrapLong } from "../../util/linear-congruential-generator";

const MAX_CACHE = 1024;

function floorMod(value: bigint, divisor: bigint): bigint {
  const remainder = value % divisor;
  return remainder >= 0n ? remainder : remainder + divisor;
}

function chunkPosAsLong(x: number, z: number): bigint {
  const packedX = BigInt.asUintN(32, BigInt(x));
  const packedZ = BigInt.asUintN(32, BigInt(z));
  return BigInt.asUintN(64, packedX | (packedZ << 32n));
}

export interface Area {
  get(x: number, z: number): number;
  getMaxCache(): number;
}

export type AreaFactory = () => LazyArea;
export type PixelTransformer = (x: number, z: number) => number;

export interface RandomContext {
  nextRandom(bound: number): number;
  getBiomeNoise(): ImprovedNoise;
}

export interface BigContext extends RandomContext {
  initRandom(x: number, z: number): void;
  createResult(transformer: PixelTransformer, firstArea?: LazyArea, secondArea?: LazyArea): LazyArea;
  random(first: number, second: number): number;
  random(first: number, second: number, third: number, fourth: number): number;
}

export class LazyArea implements Area {
  public constructor(
    private readonly cache: Map<bigint, number>,
    private readonly maxCache: number,
    private readonly transformer: PixelTransformer,
  ) {}

  public get(x: number, z: number): number {
    const key = chunkPosAsLong(x, z);
    const cached = this.cache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const value = this.transformer(x, z);
    this.cache.set(key, value);
    if (this.cache.size > this.maxCache) {
      const removeCount = Math.floor(this.maxCache / 16);
      for (let index = 0; index < removeCount; index++) {
        const oldest = this.cache.keys().next();
        if (oldest.done) {
          break;
        }

        this.cache.delete(oldest.value);
      }
    }

    return value;
  }

  public getMaxCache(): number {
    return this.maxCache;
  }
}

function mixSeed(left: bigint, right: bigint): bigint {
  let mixed = linearCongruentialGeneratorNext(right, right);
  mixed = linearCongruentialGeneratorNext(mixed, right);
  mixed = linearCongruentialGeneratorNext(mixed, right);
  let seed = linearCongruentialGeneratorNext(left, mixed);
  seed = linearCongruentialGeneratorNext(seed, mixed);
  return linearCongruentialGeneratorNext(seed, mixed);
}

export class LazyAreaContext implements BigContext {
  private readonly cache = new Map<bigint, number>();
  private readonly biomeNoise: ImprovedNoise;
  private readonly seed: bigint;
  private rval = 0n;

  public constructor(
    private readonly maxCache: number,
    worldSeed: bigint,
    salt: number,
  ) {
    this.seed = mixSeed(worldSeed, BigInt(salt));
    this.biomeNoise = new ImprovedNoise(new SimpleRandomSource(worldSeed));
  }

  public createResult(transformer: PixelTransformer, firstArea?: LazyArea, secondArea?: LazyArea): LazyArea {
    let resultMaxCache = this.maxCache;
    if (firstArea !== undefined && secondArea !== undefined) {
      resultMaxCache = Math.min(MAX_CACHE, Math.max(firstArea.getMaxCache(), secondArea.getMaxCache()) * 4);
    } else if (firstArea !== undefined) {
      resultMaxCache = Math.min(MAX_CACHE, firstArea.getMaxCache() * 4);
    }

    return new LazyArea(this.cache, resultMaxCache, transformer);
  }

  public initRandom(x: number, z: number): void {
    let value = this.seed;
    value = linearCongruentialGeneratorNext(value, x);
    value = linearCongruentialGeneratorNext(value, z);
    value = linearCongruentialGeneratorNext(value, x);
    value = linearCongruentialGeneratorNext(value, z);
    this.rval = value;
  }

  public nextRandom(bound: number): number {
    if (!Number.isInteger(bound) || bound <= 0) {
      throw new RangeError("bound must be a positive integer");
    }

    const value = Number(floorMod(this.rval >> 24n, BigInt(bound)));
    this.rval = linearCongruentialGeneratorNext(this.rval, this.seed);
    return value;
  }

  public getBiomeNoise(): ImprovedNoise {
    return this.biomeNoise;
  }

  public random(first: number, second: number): number;
  public random(first: number, second: number, third: number, fourth: number): number;
  public random(first: number, second: number, third?: number, fourth?: number): number {
    if (third === undefined || fourth === undefined) {
      return this.nextRandom(2) === 0 ? first : second;
    }

    switch (this.nextRandom(4)) {
      case 0:
        return first;
      case 1:
        return second;
      case 2:
        return third;
      default:
        return fourth;
    }
  }
}

export function wrapSeed(seed: bigint): bigint {
  return wrapLong(seed);
}
