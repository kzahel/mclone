const BASE_16 = 0x1_0000;
const MASK_16 = 0xffff;
const MULTIPLIER_LOW = 0xe66d;
const MULTIPLIER_MID = 0xdeec;
const MULTIPLIER_HIGH = 0x0005;
const ADDEND = 0x000b;
const MULTIPLIER_BIGINT = 0x5deece66dn;
const TWO_POW_24 = 16_777_216;
const TWO_POW_27 = 134_217_728;
const TWO_POW_53 = 9_007_199_254_740_992;

export interface Int64Parts {
  hi: number;
  lo: number;
}

export type LongSeed = bigint | number | string;

function normalizeLongSeed(seed: LongSeed): bigint {
  if (typeof seed === "bigint") {
    return BigInt.asIntN(64, seed);
  }

  if (typeof seed === "number") {
    if (!Number.isSafeInteger(seed)) {
      throw new RangeError("number seeds must be safe integers; use bigint for full 64-bit seeds");
    }

    return BigInt(seed);
  }

  return BigInt.asIntN(64, BigInt(seed));
}

export function longPartsToBigInt(parts: Int64Parts): bigint {
  return BigInt.asIntN(64, (BigInt(parts.hi | 0) << 32n) + BigInt(parts.lo | 0));
}

export function longPartsToSignedDecimalString(parts: Int64Parts): string {
  return longPartsToBigInt(parts).toString();
}

export class SimpleRandomSource {
  private seedHigh = 0;
  private seedMid = 0;
  private seedLow = 0;
  private nextNextGaussian = 0;
  private haveNextNextGaussian = false;

  public constructor(seed: LongSeed) {
    this.setSeed(seed);
  }

  public setSeed(seed: LongSeed): void {
    const scrambled = BigInt.asUintN(48, normalizeLongSeed(seed) ^ MULTIPLIER_BIGINT);
    this.seedHigh = Number((scrambled >> 32n) & 0xffffn);
    this.seedMid = Number((scrambled >> 16n) & 0xffffn);
    this.seedLow = Number(scrambled & 0xffffn);
  }

  public nextInt(): number;
  public nextInt(bound: number): number;
  public nextInt(bound?: number): number {
    if (bound === undefined) {
      return this.nextBits(32) | 0;
    }

    if (!Number.isInteger(bound) || bound <= 0) {
      throw new RangeError("Bound must be positive");
    }

    if ((bound & (bound - 1)) === 0) {
      return Number((BigInt(bound) * BigInt(this.nextBits(31))) >> 31n);
    }

    let bits = 0;
    let value = 0;
    do {
      bits = this.nextBits(31);
      value = bits % bound;
    } while ((((bits - value) + (bound - 1)) | 0) < 0);

    return value;
  }

  public nextLong(): Int64Parts {
    return {
      hi: this.nextBits(32),
      lo: this.nextBits(32),
    };
  }

  public nextBoolean(): boolean {
    return this.nextBits(1) !== 0;
  }

  public nextFloat(): number {
    return this.nextBits(24) / TWO_POW_24;
  }

  public nextDouble(): number {
    const upper = this.nextBits(26);
    const lower = this.nextBits(27);
    return ((upper * TWO_POW_27) + lower) / TWO_POW_53;
  }

  public nextGaussian(): number {
    if (this.haveNextNextGaussian) {
      this.haveNextNextGaussian = false;
      return this.nextNextGaussian;
    }

    let first = 0;
    let second = 0;
    let radiusSquared = 0;
    do {
      first = (2 * this.nextDouble()) - 1;
      second = (2 * this.nextDouble()) - 1;
      radiusSquared = (first * first) + (second * second);
    } while (radiusSquared >= 1 || radiusSquared === 0);

    const scale = Math.sqrt((-2 * Math.log(radiusSquared)) / radiusSquared);
    this.nextNextGaussian = second * scale;
    this.haveNextNextGaussian = true;
    return first * scale;
  }

  private nextBits(bits: number): number {
    if (!Number.isInteger(bits) || bits < 1 || bits > 32) {
      throw new RangeError("bits must be between 1 and 32");
    }

    // Three 16-bit limbs keep the 48-bit LCG exact while staying in safe JS integer ranges.
    const step0 = (this.seedLow * MULTIPLIER_LOW) + ADDEND;
    const nextLow = step0 & MASK_16;
    const carry0 = Math.floor(step0 / BASE_16);

    const step1 = (this.seedLow * MULTIPLIER_MID) + (this.seedMid * MULTIPLIER_LOW) + carry0;
    const nextMid = step1 & MASK_16;
    const carry1 = Math.floor(step1 / BASE_16);

    const step2 =
      (this.seedLow * MULTIPLIER_HIGH) +
      (this.seedMid * MULTIPLIER_MID) +
      (this.seedHigh * MULTIPLIER_LOW) +
      carry1;

    this.seedLow = nextLow;
    this.seedMid = nextMid;
    this.seedHigh = step2 & MASK_16;

    const upper32 = ((this.seedHigh << 16) | this.seedMid) >>> 0;
    return bits === 32 ? upper32 : upper32 >>> (32 - bits);
  }
}
