import { ImprovedNoise, type NoiseRandomSource } from "./improved-noise";
import type { SurfaceNoise } from "./surface-noise";

const ROUND_OFF = 33_554_432;

function skipOctave(random: NoiseRandomSource): void {
  if ("consumeCount" in random && typeof random.consumeCount === "function") {
    random.consumeCount(262);
    return;
  }

  for (let index = 0; index < 262; index++) {
    random.nextInt();
  }
}

function makeAmplitudes(octaves: readonly number[]): { firstOctave: number; amplitudes: number[] } {
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

  const amplitudes = Array.from({ length: total }, () => 0);
  for (const octave of uniqueSortedOctaves) {
    amplitudes[octave + first] = 1;
  }

  return {
    firstOctave: -first,
    amplitudes,
  };
}

function validateAmplitudeConfiguration(firstOctave: number, amplitudes: readonly number[]): { firstOctave: number; amplitudes: number[] } {
  if (!Number.isInteger(firstOctave)) {
    throw new RangeError("firstOctave must be an integer");
  }

  if (amplitudes.length === 0) {
    throw new RangeError("Need some amplitudes!");
  }

  return {
    firstOctave,
    amplitudes: [...amplitudes],
  };
}

export class PerlinNoise implements SurfaceNoise {
  private readonly noiseLevels: Array<ImprovedNoise | undefined>;
  private readonly amplitudes: number[];
  private readonly lowestFreqValueFactor: number;
  private readonly lowestFreqInputFactor: number;

  public constructor(random: NoiseRandomSource, octaves: readonly number[]);
  public constructor(random: NoiseRandomSource, firstOctave: number, amplitudes: readonly number[]);
  public constructor(
    random: NoiseRandomSource,
    octavesOrFirstOctave: readonly number[] | number,
    amplitudesOverride?: readonly number[],
  ) {
    let configuration: { firstOctave: number; amplitudes: number[] };
    if (typeof octavesOrFirstOctave === "number") {
      configuration = validateAmplitudeConfiguration(octavesOrFirstOctave, amplitudesOverride ?? []);
    } else {
      configuration = makeAmplitudes(octavesOrFirstOctave);
    }
    const { firstOctave, amplitudes } = configuration;

    this.amplitudes = amplitudes;

    const baseNoise = new ImprovedNoise(random);
    const size = amplitudes.length;
    const zeroOctaveIndex = -firstOctave;
    this.noiseLevels = Array.from({ length: size }, () => undefined);

    if (zeroOctaveIndex >= 0 && zeroOctaveIndex < size) {
      const amplitude = amplitudes[zeroOctaveIndex]!;
      if (amplitude !== 0) {
        this.noiseLevels[zeroOctaveIndex] = baseNoise;
      }
    }

    for (let index = zeroOctaveIndex - 1; index >= 0; index--) {
      if (index < size) {
        const amplitude = amplitudes[index]!;
        if (amplitude !== 0) {
          this.noiseLevels[index] = new ImprovedNoise(random);
        } else {
          skipOctave(random);
        }
      } else {
        skipOctave(random);
      }
    }

    if (zeroOctaveIndex < size - 1) {
      throw new RangeError("Positive octaves are temporarily disabled");
    }

    this.lowestFreqInputFactor = 2 ** (-zeroOctaveIndex);
    this.lowestFreqValueFactor = (2 ** (size - 1)) / ((2 ** size) - 1);
  }

  public getValue(x: number, y: number, z: number): number;
  public getValue(x: number, y: number, z: number, yScale: number, yMax: number, useFixedY: boolean): number;
  public getValue(x: number, y: number, z: number, yScale = 0, yMax = 0, useFixedY = false): number {
    let value = 0;
    let inputFactor = this.lowestFreqInputFactor;
    let valueFactor = this.lowestFreqValueFactor;

    for (let index = 0; index < this.noiseLevels.length; index++) {
      const noise = this.noiseLevels[index];
      if (noise !== undefined) {
        const sample = noise.noise(
          PerlinNoise.wrap(x * inputFactor),
          useFixedY ? -noise.yo : PerlinNoise.wrap(y * inputFactor),
          PerlinNoise.wrap(z * inputFactor),
          yScale * inputFactor,
          yMax * inputFactor,
        );
        value += this.amplitudes[index]! * sample * valueFactor;
      }

      inputFactor *= 2;
      valueFactor /= 2;
    }

    return value;
  }

  public getSurfaceNoiseValue(x: number, y: number, z: number, yMax: number): number {
    return this.getValue(x, y, 0, z, yMax, false);
  }

  public getOctaveNoise(octave: number): ImprovedNoise | undefined {
    return this.noiseLevels[this.noiseLevels.length - 1 - octave];
  }

  public static create(random: NoiseRandomSource, firstOctave: number, amplitudes: readonly number[]): PerlinNoise {
    return new PerlinNoise(random, firstOctave, amplitudes);
  }

  public static wrap(value: number): number {
    return value - (Math.floor((value / ROUND_OFF) + 0.5) * ROUND_OFF);
  }
}
