import type { NoiseRandomSource } from "./improved-noise";
import { SimplexNoise } from "./simplex-noise";
import type { SurfaceNoise } from "./surface-noise";
import { WorldgenRandom } from "../prng/worldgen-random";

const RESEED_FACTOR = Math.fround(9.223372e18);
const LONG_MIN = -9_223_372_036_854_775_808n;
const LONG_MAX = 9_223_372_036_854_775_807n;
const DOUBLE_VIEW = new DataView(new ArrayBuffer(8));

function consumeCount(random: NoiseRandomSource, count: number): void {
  if ("consumeCount" in random && typeof random.consumeCount === "function") {
    random.consumeCount(count);
    return;
  }

  for (let index = 0; index < count; index++) {
    random.nextInt();
  }
}

function toJavaLong(value: number): bigint {
  if (Number.isNaN(value)) {
    return 0n;
  }

  if (!Number.isFinite(value)) {
    return value < 0 ? LONG_MIN : LONG_MAX;
  }

  DOUBLE_VIEW.setFloat64(0, value, false);
  const high = DOUBLE_VIEW.getUint32(0, false);
  const low = DOUBLE_VIEW.getUint32(4, false);
  const sign = (high >>> 31) !== 0;
  const exponent = (high >>> 20) & 0x7ff;
  const mantissa = (BigInt(high & 0x000f_ffff) << 32n) | BigInt(low);
  if (exponent === 0) {
    return 0n;
  }

  const unbiasedExponent = exponent - 1023;
  if (unbiasedExponent < 0) {
    return 0n;
  }

  let integer = (1n << 52n) | mantissa;
  if (unbiasedExponent > 52) {
    integer <<= BigInt(unbiasedExponent - 52);
  } else {
    integer >>= BigInt(52 - unbiasedExponent);
  }

  if (sign) {
    integer = -integer;
  }

  if (integer < LONG_MIN) {
    return LONG_MIN;
  }

  if (integer > LONG_MAX) {
    return LONG_MAX;
  }

  return integer;
}

export class PerlinSimplexNoise implements SurfaceNoise {
  private readonly noiseLevels: Array<SimplexNoise | undefined>;
  private readonly highestFreqValueFactor: number;
  private readonly highestFreqInputFactor: number;

  public constructor(random: NoiseRandomSource, octaves: readonly number[]) {
    const uniqueSortedOctaves = [...new Set(octaves)].sort((left, right) => left - right);
    if (uniqueSortedOctaves.length === 0) {
      throw new RangeError("Need some octaves!");
    }

    const first = -uniqueSortedOctaves[0]!;
    const last = uniqueSortedOctaves.at(-1)!;
    const total = first + last + 1;
    if (total < 1) {
      throw new RangeError("Total number of octaves needs to be >= 1");
    }

    const octaveSet = new Set(uniqueSortedOctaves);
    const baseNoise = new SimplexNoise(random);
    const highestOctave = last;
    this.noiseLevels = Array.from({ length: total }, () => undefined);

    if (last >= 0 && last < total && octaveSet.has(0)) {
      this.noiseLevels[last] = baseNoise;
    }

    for (let index = last + 1; index < total; index++) {
      if (index >= 0 && octaveSet.has(highestOctave - index)) {
        this.noiseLevels[index] = new SimplexNoise(random);
      } else {
        consumeCount(random, 262);
      }
    }

    if (last > 0) {
      const seed = toJavaLong(baseNoise.getValue(baseNoise.xo, baseNoise.yo, baseNoise.zo) * RESEED_FACTOR);
      const fork = new WorldgenRandom(seed);

      for (let index = highestOctave - 1; index >= 0; index--) {
        if (index < total && octaveSet.has(highestOctave - index)) {
          this.noiseLevels[index] = new SimplexNoise(fork);
        } else {
          fork.consumeCount(262);
        }
      }
    }

    this.highestFreqInputFactor = 2 ** last;
    this.highestFreqValueFactor = 1 / ((2 ** total) - 1);
  }

  public getValue(x: number, y: number, useNoiseOffsets: boolean): number {
    let value = 0;
    let inputFactor = this.highestFreqInputFactor;
    let valueFactor = this.highestFreqValueFactor;

    for (const noise of this.noiseLevels) {
      if (noise !== undefined) {
        value += noise.getValue(
          (x * inputFactor) + (useNoiseOffsets ? noise.xo : 0),
          (y * inputFactor) + (useNoiseOffsets ? noise.yo : 0),
        ) * valueFactor;
      }

      inputFactor /= 2;
      valueFactor *= 2;
    }

    return value;
  }

  public getSurfaceNoiseValue(x: number, y: number, _z: number, _yMax: number): number {
    return this.getValue(x, y, true) * 0.55;
  }
}
