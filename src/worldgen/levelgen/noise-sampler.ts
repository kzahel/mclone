import type { NoiseBiomeSource } from "../biome/noise-biome-source";
import { BlendedNoise } from "../noise/blended-noise";
import { PerlinNoise } from "../noise/perlin-noise";
import { SimplexNoise } from "../noise/simplex-noise";
import type { NoiseModifier as NoiseModifierType } from "./noise-modifier";
import { NoiseSettings } from "./noise-settings";

const OLD_CELL_COUNT_Y = 32;
const BIOME_WEIGHT_RADIUS = 2;
const BIOME_WEIGHT_DIAMETER = (BIOME_WEIGHT_RADIUS * 2) + 1;
const FLOAT_ZERO = Math.fround(0);
const FLOAT_ONE = Math.fround(1);
const FLOAT_POINT_ONE = Math.fround(0.1);
const FLOAT_POINT_ONE_TWO_FIVE = Math.fround(0.125);
const FLOAT_POINT_TWO = Math.fround(0.2);
const FLOAT_POINT_FIVE = Math.fround(0.5);
const FLOAT_POINT_NINE = Math.fround(0.9);
const FLOAT_TWO = Math.fround(2);
const FLOAT_FOUR = Math.fround(4);
const FLOAT_TEN = Math.fround(10);

function f32(value: number): number {
  return Math.fround(value);
}

function buildBiomeWeights(): Float32Array {
  const weights = new Float32Array(BIOME_WEIGHT_DIAMETER * BIOME_WEIGHT_DIAMETER);

    for (let offsetX = -BIOME_WEIGHT_RADIUS; offsetX <= BIOME_WEIGHT_RADIUS; offsetX++) {
      for (let offsetZ = -BIOME_WEIGHT_RADIUS; offsetZ <= BIOME_WEIGHT_RADIUS; offsetZ++) {
        const index = (offsetX + BIOME_WEIGHT_RADIUS) + ((offsetZ + BIOME_WEIGHT_RADIUS) * BIOME_WEIGHT_DIAMETER);
        const squaredDistance = f32((offsetX * offsetX) + (offsetZ * offsetZ) + FLOAT_POINT_TWO);
        const magnitude = f32(Math.sqrt(squaredDistance));
        weights[index] = f32(FLOAT_TEN / magnitude);
      }
    }

  return weights;
}

function clampedLerp(start: number, end: number, delta: number): number {
  if (delta < 0) {
    return start;
  }

  if (delta > 1) {
    return end;
  }

  return start + (delta * (end - start));
}

const BIOME_WEIGHTS = buildBiomeWeights();

export class NoiseSampler {
  private readonly topSlideTarget: number;
  private readonly topSlideSize: number;
  private readonly topSlideOffset: number;
  private readonly bottomSlideTarget: number;
  private readonly bottomSlideSize: number;
  private readonly bottomSlideOffset: number;
  private readonly dimensionDensityFactor: number;
  private readonly dimensionDensityOffset: number;

  public constructor(
    private readonly biomeSource: NoiseBiomeSource,
    private readonly cellWidth: number,
    private readonly cellHeight: number,
    private readonly cellCountY: number,
    private readonly noiseSettings: NoiseSettings,
    private readonly blendedNoise: BlendedNoise,
    private readonly islandNoise: SimplexNoise | undefined,
    private readonly depthNoise: PerlinNoise,
    private readonly caveNoiseModifier: NoiseModifierType,
  ) {
    this.topSlideTarget = noiseSettings.topSlideSettings().target();
    this.topSlideSize = noiseSettings.topSlideSettings().size();
    this.topSlideOffset = noiseSettings.topSlideSettings().offset();
    this.bottomSlideTarget = noiseSettings.bottomSlideSettings().target();
    this.bottomSlideSize = noiseSettings.bottomSlideSettings().size();
    this.bottomSlideOffset = noiseSettings.bottomSlideSettings().offset();
    this.dimensionDensityFactor = noiseSettings.densityFactor();
    this.dimensionDensityOffset = noiseSettings.densityOffset();
  }

