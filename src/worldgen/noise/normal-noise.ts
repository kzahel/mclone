import type { NoiseRandomSource } from "./improved-noise";
import { PerlinNoise } from "./perlin-noise";

const INPUT_FACTOR = 1.0181268882175227;
const VALUE_FACTOR_SCALE = 0.16666666666666666;
const EXPECTED_DEVIATION_FACTOR = 0.1;
const INT_MAX = 2_147_483_647;
const INT_MIN = -2_147_483_648;

function expectedDeviation(octaveSpan: number): number {
  return EXPECTED_DEVIATION_FACTOR * (1 + (1 / (octaveSpan + 1)));
}

export class NormalNoise {
  private readonly valueFactor: number;
  private readonly first: PerlinNoise;
  private readonly second: PerlinNoise;

  public constructor(random: NoiseRandomSource, firstOctave: number, amplitudes: readonly number[]) {
    this.first = PerlinNoise.create(random, firstOctave, amplitudes);
    this.second = PerlinNoise.create(random, firstOctave, amplitudes);

    let minIndex = INT_MAX;
    let maxIndex = INT_MIN;
    for (let index = 0; index < amplitudes.length; index++) {
      if (amplitudes[index] !== 0) {
        minIndex = Math.min(minIndex, index);
        maxIndex = Math.max(maxIndex, index);
      }
    }

    this.valueFactor = VALUE_FACTOR_SCALE / expectedDeviation((maxIndex - minIndex) | 0);
  }

  public getValue(x: number, y: number, z: number): number {
    return (
      this.first.getValue(x, y, z) +
      this.second.getValue(x * INPUT_FACTOR, y * INPUT_FACTOR, z * INPUT_FACTOR)
    ) * this.valueFactor;
  }

  public static create(random: NoiseRandomSource, firstOctave: number, amplitudes: readonly number[]): NormalNoise {
    return new NormalNoise(random, firstOctave, amplitudes);
  }
}
