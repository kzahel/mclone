import type { NoiseRandomSource } from "./improved-noise";
import { PerlinNoise } from "./perlin-noise";

const LIMIT_OCTAVES = Array.from({ length: 16 }, (_, index) => index - 15);
const MAIN_OCTAVES = Array.from({ length: 8 }, (_, index) => index - 7);

function clampedLerp(start: number, end: number, delta: number): number {
  if (delta < 0) {
    return start;
  }

  if (delta > 1) {
    return end;
  }

  return start + (delta * (end - start));
}

export class BlendedNoise {
  private readonly minLimitNoise: PerlinNoise;
  private readonly maxLimitNoise: PerlinNoise;
  private readonly mainNoise: PerlinNoise;

  public constructor(minLimitNoise: PerlinNoise, maxLimitNoise: PerlinNoise, mainNoise: PerlinNoise);
  public constructor(random: NoiseRandomSource);
  public constructor(
    minLimitNoiseOrRandom: NoiseRandomSource | PerlinNoise,
    maxLimitNoise?: PerlinNoise,
    mainNoise?: PerlinNoise,
  ) {
    if (maxLimitNoise !== undefined || mainNoise !== undefined) {
      if (!(minLimitNoiseOrRandom instanceof PerlinNoise) || maxLimitNoise === undefined || mainNoise === undefined) {
        throw new TypeError("Expected either a random source or three PerlinNoise instances");
      }

      this.minLimitNoise = minLimitNoiseOrRandom;
      this.maxLimitNoise = maxLimitNoise;
      this.mainNoise = mainNoise;
      return;
    }

    const random = minLimitNoiseOrRandom as NoiseRandomSource;
    this.minLimitNoise = new PerlinNoise(random, LIMIT_OCTAVES);
    this.maxLimitNoise = new PerlinNoise(random, LIMIT_OCTAVES);
    this.mainNoise = new PerlinNoise(random, MAIN_OCTAVES);
  }

  public sampleAndClampNoise(
    x: number,
    y: number,
    z: number,
    limitHorizontalScale: number,
    limitVerticalScale: number,
    mainHorizontalScale: number,
    mainVerticalScale: number,
  ): number {
    let min = 0;
    let max = 0;
    let main = 0;
    let inputFactor = 1;

    for (let octave = 0; octave < 8; octave++) {
      const noise = this.mainNoise.getOctaveNoise(octave);
      if (noise !== undefined) {
        main += noise.noise(
          PerlinNoise.wrap(x * mainHorizontalScale * inputFactor),
          PerlinNoise.wrap(y * mainVerticalScale * inputFactor),
          PerlinNoise.wrap(z * mainHorizontalScale * inputFactor),
          mainVerticalScale * inputFactor,
          y * mainVerticalScale * inputFactor,
        ) / inputFactor;
      }

      inputFactor /= 2;
    }

    const blend = ((main / 10) + 1) / 2;
    const skipMin = blend >= 1;
    const skipMax = blend <= 0;
    inputFactor = 1;

    for (let octave = 0; octave < 16; octave++) {
      const wrappedX = PerlinNoise.wrap(x * limitHorizontalScale * inputFactor);
      const wrappedY = PerlinNoise.wrap(y * limitVerticalScale * inputFactor);
      const wrappedZ = PerlinNoise.wrap(z * limitHorizontalScale * inputFactor);
      const yScale = limitVerticalScale * inputFactor;
      if (!skipMin) {
        const noise = this.minLimitNoise.getOctaveNoise(octave);
        if (noise !== undefined) {
          min += noise.noise(wrappedX, wrappedY, wrappedZ, yScale, y * yScale) / inputFactor;
        }
      }

      if (!skipMax) {
        const noise = this.maxLimitNoise.getOctaveNoise(octave);
        if (noise !== undefined) {
          max += noise.noise(wrappedX, wrappedY, wrappedZ, yScale, y * yScale) / inputFactor;
        }
      }

      inputFactor /= 2;
    }

    return clampedLerp(min / 512, max / 512, blend);
  }
}