  public fillNoiseColumn(
    noiseValues: number[],
    cellX: number,
    cellZ: number,
    noiseSettings: NoiseSettings,
    biomeY: number,
    minCellY: number,
    cellCountY: number,
  ): void {
    if (this.islandNoise !== undefined) {
      throw new RangeError("NoiseSampler island noise override is out of scope for the 1.17.1 overworld target");
    }

    const { depth, scale } = this.computeBiomeDensity(cellX, cellZ, biomeY, noiseSettings);
    const limitHorizontalScale = 684.412 * noiseSettings.noiseSamplingSettings().xzScale();
    const limitVerticalScale = 684.412 * noiseSettings.noiseSamplingSettings().yScale();
    const mainHorizontalScale = limitHorizontalScale / noiseSettings.noiseSamplingSettings().xzFactor();
    const mainVerticalScale = limitVerticalScale / noiseSettings.noiseSamplingSettings().yFactor();
    const randomDensityOffset = noiseSettings.randomDensityOffset() ? this.getRandomDensity(cellX, cellZ) : 0;

    for (let index = 0; index <= cellCountY; index++) {
      const y = index + minCellY;
      let noise = this.blendedNoise.sampleAndClampNoise(
        cellX,
        y,
        cellZ,
        limitHorizontalScale,
        limitVerticalScale,
        mainHorizontalScale,
        mainVerticalScale,
      );
      noise = this.computeInitialDensity(y, depth, scale, randomDensityOffset) + noise;
      noise = this.caveNoiseModifier.modifyNoise(noise, y * this.cellHeight, cellZ * this.cellWidth, cellX * this.cellWidth);
      noiseValues[index] = this.applySlide(noise, y);
    }
  }

  private computeBiomeDensity(cellX: number, cellZ: number, biomeY: number, noiseSettings: NoiseSettings): { depth: number; scale: number } {
    let weightedScale = FLOAT_ZERO;
    let weightedDepth = FLOAT_ZERO;
    let totalWeight = FLOAT_ZERO;
    const centerDepth = f32(this.biomeSource.getNoiseBiome(cellX, biomeY, cellZ).getDepth());

    for (let offsetX = -BIOME_WEIGHT_RADIUS; offsetX <= BIOME_WEIGHT_RADIUS; offsetX++) {
      for (let offsetZ = -BIOME_WEIGHT_RADIUS; offsetZ <= BIOME_WEIGHT_RADIUS; offsetZ++) {
        const biome = this.biomeSource.getNoiseBiome(cellX + offsetX, biomeY, cellZ + offsetZ);
        const biomeDepth = f32(biome.getDepth());
        const biomeScale = f32(biome.getScale());
        let adjustedDepth = biomeDepth;
        let adjustedScale = biomeScale;
        if (noiseSettings.isAmplified() && biomeDepth > 0) {
          adjustedDepth = f32(FLOAT_ONE + f32(biomeDepth * FLOAT_TWO));
          adjustedScale = f32(FLOAT_ONE + f32(biomeScale * FLOAT_FOUR));
        }

        const weightMultiplier = biomeDepth > centerDepth ? FLOAT_POINT_FIVE : FLOAT_ONE;
        const weightIndex = (offsetX + BIOME_WEIGHT_RADIUS) + ((offsetZ + BIOME_WEIGHT_RADIUS) * BIOME_WEIGHT_DIAMETER);
        const weight = f32(f32(weightMultiplier * BIOME_WEIGHTS[weightIndex]!) / f32(adjustedDepth + FLOAT_TWO));
        weightedScale = f32(weightedScale + f32(adjustedScale * weight));
        weightedDepth = f32(weightedDepth + f32(adjustedDepth * weight));
        totalWeight = f32(totalWeight + weight);
      }
    }

    const averageDepth = f32(weightedDepth / totalWeight);
    const averageScale = f32(weightedScale / totalWeight);
    const depthOffset = f32(f32(averageDepth * FLOAT_POINT_FIVE) - FLOAT_POINT_ONE_TWO_FIVE);
    const scaleFactor = f32(f32(averageScale * FLOAT_POINT_NINE) + FLOAT_POINT_ONE);
    return {
      depth: depthOffset * 0.265625,
      scale: 96 / scaleFactor,
    };
  }

  private computeInitialDensity(y: number, depth: number, scale: number, randomDensityOffset: number): number {
    const density = 1 - ((y * 2) / OLD_CELL_COUNT_Y) + randomDensityOffset;
    const dimensionDensity = (density * this.dimensionDensityFactor) + this.dimensionDensityOffset;
    const value = (dimensionDensity + depth) * scale;
    return value * (value > 0 ? 4 : 1);
  }

  private applySlide(noise: number, y: number): number {
    const minCellY = Math.floor(this.noiseSettings.minY() / this.cellHeight);
    const relativeY = y - minCellY;

    if (this.topSlideSize > 0) {
      const topSlideDelta = (this.cellCountY - relativeY - this.topSlideOffset) / this.topSlideSize;
      noise = clampedLerp(this.topSlideTarget, noise, topSlideDelta);
    }

    if (this.bottomSlideSize > 0) {
      const bottomSlideDelta = (relativeY - this.bottomSlideOffset) / this.bottomSlideSize;
      noise = clampedLerp(this.bottomSlideTarget, noise, bottomSlideDelta);
    }

    return noise;
  }

  private getRandomDensity(x: number, z: number): number {
    const value = this.depthNoise.getValue(x * 200, 10, z * 200, 1, 0, true);
    const adjustedValue = value < 0 ? -value * 0.3 : value;
    const density = (adjustedValue * 24.575625) - 2;
    return density < 0 ? density * 0.009486607142857142 : Math.min(density, 1) * 0.006640625;
  }
}
